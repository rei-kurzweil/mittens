// Dynamic bone uploads, deformation storage and compute recording.
impl VulkanoState {
    // Return an open builder: first-use/fallback views append graphics to it,
    // while retained views finalize this as their small dynamic primary.
    fn record_deformation_commands(
        &mut self,
        visual_world: &VisualWorld,
        dirty_bones: &[crate::engine::graphics::deformation::DeformationRange],
        #[cfg(test)] eye: usize,
        #[cfg(test)] cpu_stage_clock: &mut Option<std::time::Instant>,
    ) -> Result<
        (
            AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
            Arc<DescriptorSet>,
        ),
        Box<dyn std::error::Error>,
    > {
        let mut cbb = AutoCommandBufferBuilder::primary(
            self.command_buffer_allocator.clone(),
            self.context.graphics_queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )?;
        profile_cpu_stage!(self, cpu_stage_clock, "builder_create");
        #[cfg(test)]
        if let Some(profile) = self.profile.as_ref() {
            profile.timestamp(&mut cbb, eye, 0)?;
        }
        #[cfg(test)]
        let deform_started = self.profile.as_ref().map(|_| std::time::Instant::now());
        let output = self.record_dirty_deformations(&mut cbb, visual_world, dirty_bones)?;
        #[cfg(test)]
        if let Some(profile) = self.profile.as_mut() {
            profile.timestamp(&mut cbb, eye, 1)?;
            profile.deformation_cpu_ns += deform_started.unwrap().elapsed().as_nanos() as u64;
        }
        profile_cpu_stage!(self, cpu_stage_clock, "deformation_record");
        let rig_set = self.retained_rig_set(output)?;
        profile_cpu_stage!(self, cpu_stage_clock, "rig_buffer_descriptor");
        Ok((cbb, rig_set))
    }
    fn record_dirty_deformations(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        visual_world: &VisualWorld,
        dirty_bones: &[crate::engine::graphics::deformation::DeformationRange],
    ) -> Result<Subbuffer<[GpuDeformedVertex]>, Box<dyn std::error::Error>> {
        let memory_allocator = self.context.memory_allocator().clone();
        let wanted_capacity = visual_world.deformation_cache_capacity().max(1);
        if self.deformation_output_capacity < wanted_capacity {
            let new_capacity = wanted_capacity.next_power_of_two();
            let new_output = Buffer::new_slice::<GpuDeformedVertex>(
                memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::STORAGE_BUFFER
                        | BufferUsage::TRANSFER_SRC
                        | BufferUsage::TRANSFER_DST,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                    ..Default::default()
                },
                new_capacity as DeviceSize,
            )?;
            if let Some(old_output) = self.deformation_output_buffer.as_ref() {
                cbb.copy_buffer(CopyBufferInfo::buffers(
                    old_output.clone(),
                    new_output.clone(),
                ))?;
            }
            self.deformation_output_buffer = Some(new_output);
            self.deformation_output_capacity = new_capacity;
            self.deformation_stats.resizes += 1;
            self.deformation_stats.buffer_allocations += 1;
        }
        let output = self
            .deformation_output_buffer
            .as_ref()
            .expect("deformation output allocated")
            .clone();

        let bones_len = visual_world.bones_palette().len().max(1);
        let bones_reallocated = self
            .deformation_bones_buffer
            .as_ref()
            .is_none_or(|buffer| buffer.len() < bones_len as DeviceSize);
        if bones_reallocated {
            self.deformation_stats.bone_resizes += 1;
            self.deformation_stats.buffer_allocations += 1;
            self.deformation_bones_buffer = Some(Buffer::new_slice::<GpuMat4>(
                memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::STORAGE_BUFFER | BufferUsage::TRANSFER_DST,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                    ..Default::default()
                },
                bones_len.next_power_of_two() as DeviceSize,
            )?);
        }
        let bones_buffer = self
            .deformation_bones_buffer
            .as_ref()
            .expect("deformation bones allocated")
            .clone();
        let upload_ranges: Vec<_> = if bones_reallocated {
            vec![crate::engine::graphics::deformation::DeformationRange {
                base: 0,
                vertex_count: bones_len as u32,
            }]
        } else {
            dirty_bones.to_vec()
        };
        for range in upload_ranges {
            let start = range.base as usize;
            let end = start + range.vertex_count as usize;
            if end > visual_world.bones_palette().len() {
                return Err("dirty bones interval is outside the palette".into());
            }
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
                visual_world.bones_palette()[start..end]
                    .iter()
                    .copied()
                    .map(|cols| GpuMat4 { cols }),
            )?;
            cbb.copy_buffer(CopyBufferInfo::buffers(
                staging,
                bones_buffer
                    .clone()
                    .slice(start as DeviceSize..end as DeviceSize),
            ))?;
            self.deformation_stats.buffer_allocations += 1;
            self.deformation_stats.bone_upload_bytes +=
                (range.vertex_count as usize * size_of::<GpuMat4>()) as u64;
        }

        let mut jobs = Vec::new();
        let mut active_morphs = Vec::new();
        for (instance_index, instance) in visual_world.instances().iter().enumerate() {
            if !instance.deformation_dirty || instance.deformed_count == 0 {
                continue;
            }
            let Some(mesh) = self.meshes.get(&instance.renderable.mesh) else {
                continue;
            };
            let (Some(base_vertex), Some(skin_vertex)) =
                (mesh.deformation_base, mesh.deformation_skin_base)
            else {
                continue;
            };
            let active_morph_base = active_morphs.len() as u32;
            if let Some(morph_delta_base) = mesh.morph_delta_base {
                for &(target_index, weight) in visual_world.active_morphs(instance_index) {
                    if (target_index as usize)
                        < (self.deformation_morph_cpu.len() / mesh.vertex_count as usize)
                    {
                        active_morphs.push(GpuActiveMorph {
                            delta_base: morph_delta_base + target_index * mesh.vertex_count,
                            weight,
                        });
                    }
                }
            }
            let skin_start = skin_vertex as usize;
            let skin_end = skin_start + mesh.vertex_count as usize;
            if self.deformation_skin_cpu[skin_start..skin_end]
                .iter()
                .flat_map(|vertex| vertex.joints)
                .any(|joint| joint >= instance.bones_count)
            {
                return Err(format!(
                    "mesh {:?} contains a joint index outside instance palette count {}",
                    instance.renderable.mesh, instance.bones_count
                )
                .into());
            }
            #[cfg(test)]
            if let Some(profile) = self.profile.as_mut() {
                profile.dirty_components.push(instance.component);
            }
            jobs.push(GpuDeformationJob {
                base_vertex,
                skin_vertex,
                output_vertex: instance.deformed_base,
                vertex_count: instance.deformed_count,
                bones_base: instance.bones_base,
                bones_count: instance.bones_count,
                active_morph_base,
                active_morph_count: active_morphs.len() as u32 - active_morph_base,
            });
        }
        let workgroups = build_workgroups(&jobs);
        if jobs.is_empty() {
            self.deformation_stats.live_cache_bytes = visual_world.deformation_cache_live_vertices()
                as u64
                * size_of::<GpuDeformedVertex>() as u64;
            self.deformation_stats.allocated_cache_bytes =
                self.deformation_output_capacity as u64 * size_of::<GpuDeformedVertex>() as u64;
            return Ok(output);
        }

        let base_buffer = self
            .deformation_base_buffer
            .as_ref()
            .ok_or("missing global base deformation arena")?
            .clone();
        let skin_buffer = self
            .deformation_skin_buffer
            .as_ref()
            .ok_or("missing global skin deformation arena")?
            .clone();
        // Mesh loading uploads the immutable global morph arena and replaces
        // it only when a new mesh extends it. Descriptor sets/recorded command
        // buffers retain the old allocation across replacement.
        let morph_buffer = if let Some(buffer) = &self.deformation_morph_buffer {
            buffer.clone()
        } else {
            if !self.deformation_morph_cpu.is_empty() {
                return Err("missing uploaded global morph deformation arena".into());
            }
            // Vulkan descriptors cannot reference an empty buffer. Keep one
            // zero entry for skin-only scenes; no job reads it without morphs.
            let buffer = Buffer::from_iter(
                memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::STORAGE_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                [GpuMorphDelta::default()],
            )?;
            self.deformation_stats.morph_delta_upload_bytes += size_of::<GpuMorphDelta>() as u64;
            self.deformation_stats.buffer_allocations += 1;
            self.deformation_morph_buffer = Some(buffer.clone());
            buffer
        };
        let active_morph_upload: Vec<_> = if active_morphs.is_empty() {
            vec![GpuActiveMorph::default()]
        } else {
            active_morphs
        };
        self.deformation_stats.weight_upload_bytes +=
            (active_morph_upload.len() * size_of::<GpuActiveMorph>()) as u64;
        self.deformation_stats.buffer_allocations += 6;
        let active_morph_staging = Buffer::from_iter(
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
            active_morph_upload.iter().copied(),
        )?;
        let active_morph_buffer = Buffer::new_slice::<GpuActiveMorph>(
            memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER | BufferUsage::TRANSFER_DST,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                ..Default::default()
            },
            active_morph_upload.len() as DeviceSize,
        )?;
        cbb.copy_buffer(CopyBufferInfo::buffers(
            active_morph_staging,
            active_morph_buffer.clone(),
        ))?;
        let jobs_staging = Buffer::from_iter(
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
            jobs.iter().copied(),
        )?;
        let jobs_buffer = Buffer::new_slice::<GpuDeformationJob>(
            memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER | BufferUsage::TRANSFER_DST,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                ..Default::default()
            },
            jobs.len() as DeviceSize,
        )?;
        cbb.copy_buffer(CopyBufferInfo::buffers(jobs_staging, jobs_buffer.clone()))?;
        let workgroups_staging = Buffer::from_iter(
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
            workgroups.iter().copied(),
        )?;
        let workgroups_buffer = Buffer::new_slice::<GpuDeformationWorkgroup>(
            memory_allocator,
            BufferCreateInfo {
                usage: BufferUsage::STORAGE_BUFFER | BufferUsage::TRANSFER_DST,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                ..Default::default()
            },
            workgroups.len() as DeviceSize,
        )?;
        cbb.copy_buffer(CopyBufferInfo::buffers(
            workgroups_staging,
            workgroups_buffer.clone(),
        ))?;
        let set = DescriptorSet::new(
            self.descriptor_set_allocator.clone(),
            self.deformation_pipeline.layout().set_layouts()[0].clone(),
            [
                WriteDescriptorSet::buffer(0, base_buffer),
                WriteDescriptorSet::buffer(1, skin_buffer),
                WriteDescriptorSet::buffer(2, bones_buffer),
                WriteDescriptorSet::buffer(3, morph_buffer),
                WriteDescriptorSet::buffer(4, active_morph_buffer),
                WriteDescriptorSet::buffer(5, jobs_buffer),
                WriteDescriptorSet::buffer(6, workgroups_buffer),
                WriteDescriptorSet::buffer(7, output.clone()),
            ],
            [],
        )?;
        cbb.bind_pipeline_compute(self.deformation_pipeline.clone())?;
        cbb.bind_descriptor_sets(
            PipelineBindPoint::Compute,
            self.deformation_pipeline.layout().clone(),
            0,
            set,
        )?;
        let max_dispatch = self
            .context
            .device()
            .physical_device()
            .properties()
            .max_compute_work_group_count[0] as usize;
        for first in (0..workgroups.len()).step_by(max_dispatch) {
            let count = (workgroups.len() - first).min(max_dispatch);
            cbb.push_constants(
                self.deformation_pipeline.layout().clone(),
                0,
                mesh_deformation_cs::PushConstants {
                    workgroup_base: first as u32,
                },
            )?;
            unsafe { cbb.dispatch([count as u32, 1, 1])? };
            self.deformation_stats.dispatches += 1;
        }
        self.deformation_stats.jobs += jobs.len() as u64;
        self.deformation_stats.workgroups += workgroups.len() as u64;
        self.deformation_stats.dirty_vertices +=
            jobs.iter().map(|job| job.vertex_count as u64).sum::<u64>();
        self.deformation_stats.job_upload_bytes += (jobs.len() * size_of::<GpuDeformationJob>()
            + workgroups.len() * size_of::<GpuDeformationWorkgroup>())
            as u64;
        self.deformation_stats.live_cache_bytes = visual_world.deformation_cache_live_vertices()
            as u64
            * size_of::<GpuDeformedVertex>() as u64;
        self.deformation_stats.allocated_cache_bytes =
            self.deformation_output_capacity as u64 * size_of::<GpuDeformedVertex>() as u64;
        Ok(output)
    }
}
