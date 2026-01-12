use crate::engine::graphics::MeshUploader;
use crate::engine::graphics::TextureUploader;
use crate::engine::graphics::mesh::CpuMesh;
use crate::engine::graphics::primitives::MeshHandle;
use crate::engine::graphics::primitives::TextureHandle;
use crate::engine::graphics::visual_world::VisualWorld;
use std::sync::Arc;
use winit::window::Window;

mod vulkano_backend {
    use std::collections::HashMap;
    use std::mem::size_of;
    use std::sync::Arc;

    use crate::engine::graphics::mesh::{CpuMesh, CpuVertex};
    use crate::engine::graphics::pipeline_descriptor_set_layouts::PipelineDescriptorSetLayouts;
    use crate::engine::graphics::primitives::MeshHandle;
    use crate::engine::graphics::primitives::TextureHandle;
    use crate::engine::graphics::visual_world::VisualWorld;
    use crate::engine::graphics::vulkano_swapchain::VulkanoSwapchainState;
    use vulkano::buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer};
    use vulkano::command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferInfo, PrimaryCommandBufferAbstract,
        RenderPassBeginInfo, SubpassBeginInfo, SubpassEndInfo,
        allocator::StandardCommandBufferAllocator,
    };
    use vulkano::descriptor_set::allocator::StandardDescriptorSetAllocator;
    use vulkano::descriptor_set::{DescriptorSet, WriteDescriptorSet};
    use vulkano::format::ClearValue;
    use vulkano::image::view::ImageView;
    use vulkano::image::{Image, ImageCreateInfo, ImageType, ImageUsage};
    use vulkano::memory::allocator::{AllocationCreateInfo, MemoryTypeFilter};
    use vulkano::pipeline::graphics::color_blend::{
        AttachmentBlend, BlendFactor, BlendOp, ColorBlendAttachmentState, ColorBlendState,
        ColorComponents,
    };
    use vulkano::pipeline::graphics::depth_stencil::{DepthState, DepthStencilState};
    use vulkano::pipeline::graphics::input_assembly::InputAssemblyState;
    use vulkano::pipeline::graphics::multisample::MultisampleState;
    use vulkano::pipeline::graphics::rasterization::RasterizationState;
    use vulkano::pipeline::graphics::subpass::PipelineSubpassType;
    use vulkano::pipeline::graphics::vertex_input::{
        VertexInputAttributeDescription, VertexInputBindingDescription, VertexInputRate,
        VertexInputState,
    };
    use vulkano::pipeline::graphics::viewport::{Scissor, Viewport, ViewportState};
    use vulkano::pipeline::layout::{PipelineLayout, PipelineLayoutCreateInfo};

    use vulkano::DeviceSize;
    use vulkano::command_buffer::CopyBufferToImageInfo;
    use vulkano::format::Format;
    use vulkano::image::sampler::{Sampler, SamplerCreateInfo};
    use vulkano::pipeline::{
        DynamicState, GraphicsPipeline, Pipeline, PipelineBindPoint, PipelineShaderStageCreateInfo,
    };
    use vulkano::render_pass::{RenderPass, Subpass};
    use vulkano::swapchain::{self, SwapchainPresentInfo};
    use vulkano::sync::{self, GpuFuture};
    use vulkano::{Validated, VulkanError};
    use vulkano_util::context::{VulkanoConfig, VulkanoContext};
    use winit::window::Window;

    mod toon_mesh_vs {
        vulkano_shaders::shader! {
            ty: "vertex",
            path: "assets/shaders/toon-mesh.vert",
        }
    }

    mod toon_mesh_fs {
        vulkano_shaders::shader! {
            ty: "fragment",
            path: "assets/shaders/toon-mesh.frag",
        }
    }

    #[derive(BufferContents, Clone, Copy, Debug, Default)]
    #[repr(C, align(16))]
    pub struct CameraUBO {
        pub view: [[f32; 4]; 4],
        pub proj: [[f32; 4]; 4],
        // std140 mat3 = 3x vec4 columns.
        pub camera2d: [[f32; 4]; 3],
        // Swapchain size in pixels (width, height). Used for aspect correction in 2D.
        pub viewport: [f32; 2],
        pub _pad0: [f32; 2],
    }

    #[derive(BufferContents, Clone, Copy, Debug, Default)]
    #[repr(C, align(16))]
    struct MaterialUBO {
        base_color: [f32; 4],
        quant_steps: f32,
        emissive: u32,
        _pad0: [u32; 2],
    }

    #[derive(
        BufferContents,
        vulkano::pipeline::graphics::vertex_input::Vertex,
        Clone,
        Copy,
        Debug,
        Default,
    )]
    #[repr(C)]
    pub struct InstanceData {
        #[format(R32G32B32A32_SFLOAT)]
        pub i_model_c0: [f32; 4],
        #[format(R32G32B32A32_SFLOAT)]
        pub i_model_c1: [f32; 4],
        #[format(R32G32B32A32_SFLOAT)]
        pub i_model_c2: [f32; 4],
        #[format(R32G32B32A32_SFLOAT)]
        pub i_model_c3: [f32; 4],
        #[format(R32G32B32A32_SFLOAT)]
        pub i_color: [f32; 4],
    }

    pub struct VulkanoGpuMesh {
        #[allow(dead_code)]
        pub vertices: Subbuffer<[CpuVertex]>,
        #[allow(dead_code)]
        pub indices: Subbuffer<[u32]>,
        #[allow(dead_code)]
        pub index_count: u32,
    }

    pub struct VulkanoGpuTexture {
        pub view: Arc<ImageView>,
    }

    pub struct VulkanoState {
        #[allow(dead_code)]
        pub context: VulkanoContext,
        #[allow(dead_code)]
        pub window: Arc<Window>,

        #[allow(dead_code)]
        pub swapchain_state: VulkanoSwapchainState,

        #[allow(dead_code)]
        pub command_buffer_allocator: Arc<StandardCommandBufferAllocator>,

        #[allow(dead_code)]
        pub descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,

        #[allow(dead_code)]
        pub set_layouts: PipelineDescriptorSetLayouts,

        #[allow(dead_code)]
        pub meshes: HashMap<MeshHandle, VulkanoGpuMesh>,

        pub textures: HashMap<TextureHandle, VulkanoGpuTexture>,
        pub sampler: Arc<Sampler>,
        pub default_white_texture: TextureHandle,

        pub pipeline_toon_mesh: Arc<GraphicsPipeline>,

        pub window_resized: bool,
        pub recreate_swapchain: bool,
        pub images_in_flight: Vec<Option<Box<dyn GpuFuture>>>,
    }

    const MAX_POINT_LIGHTS: usize = 64;

    #[derive(BufferContents, Clone, Copy, Debug, Default)]
    #[repr(C, align(16))]
    struct GpuPointLight {
        // xyz position (world), w intensity
        pos_intensity: [f32; 4],
        // rgb color, w distance
        color_distance: [f32; 4],
    }

    #[derive(BufferContents, Clone, Copy, Debug)]
    #[repr(C, align(16))]
    struct LightsSSBO {
        count: u32,
        _pad0: [u32; 3],
        lights: [GpuPointLight; MAX_POINT_LIGHTS],
    }

    impl Default for LightsSSBO {
        fn default() -> Self {
            Self {
                count: 0,
                _pad0: [0, 0, 0],
                lights: [GpuPointLight::default(); MAX_POINT_LIGHTS],
            }
        }
    }

    impl VulkanoState {
        fn create_material_ubo(material: crate::engine::graphics::MaterialHandle) -> MaterialUBO {
            match material {
                crate::engine::graphics::MaterialHandle::TOON_MESH => MaterialUBO {
                    base_color: [1.0, 0.7, 0.2, 1.0],
                    quant_steps: 4.0,
                    emissive: 0,
                    _pad0: [0, 0],
                },
                // While migrating, treat UNLIT as a simple toon material too.
                crate::engine::graphics::MaterialHandle::UNLIT_MESH => MaterialUBO {
                    base_color: [1.0, 1.0, 1.0, 1.0],
                    quant_steps: 1.0,
                    emissive: 1,
                    _pad0: [0, 0],
                },
                _ => MaterialUBO::default(),
            }
        }

        pub fn new(window: Arc<Window>) -> Result<Self, Box<dyn std::error::Error>> {
            // Prefer the helper context while we're migrating: it enables surface extensions
            // and sets up graphics/compute queues and allocators.
            let context = VulkanoContext::new(VulkanoConfig::default());
            let device = context.device().clone();

            let swapchain_state = VulkanoSwapchainState::new(&context, window.clone())?;
            let framebuffer_count = swapchain_state.framebuffers.len();

            let set_layouts = PipelineDescriptorSetLayouts::new(device.clone())?;

            let vs = toon_mesh_vs::load(device.clone())?;
            let fs = toon_mesh_fs::load(device.clone())?;

            let stages = vec![
                PipelineShaderStageCreateInfo::new(
                    vs.entry_point("main")
                        .ok_or("missing toon-mesh.vert entry point")?,
                ),
                PipelineShaderStageCreateInfo::new(
                    fs.entry_point("main")
                        .ok_or("missing toon-mesh.frag entry point")?,
                ),
            ];

            let layout = PipelineLayout::new(
                device.clone(),
                PipelineLayoutCreateInfo {
                    set_layouts: vec![set_layouts.global.clone(), set_layouts.material.clone()],
                    ..Default::default()
                },
            )?;

            // Important: `CpuVertex` contains more than just position (e.g. UV).
            // We explicitly declare which attributes are consumed by the shader.
            // Instance data occupies locations 1-4.
            let vertex_input_state = VertexInputState::new()
                .binding(
                    0,
                    VertexInputBindingDescription {
                        stride: size_of::<CpuVertex>() as u32,
                        input_rate: VertexInputRate::Vertex,
                        ..Default::default()
                    },
                )
                .binding(
                    1,
                    VertexInputBindingDescription {
                        stride: size_of::<InstanceData>() as u32,
                        input_rate: VertexInputRate::Instance { divisor: 1 },
                        ..Default::default()
                    },
                )
                .attribute(
                    0,
                    VertexInputAttributeDescription {
                        binding: 0,
                        format: Format::R32G32B32_SFLOAT,
                        offset: 0,
                        ..Default::default()
                    },
                )
                .attribute(
                    5,
                    VertexInputAttributeDescription {
                        binding: 0,
                        format: Format::R32G32_SFLOAT,
                        offset: 12,
                        ..Default::default()
                    },
                )
                .attribute(
                    1,
                    VertexInputAttributeDescription {
                        binding: 1,
                        format: Format::R32G32B32A32_SFLOAT,
                        offset: 0,
                        ..Default::default()
                    },
                )
                .attribute(
                    2,
                    VertexInputAttributeDescription {
                        binding: 1,
                        format: Format::R32G32B32A32_SFLOAT,
                        offset: 16,
                        ..Default::default()
                    },
                )
                .attribute(
                    3,
                    VertexInputAttributeDescription {
                        binding: 1,
                        format: Format::R32G32B32A32_SFLOAT,
                        offset: 32,
                        ..Default::default()
                    },
                )
                .attribute(
                    4,
                    VertexInputAttributeDescription {
                        binding: 1,
                        format: Format::R32G32B32A32_SFLOAT,
                        offset: 48,
                        ..Default::default()
                    },
                )
                .attribute(
                    6,
                    VertexInputAttributeDescription {
                        binding: 1,
                        format: Format::R32G32B32A32_SFLOAT,
                        offset: 64,
                        ..Default::default()
                    },
                );

            let subpass = Subpass::from(swapchain_state.render_pass.clone(), 0)
                .ok_or("missing subpass 0")?;
            let mut pipeline_ci =
                vulkano::pipeline::graphics::GraphicsPipelineCreateInfo::layout(layout);
            pipeline_ci.stages = stages.into();
            pipeline_ci.vertex_input_state = Some(vertex_input_state);
            pipeline_ci.input_assembly_state = Some(InputAssemblyState::default());
            pipeline_ci.viewport_state = Some(ViewportState::default());
            pipeline_ci.rasterization_state = Some(RasterizationState::default());
            pipeline_ci.multisample_state = Some(MultisampleState::default());
            // Enable depth testing so 3D geometry occludes correctly.
            pipeline_ci.depth_stencil_state = Some(DepthStencilState {
                depth: Some(DepthState::simple()),
                ..Default::default()
            });
            // Enable alpha blending so textures with transparency (e.g. PNG alpha) render correctly.
            // Uses straight alpha: out.rgb = src.rgb * src.a + dst.rgb * (1-src.a)
            pipeline_ci.color_blend_state = Some(ColorBlendState::with_attachment_states(
                1,
                ColorBlendAttachmentState {
                    blend: Some(AttachmentBlend {
                        src_color_blend_factor: BlendFactor::SrcAlpha,
                        dst_color_blend_factor: BlendFactor::OneMinusSrcAlpha,
                        color_blend_op: BlendOp::Add,
                        src_alpha_blend_factor: BlendFactor::One,
                        dst_alpha_blend_factor: BlendFactor::OneMinusSrcAlpha,
                        alpha_blend_op: BlendOp::Add,
                    }),
                    color_write_enable: true,
                    color_write_mask: ColorComponents::all(),
                },
            ));
            pipeline_ci.dynamic_state = [DynamicState::Viewport, DynamicState::Scissor]
                .into_iter()
                .collect();
            pipeline_ci.subpass = Some(PipelineSubpassType::BeginRenderPass(subpass));

            let pipeline_toon_mesh = GraphicsPipeline::new(device.clone(), None, pipeline_ci)?;

            let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(
                device.clone(),
                Default::default(),
            ));

            let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(
                device.clone(),
                Default::default(),
            ));

            let sampler = Sampler::new(device.clone(), SamplerCreateInfo::simple_repeat_linear())?;

            let mut state = Self {
                context,
                window,

                swapchain_state,

                command_buffer_allocator,
                descriptor_set_allocator,
                meshes: HashMap::new(),

                textures: HashMap::new(),
                sampler,
                default_white_texture: TextureHandle(0),

                set_layouts,

                pipeline_toon_mesh,

                window_resized: false,
                recreate_swapchain: false,
                images_in_flight: (0..framebuffer_count).map(|_| None).collect(),
            };

            // Default texture: 1x1 white so untextured materials can still bind a sampler.
            state.upload_texture_rgba8(TextureHandle(0), &[255, 255, 255, 255], 1, 1)?;

            Ok(state)
        }

        fn recreate_swapchain_if_needed(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            if !(self.window_resized || self.recreate_swapchain) {
                return Ok(());
            }

            // Swapchain recreation can race with in-flight frames during rapid resize/fullscreen
            // transitions. Ensure the GPU is idle before we rebuild swapchain-dependent
            // resources (framebuffers/depth images).
            unsafe {
                self.context
                    .device()
                    .wait_idle()
                    .map_err(|e| -> Box<dyn std::error::Error> { format!("wait_idle failed: {e}").into() })?;

                // IMPORTANT: Vulkano's internal resource tracking is tied to futures. If we drop
                // futures without telling Vulkano they've finished, it can permanently believe a
                // resource is still in use (even if the GPU is idle).
                for slot in self.images_in_flight.iter_mut() {
                    if let Some(mut fut) = slot.take() {
                        fut.signal_finished();
                        fut.cleanup_finished();
                    }
                }
            }

            self.recreate_swapchain = false;

            if let Err(e) = self.swapchain_state.recreate(&self.context, &self.window) {
                self.recreate_swapchain = true;
                println!(
                    "[VulkanoRenderer] failed to recreate swapchain: {}",
                    e
                );
                return Ok(());
            }

            // After swapchain recreation, all old swapchain images/depth attachments are gone.
            // Reset per-image in-flight tracking.
            self.images_in_flight = (0..self.swapchain_state.framebuffers.len())
                .map(|_| None)
                .collect();

            self.window_resized = false;
            Ok(())
        }

        pub fn render_visual_world(
            &mut self,
            visual_world: &mut VisualWorld,
        ) -> Result<(), Box<dyn std::error::Error>> {
            self.recreate_swapchain_if_needed()?;

            let device = self.context.device().clone();
            let queue = self.context.graphics_queue().clone();

            // Let Vulkano drop finished per-frame resources incrementally.
            for fut in self.images_in_flight.iter_mut() {
                if let Some(fut) = fut.as_mut() {
                    fut.cleanup_finished();
                }
            }

            let (image_i, suboptimal, acquire_future) =
                match swapchain::acquire_next_image(self.swapchain_state.swapchain.clone(), None)
                    .map_err(Validated::unwrap)
                {
                    Ok(r) => r,
                    Err(VulkanError::OutOfDate) => {
                        self.recreate_swapchain = true;
                        return Ok(());
                    }
                    Err(e) => return Err(Box::new(e)),
                };

            if suboptimal {
                self.recreate_swapchain = true;
            }

            // Always rebuild draw cache cheaply.
            visual_world.prepare_draw_cache();

            // Build instance buffer in draw order so each DrawBatch maps to a contiguous range.
            let instance_count = visual_world.draw_order().len();
            let instances_ref = visual_world.instances();

            let instance_data_iter = visual_world.draw_order().iter().map(|&idx| {
                let inst = instances_ref[idx as usize];
                let m = inst.transform.model;
                InstanceData {
                    i_model_c0: m[0],
                    i_model_c1: m[1],
                    i_model_c2: m[2],
                    i_model_c3: m[3],
                    i_color: inst.color,
                }
            });

            let instance_buffer: Subbuffer<[InstanceData]> = Buffer::from_iter(
                self.context.memory_allocator().clone(),
                BufferCreateInfo {
                    usage: BufferUsage::VERTEX_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                instance_data_iter,
            )?;

            let framebuffer = self.swapchain_state.framebuffers[image_i as usize].clone();
            let mut render_pass_begin = RenderPassBeginInfo::framebuffer(framebuffer);
            render_pass_begin.clear_values = VulkanoSwapchainState::clear_values();

            let extent = self.swapchain_state.swapchain.image_extent();
            let viewport = Viewport {
                offset: [0.0, 0.0],
                extent: [extent[0] as f32, extent[1] as f32],
                depth_range: 0.0..=1.0,
                ..Default::default()
            };

            // Keep VisualWorld informed of the current output size so camera systems can
            // build aspect-correct projection matrices.
            visual_world.set_viewport([extent[0] as f32, extent[1] as f32]);

            // Camera uniform buffer (set=0, binding=0).
            // `camera2d` currently feeds the 2D path directly; we also pass the current
            // swapchain extent so shaders can correct for aspect ratio.
            let camera_ubo = CameraUBO {
                view: visual_world.camera_view(),
                proj: visual_world.camera_proj(),
                camera2d: visual_world.camera_2d(),
                viewport: [extent[0] as f32, extent[1] as f32],
                _pad0: [0.0, 0.0],
            };

            let camera_buffer: Subbuffer<CameraUBO> = Buffer::from_data(
                self.context.memory_allocator().clone(),
                BufferCreateInfo {
                    usage: BufferUsage::UNIFORM_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                camera_ubo,
            )?;

            // Lights storage buffer (set=0, binding=1). Placeholder for now.
            let mut lights_ssbo = LightsSSBO::default();
            let lights = visual_world.point_lights();
            let count = (lights.len()).min(MAX_POINT_LIGHTS);
            lights_ssbo.count = count as u32;
            for (i, l) in lights.iter().take(count).enumerate() {
                lights_ssbo.lights[i] = GpuPointLight {
                    pos_intensity: [
                        l.position_ws[0],
                        l.position_ws[1],
                        l.position_ws[2],
                        l.intensity,
                    ],
                    color_distance: [l.color[0], l.color[1], l.color[2], l.distance],
                };
            }

            let lights_buffer: Subbuffer<LightsSSBO> = Buffer::from_data(
                self.context.memory_allocator().clone(),
                BufferCreateInfo {
                    usage: BufferUsage::STORAGE_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                lights_ssbo,
            )?;

            let global_set = DescriptorSet::new(
                self.descriptor_set_allocator.clone(),
                self.set_layouts.global.clone(),
                [
                    WriteDescriptorSet::buffer(0, camera_buffer),
                    WriteDescriptorSet::buffer(1, lights_buffer),
                ],
                [],
            )?;

            let mut cbb = AutoCommandBufferBuilder::primary(
                self.command_buffer_allocator.clone(),
                queue.queue_family_index(),
                CommandBufferUsage::OneTimeSubmit,
            )?;

            cbb.begin_render_pass(render_pass_begin, SubpassBeginInfo::default())?;

            cbb.set_viewport(0, vec![viewport].into())?;
            cbb.set_scissor(
                0,
                vec![Scissor {
                    offset: [0, 0],
                    extent: [extent[0], extent[1]],
                    ..Default::default()
                }]
                .into(),
            )?;

            // Bind pipeline/descriptor sets per (material, texture).
            // For now, TOON_MESH is the primary bring-up pipeline.
            // UNLIT_MESH is treated as an alias to TOON_MESH for compatibility while migrating.
            let mut bound_material: Option<crate::engine::graphics::MaterialHandle> = None;
            let mut bound_texture: Option<TextureHandle> = None;

            for batch in visual_world.draw_batches() {
                let texture_handle = batch.texture.unwrap_or(self.default_white_texture);

                if bound_material != Some(batch.material) || bound_texture != Some(texture_handle) {
                    match batch.material {
                        crate::engine::graphics::MaterialHandle::TOON_MESH
                        | crate::engine::graphics::MaterialHandle::UNLIT_MESH => {
                            let Some(tex) = self.textures.get(&texture_handle) else {
                                // Missing texture: skip this batch.
                                continue;
                            };

                            let material_ubo = Self::create_material_ubo(batch.material);
                            let material_buffer: Subbuffer<MaterialUBO> = Buffer::from_data(
                                self.context.memory_allocator().clone(),
                                BufferCreateInfo {
                                    usage: BufferUsage::UNIFORM_BUFFER,
                                    ..Default::default()
                                },
                                AllocationCreateInfo {
                                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                                    ..Default::default()
                                },
                                material_ubo,
                            )?;

                            let material_set = DescriptorSet::new(
                                self.descriptor_set_allocator.clone(),
                                self.set_layouts.material.clone(),
                                [
                                    WriteDescriptorSet::buffer(0, material_buffer),
                                    WriteDescriptorSet::image_view_sampler(
                                        1,
                                        tex.view.clone(),
                                        self.sampler.clone(),
                                    ),
                                ],
                                [],
                            )?;

                            cbb.bind_pipeline_graphics(self.pipeline_toon_mesh.clone())?;
                            cbb.bind_descriptor_sets(
                                PipelineBindPoint::Graphics,
                                self.pipeline_toon_mesh.layout().clone(),
                                0,
                                (global_set.clone(), material_set),
                            )?;
                        }
                        _ => {
                            // Unknown material: skip this batch.
                            continue;
                        }
                    }

                    bound_material = Some(batch.material);
                    bound_texture = Some(texture_handle);
                }

                let Some(mesh) = self.meshes.get(&batch.mesh) else {
                    continue;
                };
                cbb.bind_vertex_buffers(0, (mesh.vertices.clone(), instance_buffer.clone()))?;
                cbb.bind_index_buffer(mesh.indices.clone())?;

                if instance_count > 0 {
                    unsafe {
                        cbb.draw_indexed(
                            mesh.index_count,
                            batch.count as u32,
                            0,
                            0,
                            batch.start as u32,
                        )?;
                    }
                }
            }

            cbb.end_render_pass(SubpassEndInfo::default())?;

            let cb = cbb.build()?;

            // Ensure we never render into a swapchain image (and its paired depth attachment)
            // while a previous frame that used that image is still in flight.
            let image_i_usize = image_i as usize;
            if self.images_in_flight.len() != self.swapchain_state.framebuffers.len() {
                // Defensive: should only happen if swapchain recreation failed partially.
                self.images_in_flight = (0..self.swapchain_state.framebuffers.len())
                    .map(|_| None)
                    .collect();
            }

            let image_future: Box<dyn GpuFuture> = self.images_in_flight[image_i_usize]
                .take()
                .unwrap_or_else(|| sync::now(device.clone()).boxed());

            let execution = image_future
                .join(acquire_future)
                .then_execute(queue.clone(), cb)?
                .then_swapchain_present(
                    queue.clone(),
                    SwapchainPresentInfo::swapchain_image_index(
                        self.swapchain_state.swapchain.clone(),
                        image_i,
                    ),
                )
                .then_signal_fence_and_flush();

            match execution.map_err(Validated::unwrap) {
                Ok(future) => {
                    // Track this swapchain image as in flight; this also keeps per-frame
                    // resources alive until the GPU is done.
                    self.images_in_flight[image_i_usize] = Some(future.boxed());
                }
                Err(VulkanError::OutOfDate) => {
                    self.recreate_swapchain = true;
                    // During resize/out-of-date thrash, the command buffer may have been
                    // submitted but we might not get a usable future back. Ensure the GPU
                    // is idle before we drop tracking; otherwise resources (e.g. depth
                    // attachments) can appear "already in use" next frame.
                    unsafe {
                        println!("[VulkanoRenderer] swapchain out of date during flush");
                        let _ = device.wait_idle();
                        for slot in self.images_in_flight.iter_mut() {
                            if let Some(mut fut) = slot.take() {
                                fut.signal_finished();
                                fut.cleanup_finished();
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("[VulkanoRenderer] failed to flush future: {e}");

                    unsafe {
                        println!("[VulkanoRenderer] waiting for device idle after flush failure");
                        let _ = device.wait_idle();
                        for slot in self.images_in_flight.iter_mut() {
                            if let Some(mut fut) = slot.take() {
                                fut.signal_finished();
                                fut.cleanup_finished();
                            }
                        }
                    }
                }
            }

            Ok(())
        }

        pub fn upload_texture_rgba8(
            &mut self,
            handle: TextureHandle,
            rgba: &[u8],
            width: u32,
            height: u32,
        ) -> Result<(), Box<dyn std::error::Error>> {
            if self.textures.contains_key(&handle) {
                return Ok(());
            }

            if width == 0 || height == 0 {
                return Err("texture has zero size".into());
            }

            let expected_len = width as usize * height as usize * 4;
            if rgba.len() != expected_len {
                return Err(format!(
                    "texture rgba length mismatch: got={}, expected={}",
                    rgba.len(),
                    expected_len
                )
                .into());
            }

            let memory_allocator = self.context.memory_allocator().clone();
            let queue = self.context.graphics_queue().clone();

            let staging = Buffer::from_iter(
                memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::TRANSFER_SRC,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                rgba.iter().copied(),
            )?;

            let image = Image::new(
                memory_allocator,
                ImageCreateInfo {
                    image_type: ImageType::Dim2d,
                    format: Format::R8G8B8A8_UNORM,
                    extent: [width, height, 1],
                    usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                    ..Default::default()
                },
            )?;

            let mut cbb = AutoCommandBufferBuilder::primary(
                self.command_buffer_allocator.clone(),
                queue.queue_family_index(),
                CommandBufferUsage::OneTimeSubmit,
            )?;

            cbb.copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(staging, image.clone()))?;

            let cb = cbb.build()?;

            cb.execute(queue.clone())?
                .then_signal_fence_and_flush()?
                .wait(None)?;

            let view = ImageView::new_default(image)
                .map_err(|e| -> Box<dyn std::error::Error> { format!("{e:?}").into() })?;
            self.textures.insert(handle, VulkanoGpuTexture { view });
            Ok(())
        }

        pub fn upload_texture_bc7(
            &mut self,
            handle: TextureHandle,
            bc7_blocks: &[u8],
            width: u32,
            height: u32,
            srgb: bool,
        ) -> Result<(), Box<dyn std::error::Error>> {
            if self.textures.contains_key(&handle) {
                return Ok(());
            }

            if width == 0 || height == 0 {
                return Err("texture has zero size".into());
            }

            let blocks_w = (width + 3) / 4;
            let blocks_h = (height + 3) / 4;
            let expected_len = blocks_w as usize * blocks_h as usize * 16;
            if bc7_blocks.len() != expected_len {
                return Err(format!(
                    "texture bc7 length mismatch: got={}, expected={}",
                    bc7_blocks.len(),
                    expected_len
                )
                .into());
            }

            let memory_allocator = self.context.memory_allocator().clone();
            let queue = self.context.graphics_queue().clone();

            let staging = Buffer::from_iter(
                memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::TRANSFER_SRC,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                bc7_blocks.iter().copied(),
            )?;

            let format = if srgb {
                Format::BC7_SRGB_BLOCK
            } else {
                Format::BC7_UNORM_BLOCK
            };

            let image = Image::new(
                memory_allocator,
                ImageCreateInfo {
                    image_type: ImageType::Dim2d,
                    format,
                    extent: [width, height, 1],
                    usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                    ..Default::default()
                },
            )?;

            let mut cbb = AutoCommandBufferBuilder::primary(
                self.command_buffer_allocator.clone(),
                queue.queue_family_index(),
                CommandBufferUsage::OneTimeSubmit,
            )?;

            cbb.copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(staging, image.clone()))?;

            let cb = cbb.build()?;

            cb.execute(queue.clone())?
                .then_signal_fence_and_flush()?
                .wait(None)?;

            let view = ImageView::new_default(image)
                .map_err(|e| -> Box<dyn std::error::Error> { format!("{e:?}").into() })?;
            self.textures.insert(handle, VulkanoGpuTexture { view });
            Ok(())
        }

        pub fn upload_mesh(
            &mut self,
            handle: MeshHandle,
            mesh: &CpuMesh,
        ) -> Result<(), Box<dyn std::error::Error>> {
            if self.meshes.contains_key(&handle) {
                return Ok(());
            }

            if mesh.vertices.is_empty() {
                return Err("mesh has no vertices".into());
            }
            if mesh.indices_u32.is_empty() {
                return Err("mesh has no indices".into());
            }

            let memory_allocator = self.context.memory_allocator().clone();
            let queue = self.context.graphics_queue().clone();

            // Host-visible staging buffers.
            let vertices_src = Buffer::from_iter(
                memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::TRANSFER_SRC,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                mesh.vertices.iter().copied(),
            )?;

            let indices_src = Buffer::from_iter(
                memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::TRANSFER_SRC,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                mesh.indices_u32.iter().copied(),
            )?;

            // Device-local destination buffers.
            let vertices_dst = Buffer::new_slice::<CpuVertex>(
                memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::VERTEX_BUFFER | BufferUsage::TRANSFER_DST,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                    ..Default::default()
                },
                mesh.vertices.len() as DeviceSize,
            )?;

            let indices_dst = Buffer::new_slice::<u32>(
                memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::INDEX_BUFFER | BufferUsage::TRANSFER_DST,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                    ..Default::default()
                },
                mesh.indices_u32.len() as DeviceSize,
            )?;

            // Copy staging -> device-local.
            let mut cbb = AutoCommandBufferBuilder::primary(
                self.command_buffer_allocator.clone(),
                queue.queue_family_index(),
                CommandBufferUsage::OneTimeSubmit,
            )?;

            cbb.copy_buffer(CopyBufferInfo::buffers(vertices_src, vertices_dst.clone()))?;
            cbb.copy_buffer(CopyBufferInfo::buffers(indices_src, indices_dst.clone()))?;

            let cb = cbb.build()?;

            cb.execute(queue.clone())?
                .then_signal_fence_and_flush()?
                .wait(None)?;

            self.meshes.insert(
                handle,
                VulkanoGpuMesh {
                    vertices: vertices_dst,
                    indices: indices_dst,
                    index_count: mesh.index_count(),
                },
            );

            Ok(())
        }
    }
}

/// Vulkano-only renderer.
pub struct VulkanoRenderer {
    vulkano: Option<vulkano_backend::VulkanoState>,
    next_mesh_handle: u32,
    next_texture_handle: u32,
    did_enable_present_loop_log: bool,
}

impl VulkanoRenderer {
    pub fn new() -> Self {
        Self {
            vulkano: None,
            next_mesh_handle: 0,
            // Reserve handle 0 for the default white texture.
            next_texture_handle: 1,
            did_enable_present_loop_log: false,
        }
    }

    pub fn init_for_window(
        &mut self,
        window: &Arc<Window>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.vulkano.is_none() {
            self.vulkano = Some(vulkano_backend::VulkanoState::new(window.clone())?);
            println!("[VulkanoRenderer] Vulkano swapchain/render-pass initialized");
        }

        Ok(())
    }

    pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        let _ = size;
        if let Some(vulkano) = self.vulkano.as_mut() {
            vulkano.window_resized = true;
        }
    }

    pub fn upload_mesh(
        &mut self,
        mesh: &CpuMesh,
    ) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        let Some(vulkano) = self.vulkano.as_mut() else {
            return Err("VulkanoRenderer not initialized (call init_for_window first)".into());
        };

        let handle = MeshHandle(self.next_mesh_handle);
        self.next_mesh_handle = self.next_mesh_handle.wrapping_add(1);

        vulkano.upload_mesh(handle, mesh)?;
        Ok(handle)
    }

    pub fn render_visual_world(
        &mut self,
        visual_world: &mut VisualWorld,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(vulkano) = self.vulkano.as_mut() else {
            return Err("VulkanoRenderer not initialized (call init_for_window first)".into());
        };

        if !self.did_enable_present_loop_log {
            self.did_enable_present_loop_log = true;
            println!("[VulkanoRenderer] Present loop enabled");
        }

        vulkano.render_visual_world(visual_world)
    }
}

impl MeshUploader for VulkanoRenderer {
    fn upload_mesh(&mut self, mesh: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        self.upload_mesh(mesh)
    }
}

impl TextureUploader for VulkanoRenderer {
    fn upload_texture_rgba8(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        let Some(vulkano) = self.vulkano.as_mut() else {
            return Err("VulkanoRenderer not initialized (call init_for_window first)".into());
        };

        let handle = TextureHandle(self.next_texture_handle);
        self.next_texture_handle = self.next_texture_handle.wrapping_add(1);

        vulkano.upload_texture_rgba8(handle, rgba, width, height)?;
        Ok(handle)
    }

    fn upload_texture_bc7(
        &mut self,
        bc7_blocks: &[u8],
        width: u32,
        height: u32,
        srgb: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        let Some(vulkano) = self.vulkano.as_mut() else {
            return Err("VulkanoRenderer not initialized (call init_for_window first)".into());
        };

        let handle = TextureHandle(self.next_texture_handle);
        self.next_texture_handle = self.next_texture_handle.wrapping_add(1);

        vulkano.upload_texture_bc7(handle, bc7_blocks, width, height, srgb)?;
        Ok(handle)
    }
}
