//! Compute-cached skinning and morph deformation for the mobile renderer.
//!
//! Ports the desktop contract in `engine::graphics::deformation` and reuses
//! `assets/shaders/mesh-deformation.comp`: immutable per-mesh input arenas, the
//! engine's shared bone palette, and at most one dispatch per frame for dirty
//! instances. Output persists between frames and is read by both eye passes.
//! The output is ordinary storage memory, not tile memory.
use super::resources::{Buffer, bytes};
use crate::{
    graphics_device::Gpu,
    xr_host::{ApiResult, Result},
};
use ash::vk;
use mittens_engine::engine::graphics::{
    CpuMesh, MeshHandle,
    deformation::{
        GpuActiveMorph, GpuBaseDeformationVertex, GpuDeformationJob, GpuDeformationSkinVertex,
        GpuDeformationWorkgroup, GpuDeformedVertex, GpuMorphDelta, build_workgroups,
    },
    visual_world::VisualWorld,
};
use std::{
    collections::HashMap,
    io::Cursor,
    mem::{size_of, size_of_val},
    sync::Arc,
};

/// Where a skinned mesh's immutable inputs live in the shared arenas.
#[derive(Clone, Copy)]
struct SkinnedMesh {
    base_vertex: u32,
    skin_vertex: u32,
    morph_delta: Option<u32>,
    morph_targets: u32,
    vertex_count: u32,
    /// Highest joint index any vertex uses; instances must supply more bones.
    max_joint: u32,
}

const STORAGE: vk::BufferUsageFlags = vk::BufferUsageFlags::STORAGE_BUFFER;

/// Replaces `buffer` when `needed` bytes do not fit. Returns whether it did.
fn ensure(gpu: &Arc<Gpu>, buffer: &mut Buffer, needed: usize) -> Result<bool> {
    if needed <= buffer.capacity {
        return Ok(false);
    }
    *buffer = Buffer::new(gpu.clone(), needed.next_power_of_two(), STORAGE)?;
    Ok(true)
}

pub(super) struct Deformation {
    gpu: Arc<Gpu>,
    meshes: HashMap<MeshHandle, SkinnedMesh>,
    vertex_counts: HashMap<MeshHandle, u32>,
    base_cpu: Vec<GpuBaseDeformationVertex>,
    skin_cpu: Vec<GpuDeformationSkinVertex>,
    morph_cpu: Vec<GpuMorphDelta>,
    arenas_dirty: bool,
    jobs_cpu: Vec<GpuDeformationJob>,
    active_cpu: Vec<GpuActiveMorph>,
    base: Buffer,
    skin: Buffer,
    morph: Buffer,
    bones: Buffer,
    active_morphs: Buffer,
    jobs: Buffer,
    workgroups: Buffer,
    /// `GpuDeformedVertex` records indexed by each instance's `deformed_base`.
    pub output: Buffer,
    set_layout: vk::DescriptorSetLayout,
    layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    max_dispatch: u32,
    pending_workgroups: u32,
    pub jobs_last_frame: usize,
}
impl Drop for Deformation {
    fn drop(&mut self) {
        unsafe {
            let device = &self.gpu.device;
            device.destroy_pipeline(self.pipeline, None);
            device.destroy_pipeline_layout(self.layout, None);
            device.destroy_descriptor_pool(self.pool, None);
            device.destroy_descriptor_set_layout(self.set_layout, None);
        }
    }
}
impl Deformation {
    pub fn new(gpu: Arc<Gpu>) -> Result<Self> {
        // Descriptors cannot reference empty buffers; start with one record each.
        let empty = |size: usize| Buffer::new(gpu.clone(), size, STORAGE);
        let base = empty(size_of::<GpuBaseDeformationVertex>())?;
        let skin = empty(size_of::<GpuDeformationSkinVertex>())?;
        let morph = empty(size_of::<GpuMorphDelta>())?;
        let bones = empty(64)?;
        let active_morphs = empty(size_of::<GpuActiveMorph>())?;
        let jobs = empty(size_of::<GpuDeformationJob>())?;
        let workgroups = empty(size_of::<GpuDeformationWorkgroup>())?;
        let output = empty(size_of::<GpuDeformedVertex>())?;
        let max_dispatch = unsafe {
            gpu.instance
                .raw
                .get_physical_device_properties(gpu.physical)
                .limits
                .max_compute_work_group_count[0]
        };
        let mut deformation = Self {
            meshes: HashMap::new(),
            vertex_counts: HashMap::new(),
            base_cpu: Vec::new(),
            skin_cpu: Vec::new(),
            morph_cpu: Vec::new(),
            arenas_dirty: false,
            jobs_cpu: Vec::new(),
            active_cpu: Vec::new(),
            base,
            skin,
            morph,
            bones,
            active_morphs,
            jobs,
            workgroups,
            output,
            set_layout: vk::DescriptorSetLayout::null(),
            layout: vk::PipelineLayout::null(),
            pipeline: vk::Pipeline::null(),
            pool: vk::DescriptorPool::null(),
            set: vk::DescriptorSet::null(),
            max_dispatch,
            pending_workgroups: 0,
            jobs_last_frame: 0,
            gpu,
        };
        unsafe {
            let device = &deformation.gpu.device;
            let bindings: Vec<_> = (0..8)
                .map(|binding| {
                    vk::DescriptorSetLayoutBinding::default()
                        .binding(binding)
                        .descriptor_count(1)
                        .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                        .stage_flags(vk::ShaderStageFlags::COMPUTE)
                })
                .collect();
            deformation.set_layout = device
                .create_descriptor_set_layout(
                    &vk::DescriptorSetLayoutCreateInfo::default().bindings(&bindings),
                    None,
                )
                .api("deformation descriptor layout")?;
            let push = [vk::PushConstantRange::default()
                .stage_flags(vk::ShaderStageFlags::COMPUTE)
                .size(4)];
            deformation.layout = device
                .create_pipeline_layout(
                    &vk::PipelineLayoutCreateInfo::default()
                        .set_layouts(&[deformation.set_layout])
                        .push_constant_ranges(&push),
                    None,
                )
                .api("deformation pipeline layout")?;
            let words = ash::util::read_spv(&mut Cursor::new(include_bytes!(concat!(
                env!("OUT_DIR"),
                "/mesh-deformation.comp.spv"
            ))))
            .api("read deformation SPIR-V")?;
            let module = device
                .create_shader_module(&vk::ShaderModuleCreateInfo::default().code(&words), None)
                .api("vkCreateShaderModule(deformation)")?;
            let created = device.create_compute_pipelines(
                vk::PipelineCache::null(),
                &[vk::ComputePipelineCreateInfo::default()
                    .stage(
                        vk::PipelineShaderStageCreateInfo::default()
                            .stage(vk::ShaderStageFlags::COMPUTE)
                            .module(module)
                            .name(c"main"),
                    )
                    .layout(deformation.layout)],
                None,
            );
            device.destroy_shader_module(module, None);
            deformation.pipeline = created
                .map_err(|(_, error)| format!("vkCreateComputePipelines(deformation): {error:?}"))?
                [0];
            let sizes = [vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_BUFFER,
                descriptor_count: 8,
            }];
            deformation.pool = device
                .create_descriptor_pool(
                    &vk::DescriptorPoolCreateInfo::default()
                        .max_sets(1)
                        .pool_sizes(&sizes),
                    None,
                )
                .api("deformation descriptor pool")?;
            deformation.set = device
                .allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::default()
                        .descriptor_pool(deformation.pool)
                        .set_layouts(&[deformation.set_layout]),
                )
                .api("allocate deformation descriptors")?[0];
        }
        Ok(deformation)
    }

    /// Records a skinned mesh's inputs. Returns false for an ordinary mesh.
    /// Arenas are append-only; a renderer is rebuilt per launched scene.
    pub fn upload(&mut self, handle: MeshHandle, mesh: &CpuMesh) -> Result<bool> {
        let (joints, weights) = match (&mesh.joints0, &mesh.weights0) {
            (None, None) if mesh.morph_targets.is_empty() => return Ok(false),
            (None, None) => {
                return Err("morph targets without a skin are unsupported by the mobile renderer".into());
            }
            (Some(joints), Some(weights))
                if joints.len() == mesh.vertices.len() && weights.len() == mesh.vertices.len() =>
            {
                (joints, weights)
            }
            _ => return Err("mesh has incomplete or mismatched JOINTS_0/WEIGHTS_0".into()),
        };
        let vertex_count = u32::try_from(mesh.vertices.len()).map_err(|e| e.to_string())?;
        let base_vertex = u32::try_from(self.base_cpu.len()).map_err(|e| e.to_string())?;
        let skin_vertex = u32::try_from(self.skin_cpu.len()).map_err(|e| e.to_string())?;
        let mut max_joint = 0;
        for (index, ((vertex, joints), weights)) in
            mesh.vertices.iter().zip(joints).zip(weights).enumerate()
        {
            if !vertex
                .pos
                .iter()
                .chain(&vertex.normal)
                .chain(weights)
                .all(|value| value.is_finite())
            {
                return Err(format!(
                    "skinned mesh vertex {index} contains non-finite deformation input"
                ));
            }
            max_joint = joints.iter().fold(max_joint, |max, &j| max.max(u32::from(j)));
            self.base_cpu.push(GpuBaseDeformationVertex {
                position: [vertex.pos[0], vertex.pos[1], vertex.pos[2], 1.0],
                normal: [vertex.normal[0], vertex.normal[1], vertex.normal[2], 0.0],
            });
            self.skin_cpu.push(GpuDeformationSkinVertex {
                joints: joints.map(u32::from),
                weights: *weights,
            });
        }
        let morph_delta = if mesh.morph_targets.is_empty() {
            None
        } else {
            let base = u32::try_from(self.morph_cpu.len()).map_err(|e| e.to_string())?;
            for target in &mesh.morph_targets {
                if target.position_deltas.len() != mesh.vertices.len()
                    || target.normal_deltas.len() != mesh.vertices.len()
                {
                    return Err("morph target does not have one delta per vertex".into());
                }
                for (position, normal) in target.position_deltas.iter().zip(&target.normal_deltas) {
                    self.morph_cpu.push(GpuMorphDelta {
                        position_delta: [position[0], position[1], position[2], 0.0],
                        normal_delta: [normal[0], normal[1], normal[2], 0.0],
                    });
                }
            }
            Some(base)
        };
        self.meshes.insert(
            handle,
            SkinnedMesh {
                base_vertex,
                skin_vertex,
                morph_delta,
                morph_targets: mesh.morph_targets.len() as u32,
                vertex_count,
                max_joint,
            },
        );
        self.vertex_counts.insert(handle, vertex_count);
        self.arenas_dirty = true;
        Ok(true)
    }

    pub fn release(&mut self, handle: MeshHandle) {
        self.meshes.remove(&handle);
        self.vertex_counts.remove(&handle);
    }

    pub fn is_skinned(&self, handle: MeshHandle) -> bool {
        self.meshes.contains_key(&handle)
    }

    /// Uploads changed inputs and builds this frame's jobs for dirty
    /// instances. Returns true when the output buffer was replaced, so the
    /// vertex-stage descriptors must be rewritten.
    pub fn prepare(&mut self, visuals: &mut VisualWorld) -> Result<bool> {
        self.pending_workgroups = 0;
        self.jobs_last_frame = 0;
        let dirty_bones = visuals.take_dirty_bones_ranges();
        if self.meshes.is_empty() && visuals.deformation_cache_capacity() == 0 {
            return Ok(false);
        }
        visuals.sync_deformation_ranges(&self.vertex_counts);
        let mut replaced = false;
        let wanted = visuals.deformation_cache_capacity().max(1) as usize;
        if ensure(&self.gpu, &mut self.output, wanted * size_of::<GpuDeformedVertex>())? {
            // Recompute every live range rather than copying the old output.
            visuals.mark_all_deformations_dirty();
            replaced = true;
        }
        if self.arenas_dirty {
            ensure(&self.gpu, &mut self.base, size_of_val(self.base_cpu.as_slice()))?;
            ensure(&self.gpu, &mut self.skin, size_of_val(self.skin_cpu.as_slice()))?;
            ensure(&self.gpu, &mut self.morph, size_of_val(self.morph_cpu.as_slice()))?;
            unsafe {
                self.base.write(bytes(&self.base_cpu))?;
                self.skin.write(bytes(&self.skin_cpu))?;
                self.morph.write(bytes(&self.morph_cpu))?;
            }
            self.arenas_dirty = false;
        }
        let palette = visuals.bones_palette();
        if ensure(&self.gpu, &mut self.bones, size_of_val(palette))? {
            unsafe { self.bones.write(bytes(palette))? };
        } else {
            for range in dirty_bones {
                let start = range.base as usize;
                let end = start + range.vertex_count as usize;
                let matrices = palette
                    .get(start..end)
                    .ok_or("dirty bones interval is outside the palette")?;
                unsafe { self.bones.write_at(start * 64, bytes(matrices))? };
            }
        }

        self.jobs_cpu.clear();
        self.active_cpu.clear();
        for (index, instance) in visuals.instances().iter().enumerate() {
            if !instance.deformation_dirty || instance.deformed_count == 0 {
                continue;
            }
            let Some(mesh) = self.meshes.get(&instance.renderable.mesh) else {
                continue;
            };
            if mesh.max_joint >= instance.bones_count {
                return Err(format!(
                    "skinned mesh uses joint {} but its instance supplies {} bones",
                    mesh.max_joint, instance.bones_count
                ));
            }
            let active_morph_base = self.active_cpu.len() as u32;
            if let Some(delta_base) = mesh.morph_delta {
                for &(target, weight) in visuals.active_morphs(index) {
                    if target < mesh.morph_targets && weight != 0.0 {
                        self.active_cpu.push(GpuActiveMorph {
                            delta_base: delta_base + target * mesh.vertex_count,
                            weight,
                        });
                    }
                }
            }
            self.jobs_cpu.push(GpuDeformationJob {
                base_vertex: mesh.base_vertex,
                skin_vertex: mesh.skin_vertex,
                output_vertex: instance.deformed_base,
                vertex_count: instance.deformed_count,
                bones_base: instance.bones_base,
                bones_count: instance.bones_count,
                active_morph_base,
                active_morph_count: self.active_cpu.len() as u32 - active_morph_base,
            });
        }
        // The dispatch is recorded into this frame's submission, which the
        // renderer completes before the next prepare.
        visuals.mark_deformations_clean();
        if self.jobs_cpu.is_empty() {
            return Ok(replaced);
        }
        let workgroups = build_workgroups(&self.jobs_cpu);
        ensure(&self.gpu, &mut self.jobs, size_of_val(self.jobs_cpu.as_slice()))?;
        ensure(&self.gpu, &mut self.workgroups, size_of_val(workgroups.as_slice()))?;
        ensure(&self.gpu, &mut self.active_morphs, size_of_val(self.active_cpu.as_slice()))?;
        unsafe {
            self.jobs.write(bytes(&self.jobs_cpu))?;
            self.workgroups.write(bytes(&workgroups))?;
            self.active_morphs.write(bytes(&self.active_cpu))?;
        }
        self.write_descriptors();
        self.pending_workgroups = workgroups.len() as u32;
        self.jobs_last_frame = self.jobs_cpu.len();
        Ok(replaced)
    }

    fn write_descriptors(&self) {
        let buffers = [
            &self.base,
            &self.skin,
            &self.bones,
            &self.morph,
            &self.active_morphs,
            &self.jobs,
            &self.workgroups,
            &self.output,
        ];
        let infos: Vec<_> = buffers
            .iter()
            .map(|buffer| {
                [vk::DescriptorBufferInfo::default()
                    .buffer(buffer.handle)
                    .range(vk::WHOLE_SIZE)]
            })
            .collect();
        let writes: Vec<_> = infos
            .iter()
            .enumerate()
            .map(|(binding, info)| {
                vk::WriteDescriptorSet::default()
                    .dst_set(self.set)
                    .dst_binding(binding as u32)
                    .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
                    .buffer_info(info)
            })
            .collect();
        unsafe { self.gpu.device.update_descriptor_sets(&writes, &[]) };
    }

    /// Records the dispatch, if any, before the eye passes, and makes its
    /// output visible to their vertex shaders.
    pub fn record(&self, command: vk::CommandBuffer) {
        if self.pending_workgroups == 0 {
            return;
        }
        let device = &self.gpu.device;
        unsafe {
            device.cmd_bind_pipeline(command, vk::PipelineBindPoint::COMPUTE, self.pipeline);
            device.cmd_bind_descriptor_sets(
                command,
                vk::PipelineBindPoint::COMPUTE,
                self.layout,
                0,
                &[self.set],
                &[],
            );
            let mut first = 0;
            while first < self.pending_workgroups {
                let count = (self.pending_workgroups - first).min(self.max_dispatch);
                device.cmd_push_constants(
                    command,
                    self.layout,
                    vk::ShaderStageFlags::COMPUTE,
                    0,
                    &first.to_ne_bytes(),
                );
                device.cmd_dispatch(command, count, 1, 1);
                first += count;
            }
            device.cmd_pipeline_barrier(
                command,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::VERTEX_SHADER,
                vk::DependencyFlags::empty(),
                &[vk::MemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ)],
                &[],
                &[],
            );
        }
    }
}
