// Included in the renderer module. Opaque static surfaces can optionally reuse
// their emitted radiance in a depth-tested extraction pass.
struct CustomGpuProgram {
    definition: std::sync::Arc<crate::engine::graphics::custom_material::CustomMaterialDefinition>,
    pipeline: Arc<GraphicsPipeline>,
    transparent_pipeline: Arc<GraphicsPipeline>,
    extraction_pipeline: Option<Arc<GraphicsPipeline>>,
}

struct CustomMaterialSnapshot {
    revision: u64,
    texture: TextureHandle,
    filtering: TextureFiltering,
    view: Arc<ImageView>,
    buffer: Subbuffer<[u8]>,
    set: Arc<DescriptorSet>,
}

impl VulkanoState {
    fn prepare_custom_programs(&mut self, visuals: &VisualWorld) {
        let mut live = std::collections::HashSet::new();
        for (_, instance) in visuals.materials.instances() {
            let Some(definition) = visuals.materials.definition(instance.definition) else {
                continue;
            };
            let Some(custom) = &definition.custom else {
                continue;
            };
            let handle = definition.static_program;
            live.insert(handle);
            if self.custom_programs.contains_key(&handle) {
                continue;
            }
            let Some(compilation) = custom.compiled.get() else {
                continue;
            };
            let result = (|| -> Result<CustomGpuProgram, String> {
                let compiled = compilation.as_ref().map_err(Clone::clone)?;
                if compiled.byte_size
                    > self
                        .context
                        .device()
                        .physical_device()
                        .properties()
                        .max_uniform_buffer_range as usize
                {
                    return Err(
                        "custom uniform block exceeds this device's uniform-buffer limit".into(),
                    );
                }
                let shader = unsafe {
                    vulkano::shader::ShaderModule::new(
                        self.context.device().clone(),
                        vulkano::shader::ShaderModuleCreateInfo::new(&compiled.words),
                    )
                }
                .map_err(|e| e.to_string())?;
                let mut create = self.custom_pipeline_template.clone();
                create.stages[1] = PipelineShaderStageCreateInfo::new(
                    shader
                        .entry_point("main")
                        .ok_or("missing fragment entry point")?,
                );
                let pipeline =
                    GraphicsPipeline::new(self.context.device().clone(), None, create.clone())
                        .map_err(|e| e.to_string())?;
                let mut transparent_create = create.clone();
                transparent_create.depth_stencil_state = Some(DepthStencilState {
                    depth: Some(DepthState {
                        write_enable: false,
                        ..DepthState::simple()
                    }),
                    ..Default::default()
                });
                let transparent_pipeline =
                    GraphicsPipeline::new(self.context.device().clone(), None, transparent_create)
                        .map_err(|e| e.to_string())?;
                let extraction_pipeline = if definition.supports_emission_extraction {
                    // Reuse exactly the same program and descriptor revision. Equal
                    // depth admits the visible surface without overwriting scene depth.
                    create.depth_stencil_state = Some(DepthStencilState {
                        depth: Some(DepthState {
                            write_enable: false,
                            compare_op: CompareOp::LessOrEqual,
                            ..DepthState::simple()
                        }),
                        ..Default::default()
                    });
                    Some(
                        GraphicsPipeline::new(self.context.device().clone(), None, create)
                            .map_err(|e| e.to_string())?,
                    )
                } else {
                    None
                };
                Ok(CustomGpuProgram {
                    definition: custom.clone(),
                    pipeline,
                    transparent_pipeline,
                    extraction_pipeline,
                })
            })();
            if let Err(error) = &result {
                eprintln!("[Material pipeline] {}: {error}", custom.fragment_uri);
            }
            self.custom_programs.insert(handle, result);
        }
        self.custom_programs
            .retain(|handle, _| live.contains(handle));
    }

    fn custom_material_set(
        &mut self,
        id: crate::engine::graphics::materials::MaterialInstanceId,
        material: crate::engine::graphics::MaterialHandle,
        texture: TextureHandle,
        filtering: TextureFiltering,
    ) -> Result<Option<Arc<DescriptorSet>>, Box<dyn std::error::Error>> {
        let program = self
            .custom_programs
            .get(&material)
            .and_then(|p| p.as_ref().ok())
            .ok_or("custom program not ready")?;
        let compiled = program
            .definition
            .compiled
            .get()
            .and_then(|p| p.as_ref().ok())
            .ok_or("custom program not compiled")?;
        let view = self
            .textures
            .get(&texture)
            .ok_or("missing custom material texture")?
            .view
            .clone();
        let sampler = self.sampler_for(filtering).clone();
        let entry = self
            .material_instances
            .get_mut(id)
            .ok_or("stale custom material instance")?;
        let revision = entry.record.revision;
        if let Some(slot) = entry.custom_snapshots.iter().find(|slot| {
            slot.revision == revision
                && slot.texture == texture
                && slot.filtering == filtering
                && Arc::ptr_eq(&slot.view, &view)
        }) {
            self.material_upload_stats.snapshot_reuses += 1;
            return Ok(Some(slot.set.clone()));
        }
        let bytes = compiled.pack(&entry.record.values)?;
        if let Some(slot) = entry.custom_snapshots.iter_mut().find(|slot| {
            slot.texture == texture
                && slot.filtering == filtering
                && Arc::strong_count(&slot.set) == 1
        }) {
            slot.buffer.write()?.copy_from_slice(&bytes);
            if !Arc::ptr_eq(&slot.view, &view) {
                slot.set = DescriptorSet::new(
                    self.descriptor_set_allocator.clone(),
                    self.set_layouts.material.clone(),
                    [
                        WriteDescriptorSet::buffer(0, slot.buffer.clone()),
                        WriteDescriptorSet::image_view_sampler(1, view.clone(), sampler.clone()),
                    ],
                    [],
                )?;
                slot.view = view.clone();
            }
            slot.revision = revision;
            self.material_upload_stats.uploads += 1;
            return Ok(Some(slot.set.clone()));
        }
        if entry.custom_snapshots.len() == MATERIAL_SNAPSHOT_LIMIT {
            return Err("all custom material snapshots are retained by command buffers".into());
        }
        let buffer = Buffer::from_iter(
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
            bytes,
        )?;
        let set = DescriptorSet::new(
            self.descriptor_set_allocator.clone(),
            self.set_layouts.material.clone(),
            [
                WriteDescriptorSet::buffer(0, buffer.clone()),
                WriteDescriptorSet::image_view_sampler(1, view.clone(), sampler),
            ],
            [],
        )?;
        entry.custom_snapshots.push(CustomMaterialSnapshot {
            revision,
            texture,
            filtering,
            view,
            buffer,
            set: set.clone(),
        });
        self.material_upload_stats.allocations += 1;
        self.material_upload_stats.uploads += 1;
        Ok(Some(set))
    }
}
