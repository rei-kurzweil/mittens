//! Application-owned, classic-pass opaque surface renderer for mobile Vulkan.
//! The engine supplies retained materials, transforms and the ordered draw stream.
mod materials;
mod pass;
mod pipeline;
mod resources;
pub(crate) mod view;

use crate::{
    graphics_device::Gpu,
    xr_host::{ApiResult, Result},
};
use ash::vk;
use materials::Materials;
use mittens_engine::engine::graphics::{
    CpuMesh, MeshHandle, MeshUploader, TextureHandle, TextureUploader,
    visual_world::{RenderOp, VisualWorld},
};
use pass::EyePass;
use resources::{Buffer, Image, bytes};
use std::sync::Arc;
use view::{CameraUniform, EyeCamera};

struct Mesh {
    vertices: Buffer,
    indices: Buffer,
    count: u32,
}
#[repr(C)]
struct Draw {
    model: [[f32; 4]; 4],
    color: [f32; 4],
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Stats {
    pub draws: usize,
    pub instances: usize,
    pub pipelines: usize,
    pub bindings: usize,
    pub meshes: usize,
    pub textures: usize,
    pub instance_capacity: usize,
}
pub(crate) struct SceneRenderer {
    gpu: Arc<Gpu>,
    pool: vk::CommandPool,
    command: vk::CommandBuffer,
    fence: vk::Fence,
    materials: Materials,
    pass: EyePass,
    meshes: Vec<Mesh>,
    textures: Vec<Image>,
    instance_buffer: Buffer,
    instances: Vec<Draw>,
}
impl Drop for SceneRenderer {
    fn drop(&mut self) {
        unsafe {
            // Also covers queue submission/fence failures before the normal wait.
            let _ = self.gpu.device.device_wait_idle();
            self.gpu.device.destroy_fence(self.fence, None);
            self.gpu.device.destroy_command_pool(self.pool, None);
        }
    }
}
impl SceneRenderer {
    #[allow(dead_code)] // 1x entry point for the matched desktop comparison.
    pub fn new(
        gpu: Arc<Gpu>,
        format: vk::Format,
        extent: vk::Extent2D,
        images: Vec<u64>,
    ) -> Result<Self> {
        Self::with_samples(gpu, format, extent, images, vk::SampleCountFlags::TYPE_1)
    }
    pub fn with_samples(
        gpu: Arc<Gpu>,
        format: vk::Format,
        extent: vk::Extent2D,
        images: Vec<u64>,
        samples: vk::SampleCountFlags,
    ) -> Result<Self> {
        let pass = EyePass::new(gpu.clone(), format, extent, &images, samples)?;
        let materials = Materials::new(gpu.clone())?;
        let instance_buffer = Buffer::new(
            gpu.clone(),
            64 * std::mem::size_of::<Draw>(),
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )?;
        let mut renderer = Self {
            gpu,
            pass,
            materials,
            meshes: Vec::new(),
            textures: Vec::new(),
            instance_buffer,
            instances: Vec::with_capacity(64),
            pool: vk::CommandPool::null(),
            command: vk::CommandBuffer::null(),
            fence: vk::Fence::null(),
        };
        // Texture 0 is the explicit white binding for an untextured surface.
        renderer.textures.push(resources::upload_rgba(
            renderer.gpu.clone(),
            &[255; 4],
            1,
            1,
            true,
        )?);
        unsafe {
            renderer.pool = renderer
                .gpu
                .device
                .create_command_pool(
                    &vk::CommandPoolCreateInfo::default()
                        .queue_family_index(renderer.gpu.family)
                        .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER),
                    None,
                )
                .api("scene command pool")?;
            renderer.command = renderer
                .gpu
                .device
                .allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(renderer.pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1),
                )
                .api("scene command buffer")?[0];
            renderer.fence = renderer
                .gpu
                .device
                .create_fence(&vk::FenceCreateInfo::default(), None)
                .api("scene fence")?;
        }
        Ok(renderer)
    }
    pub fn attachment_policy(&self) -> String {
        let color = if self.pass.samples == vk::SampleCountFlags::TYPE_1 {
            "color=CLEAR/STORE"
        } else {
            "msaa_color=CLEAR/DONT_CARE resolve=DONT_CARE/STORE"
        };
        format!(
            "classic opaque eye pass; {color} depth=CLEAR/DONT_CARE transient=true depth_lazy={} format={:?} samples={} msaa_lazy={} in_pass_resolve={}",
            self.pass.lazy_depth(),
            self.pass.depth_format,
            self.pass.samples.as_raw(),
            self.pass.lazy_msaa_color(),
            self.pass.samples != vk::SampleCountFlags::TYPE_1,
        )
    }
    pub fn draw(
        &mut self,
        index: usize,
        visuals: &VisualWorld,
        cameras: &[EyeCamera; 2],
    ) -> Result<Stats> {
        if index >= self.pass.target_count() {
            return Err("eye target index out of range".into());
        }
        if visuals.effective_post_processing().is_active() || !visuals.mirrors().is_empty() {
            return Err(
                "post-processing and mirrors are unsupported by the mobile opaque pass".into(),
            );
        }
        // Reject unsupported scene semantics instead of rendering them incorrectly
        // or selecting a legacy program. Every retained instance is checked,
        // including ones placed in phases other than the opaque stream.
        for instance in visuals.instances() {
            if instance.opacity != 1.0
                || instance.color[3] != 1.0
                || instance.transparent_cutout
                || instance.background
                || instance.overlay
                || instance.background_occluded_lit
                || instance.emissive != 0.0
                || instance.toon_outline.is_some()
                || instance.bones_count != 0
                || instance.deformed_count != 0
                || instance.stencil_ref != 0
                || instance.is_stencil_clip
            {
                return Err(format!(
                    "unsupported mobile surface: opacity={} alpha={} cutout={} background={} overlay={} occluded_lit={} emissive={} transmission={} outline={} bones={} deformed={} stencil={} clip={}",
                    instance.opacity,
                    instance.color[3],
                    instance.transparent_cutout,
                    instance.background,
                    instance.overlay,
                    instance.background_occluded_lit,
                    instance.emissive,
                    instance.transmission[2],
                    instance.toon_outline.is_some(),
                    instance.bones_count,
                    instance.deformed_count,
                    instance.stencil_ref,
                    instance.is_stencil_clip
                ));
            }
            if instance.material_instance.is_none() {
                return Err(
                    "mobile renderer requires a custom Material(definition) on every surface"
                        .into(),
                );
            }
        }
        self.materials.collect(&visuals.materials)?;
        let (ops, indices) = visuals.opaque_stream();
        // Upload/compile work happens before recording the pass. Resources and
        // descriptors are retained between frames, and shared by both eyes.
        for op in ops {
            let RenderOp::DrawBatch(batch) = op else {
                return Err("stencil clips are unsupported by the mobile opaque pass".into());
            };
            if batch.stencil_ref != 0 {
                return Err("stencil clips are unsupported".into());
            }
            self.meshes
                .get(batch.mesh.0 as usize)
                .ok_or("unknown mobile mesh handle")?;
            let texture = self
                .textures
                .get(batch.texture.map_or(0, |h| h.0 as usize))
                .ok_or("unknown mobile texture handle")?;
            let id = batch
                .material_instance
                .ok_or("missing custom material in draw batch")?;
            self.materials.prepare(
                &visuals.materials,
                (id, batch.texture, batch.texture_filtering),
                self.pass.handle,
                self.pass.samples,
                texture,
            )?;
        }
        for (eye, camera) in cameras.iter().enumerate() {
            let uniform = CameraUniform::new(camera, visuals, self.pass.extent);
            self.materials
                .update_camera(eye, unsafe { bytes(std::slice::from_ref(&uniform)) })?;
        }
        self.instances.clear();
        self.instances.extend(indices.iter().map(|&index| {
            let instance = &visuals.instances()[index as usize];
            Draw {
                model: instance.transform.matrix_world,
                color: instance.color,
            }
        }));
        let size = std::mem::size_of_val(self.instances.as_slice());
        if size > self.instance_buffer.capacity {
            self.instance_buffer = Buffer::new(
                self.gpu.clone(),
                size.next_power_of_two(),
                vk::BufferUsageFlags::VERTEX_BUFFER,
            )?;
        }
        self.instance_buffer
            .write(unsafe { bytes(&self.instances) })?;
        let mut stats = Stats::default();
        unsafe {
            let device = &self.gpu.device;
            device
                .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
                .api("reset scene commands")?;
            device
                .begin_command_buffer(
                    self.command,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .api("begin scene commands")?;
            for eye in 0..2 {
                self.pass
                    .begin(self.command, index, eye, visuals.clear_color());
                device.cmd_bind_descriptor_sets(
                    self.command,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.materials.layout,
                    0,
                    &[self.materials.camera_sets[eye]],
                    &[],
                );
                for op in ops {
                    let RenderOp::DrawBatch(batch) = op else {
                        unreachable!()
                    };
                    let key = (
                        batch.material_instance.unwrap(),
                        batch.texture,
                        batch.texture_filtering,
                    );
                    // Asynchronous custom compilation may still be pending on
                    // the first frames. Show the clear until it is ready.
                    let Some((pipeline, set)) = self.materials.binding(&visuals.materials, key)
                    else {
                        continue;
                    };
                    let mesh = &self.meshes[batch.mesh.0 as usize];
                    device.cmd_bind_pipeline(
                        self.command,
                        vk::PipelineBindPoint::GRAPHICS,
                        pipeline,
                    );
                    device.cmd_bind_descriptor_sets(
                        self.command,
                        vk::PipelineBindPoint::GRAPHICS,
                        self.materials.layout,
                        1,
                        &[set],
                        &[],
                    );
                    device.cmd_bind_vertex_buffers(
                        self.command,
                        0,
                        &[mesh.vertices.handle, self.instance_buffer.handle],
                        &[0, 0],
                    );
                    device.cmd_bind_index_buffer(
                        self.command,
                        mesh.indices.handle,
                        0,
                        vk::IndexType::UINT32,
                    );
                    device.cmd_draw_indexed(
                        self.command,
                        mesh.count,
                        batch.count as u32,
                        0,
                        0,
                        batch.start as u32,
                    );
                    stats.draws += 1;
                    stats.instances += batch.count;
                }
                device.cmd_end_render_pass(self.command);
            }
            device
                .end_command_buffer(self.command)
                .api("end scene commands")?;
            device
                .reset_fences(&[self.fence])
                .api("reset scene fence")?;
            device
                .queue_submit(
                    self.gpu.queue,
                    &[vk::SubmitInfo::default().command_buffers(&[self.command])],
                    self.fence,
                )
                .api("submit scene")?;
            // Conservative bring-up synchronization. XR must not regain the
            // image until both eye passes have completed, including on errors.
            device
                .wait_for_fences(&[self.fence], true, u64::MAX)
                .api("complete scene before XR release")?;
        }
        (stats.pipelines, stats.bindings) = self.materials.counts();
        stats.meshes = self.meshes.len();
        stats.textures = self.textures.len();
        stats.instance_capacity = self.instance_buffer.capacity;
        Ok(stats)
    }
}
impl MeshUploader for SceneRenderer {
    fn upload_mesh(
        &mut self,
        mesh: &CpuMesh,
    ) -> std::result::Result<MeshHandle, Box<dyn std::error::Error>> {
        if mesh.vertices.is_empty()
            || mesh.indices_u32.is_empty()
            || mesh.indices_u32.len() % 3 != 0
            || mesh
                .indices_u32
                .iter()
                .any(|&i| i as usize >= mesh.vertices.len())
        {
            return Err("invalid triangle mesh".into());
        }
        if mesh.joints0.is_some() || mesh.weights0.is_some() || !mesh.morph_targets.is_empty() {
            return Err("mobile renderer does not yet support skinning/morph meshes".into());
        }
        let mut vertices = Buffer::new(
            self.gpu.clone(),
            std::mem::size_of_val(mesh.vertices.as_slice()),
            vk::BufferUsageFlags::VERTEX_BUFFER,
        )?;
        vertices.write(unsafe { bytes(&mesh.vertices) })?;
        let mut indices = Buffer::new(
            self.gpu.clone(),
            std::mem::size_of_val(mesh.indices_u32.as_slice()),
            vk::BufferUsageFlags::INDEX_BUFFER,
        )?;
        indices.write(unsafe { bytes(&mesh.indices_u32) })?;
        let handle = MeshHandle(u32::try_from(self.meshes.len())?);
        self.meshes.push(Mesh {
            vertices,
            indices,
            count: u32::try_from(mesh.indices_u32.len())?,
        });
        Ok(handle)
    }
}
impl TextureUploader for SceneRenderer {
    fn upload_texture_rgba8(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> std::result::Result<TextureHandle, Box<dyn std::error::Error>> {
        let image = resources::upload_rgba(self.gpu.clone(), rgba, width, height, true)?;
        let handle = TextureHandle(u32::try_from(self.textures.len())?);
        self.textures.push(image);
        Ok(handle)
    }
    fn upload_texture_bc7(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
        _: bool,
    ) -> std::result::Result<TextureHandle, Box<dyn std::error::Error>> {
        Err("BC7 is unsupported on the Focus 3; supply RGBA8 textures for this renderer".into())
    }
}
