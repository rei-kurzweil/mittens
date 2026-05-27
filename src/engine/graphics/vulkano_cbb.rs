use super::*;

/// Pick a pipeline from either the window or XR multiview set, by field
/// name. Both `VulkanoState` and `XrPipelines` share the same `pipeline_*`
/// field naming so a single token can address either.
macro_rules! pipe {
    ($self:expr, $xr:expr, $name:ident) => {
        if $xr {
            $self.xr_pipelines.$name.clone()
        } else {
            $self.$name.clone()
        }
    };
}

impl VulkanoState {
    fn is_skinned_material(material: crate::engine::graphics::MaterialHandle) -> bool {
        matches!(
            material,
            crate::engine::graphics::MaterialHandle::SKINNED_TOON_MESH
                | crate::engine::graphics::MaterialHandle::SKINNED_EMISSIVE_TOON_MESH
        )
    }

    fn pipeline_for_material(
        &self,
        material: crate::engine::graphics::MaterialHandle,
        pipeline_toon: Arc<GraphicsPipeline>,
        pipeline_emissive: Arc<GraphicsPipeline>,
        pipeline_skinned: Arc<GraphicsPipeline>,
        pipeline_skinned_emissive: Arc<GraphicsPipeline>,
    ) -> Arc<GraphicsPipeline> {
        match material {
            crate::engine::graphics::MaterialHandle::EMISSIVE_TOON_MESH => pipeline_emissive,
            crate::engine::graphics::MaterialHandle::SKINNED_EMISSIVE_TOON_MESH => {
                pipeline_skinned_emissive
            }
            crate::engine::graphics::MaterialHandle::SKINNED_TOON_MESH => pipeline_skinned,
            _ => pipeline_toon,
        }
    }

    pub(super) fn record_instanced_draws_for_batches(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        global_set: &Arc<DescriptorSet>,
        rig_set: &Arc<DescriptorSet>,
        instance_buffer: &Subbuffer<[InstanceData]>,
        instance_count: usize,
        batches: &[crate::engine::graphics::visual_world::DrawBatch],
        pipeline_toon: Arc<GraphicsPipeline>,
        pipeline_emissive: Arc<GraphicsPipeline>,
        pipeline_skinned: Arc<GraphicsPipeline>,
        pipeline_skinned_emissive: Arc<GraphicsPipeline>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Bind pipeline/descriptor sets per (material, texture).
        let mut bound_material: Option<crate::engine::graphics::MaterialHandle> = None;
        let mut bound_texture: Option<TextureHandle> = None;
        let mut bound_filtering: Option<TextureFiltering> = None;
        let mut bound_quant: Option<u32> = None;

        for batch in batches {
            let pipeline = self.pipeline_for_material(
                batch.material,
                pipeline_toon.clone(),
                pipeline_emissive.clone(),
                pipeline_skinned.clone(),
                pipeline_skinned_emissive.clone(),
            );

            let texture_handle = batch.texture.unwrap_or(self.default_white_texture);
            let filtering = batch.texture_filtering;
            let quant_bits = batch.quant_steps.to_bits();

            if bound_material != Some(batch.material)
                || bound_texture != Some(texture_handle)
                || bound_filtering != Some(filtering)
                || bound_quant != Some(quant_bits)
            {
                let Some(material_set) = self.get_or_create_material_set(
                    batch.material,
                    texture_handle,
                    filtering,
                    batch.quant_steps,
                )?
                else {
                    continue;
                };

                cbb.bind_pipeline_graphics(pipeline.clone())?;
                cbb.bind_descriptor_sets(
                    PipelineBindPoint::Graphics,
                    pipeline.layout().clone(),
                    0,
                    (global_set.clone(), material_set, rig_set.clone()),
                )?;

                bound_material = Some(batch.material);
                bound_texture = Some(texture_handle);
                bound_filtering = Some(filtering);
                bound_quant = Some(quant_bits);
            }

            let Some(mesh) = self.meshes.get(&batch.mesh) else {
                continue;
            };
            if Self::is_skinned_material(batch.material) {
                let Some(skin) = mesh.skin_vertices.as_ref() else {
                    // Skinned pipeline expects a skinning vertex buffer.
                    continue;
                };
                cbb.bind_vertex_buffers(
                    0,
                    (mesh.vertices.clone(), skin.clone(), instance_buffer.clone()),
                )?;
            } else {
                cbb.bind_vertex_buffers(0, (mesh.vertices.clone(), instance_buffer.clone()))?;
            }
            cbb.bind_index_buffer(mesh.indices.clone())?;

            if instance_count > 0 && batch.count > 0 {
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

        Ok(())
    }

    pub(super) fn record_background_draws(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        visual_world: &VisualWorld,
        global_set: &Arc<DescriptorSet>,
        rig_set: &Arc<DescriptorSet>,
        instance_buffer: &Subbuffer<[InstanceData]>,
        instance_count: usize,
        is_xr_multiview: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if instance_count == 0 {
            return Ok(());
        }

        self.record_instanced_draws_for_batches(
            cbb,
            global_set,
            rig_set,
            instance_buffer,
            instance_count,
            visual_world.background_batches(),
            // Plain background: no depth write.
            pipe!(self, is_xr_multiview, pipeline_toon_mesh_transparent),
            pipe!(self, is_xr_multiview, pipeline_emissive_toon_mesh_transparent),
            pipe!(self, is_xr_multiview, pipeline_skinned_toon_mesh_transparent),
            pipe!(self, is_xr_multiview, pipeline_skinned_emissive_toon_mesh_transparent),
        )
    }

    pub(super) fn record_background_occluded_lit_draws(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        visual_world: &VisualWorld,
        global_set: &Arc<DescriptorSet>,
        rig_set: &Arc<DescriptorSet>,
        instance_buffer: &Subbuffer<[InstanceData]>,
        instance_count: usize,
        is_xr_multiview: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if instance_count == 0 {
            return Ok(());
        }

        self.record_instanced_draws_for_batches(
            cbb,
            global_set,
            rig_set,
            instance_buffer,
            instance_count,
            visual_world.background_occluded_lit_batches(),
            // Occluded+lit background: depth write ON for self-occlusion.
            pipe!(self, is_xr_multiview, pipeline_toon_mesh),
            pipe!(self, is_xr_multiview, pipeline_emissive_toon_mesh),
            pipe!(self, is_xr_multiview, pipeline_skinned_toon_mesh),
            pipe!(self, is_xr_multiview, pipeline_skinned_emissive_toon_mesh),
        )
    }

    /// Draw a DFS render stream (opaque or overlay phase).
    ///
    /// `ops` / `stream_instances` come from `visual_world.opaque_stream()` or
    /// `visual_world.overlay_stream()`. The `instance_buffer` must have been built
    /// from the matching `stream_instances` slice.
    ///
    /// `pipeline_normal` / `pipeline_emissive` are used for unclipped batches
    /// (`stencil_ref == 0`). `pipeline_clipped` / `pipeline_emissive_clipped` are
    /// used when `stencil_ref > 0`. `pipeline_stencil_incr` / `pipeline_stencil_decr`
    /// (fields on `self`) are used for `EnterClip` / `ExitClip` ops.
    #[allow(clippy::too_many_arguments)]
    fn record_phase_stream_draws(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        visual_world: &VisualWorld,
        global_set: &Arc<DescriptorSet>,
        rig_set: &Arc<DescriptorSet>,
        instance_buffer: &Subbuffer<[InstanceData]>,
        ops: &[crate::engine::graphics::visual_world::RenderOp],
        stream_instances: &[u32],
        pipeline_normal: Arc<GraphicsPipeline>,
        pipeline_emissive: Arc<GraphicsPipeline>,
        pipeline_skinned: Arc<GraphicsPipeline>,
        pipeline_skinned_emissive: Arc<GraphicsPipeline>,
        pipeline_clipped: Arc<GraphicsPipeline>,
        pipeline_emissive_clipped: Arc<GraphicsPipeline>,
        is_xr_multiview: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::engine::graphics::visual_world::RenderOp;
        use vulkano::pipeline::graphics::depth_stencil::StencilFaces;

        let instances = visual_world.instances();

        let mut bound_material: Option<crate::engine::graphics::MaterialHandle> = None;
        let mut bound_texture: Option<TextureHandle> = None;
        let mut bound_filtering: Option<TextureFiltering> = None;
        let mut bound_quant: Option<u32> = None;

        for op in ops {
            match op {
                RenderOp::EnterClip { instance_index, parent_ref, .. } => {
                    let inst = instances[*instance_index as usize];
                    let Some(slot) = stream_instances.iter().position(|&i| i == *instance_index)
                    else {
                        continue;
                    };
                    // Copy mesh data before the mutable get_or_create_material_set call.
                    let (mesh_verts, mesh_indices, mesh_index_count) = {
                        let Some(mesh) = self.meshes.get(&inst.renderable.mesh) else { continue };
                        (mesh.vertices.clone(), mesh.indices.clone(), mesh.index_count)
                    };
                    let texture_handle = inst.texture.unwrap_or(self.default_white_texture);
                    let Some(material_set) = self.get_or_create_material_set(
                        inst.renderable.material,
                        texture_handle,
                        inst.texture_filtering,
                        inst.quant_steps,
                    )?
                    else {
                        continue;
                    };
                    let pipeline = pipe!(self, is_xr_multiview, pipeline_stencil_incr);
                    cbb.bind_pipeline_graphics(pipeline.clone())?;
                    cbb.bind_descriptor_sets(
                        PipelineBindPoint::Graphics,
                        pipeline.layout().clone(),
                        0,
                        (global_set.clone(), material_set, rig_set.clone()),
                    )?;
                    cbb.set_stencil_reference(StencilFaces::FrontAndBack, *parent_ref as u32)?;
                    cbb.bind_vertex_buffers(0, (mesh_verts, instance_buffer.clone()))?;
                    cbb.bind_index_buffer(mesh_indices)?;
                    unsafe { cbb.draw_indexed(mesh_index_count, 1, 0, 0, slot as u32)? };
                    // Invalidate bound-state cache so the next DrawBatch rebinds.
                    bound_material = None;
                }

                RenderOp::DrawBatch(batch) => {
                    let pipeline = if batch.stencil_ref > 0 {
                        // Clipped draw: non-skinned only (UI quads are never skinned).
                        match batch.material {
                            crate::engine::graphics::MaterialHandle::EMISSIVE_TOON_MESH => {
                                pipeline_emissive_clipped.clone()
                            }
                            _ => pipeline_clipped.clone(),
                        }
                    } else {
                        self.pipeline_for_material(
                            batch.material,
                            pipeline_normal.clone(),
                            pipeline_emissive.clone(),
                            pipeline_skinned.clone(),
                            pipeline_skinned_emissive.clone(),
                        )
                    };

                    let texture_handle = batch.texture.unwrap_or(self.default_white_texture);
                    let filtering = batch.texture_filtering;
                    let quant_bits = batch.quant_steps.to_bits();

                    if bound_material != Some(batch.material)
                        || bound_texture != Some(texture_handle)
                        || bound_filtering != Some(filtering)
                        || bound_quant != Some(quant_bits)
                    {
                        let Some(material_set) = self.get_or_create_material_set(
                            batch.material,
                            texture_handle,
                            filtering,
                            batch.quant_steps,
                        )?
                        else {
                            continue;
                        };
                        cbb.bind_pipeline_graphics(pipeline.clone())?;
                        cbb.bind_descriptor_sets(
                            PipelineBindPoint::Graphics,
                            pipeline.layout().clone(),
                            0,
                            (global_set.clone(), material_set, rig_set.clone()),
                        )?;
                        bound_material = Some(batch.material);
                        bound_texture = Some(texture_handle);
                        bound_filtering = Some(filtering);
                        bound_quant = Some(quant_bits);
                    }

                    if batch.stencil_ref > 0 {
                        cbb.set_stencil_reference(
                            StencilFaces::FrontAndBack,
                            batch.stencil_ref as u32,
                        )?;
                    }

                    let Some(mesh) = self.meshes.get(&batch.mesh) else { continue };
                    if Self::is_skinned_material(batch.material) {
                        let Some(skin) = mesh.skin_vertices.as_ref() else { continue };
                        cbb.bind_vertex_buffers(
                            0,
                            (mesh.vertices.clone(), skin.clone(), instance_buffer.clone()),
                        )?;
                    } else {
                        cbb.bind_vertex_buffers(
                            0,
                            (mesh.vertices.clone(), instance_buffer.clone()),
                        )?;
                    }
                    cbb.bind_index_buffer(mesh.indices.clone())?;
                    if batch.count > 0 {
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

                RenderOp::ExitClip { instance_index, ref_value } => {
                    let inst = instances[*instance_index as usize];
                    let Some(slot) = stream_instances.iter().position(|&i| i == *instance_index)
                    else {
                        continue;
                    };
                    let (mesh_verts, mesh_indices, mesh_index_count) = {
                        let Some(mesh) = self.meshes.get(&inst.renderable.mesh) else { continue };
                        (mesh.vertices.clone(), mesh.indices.clone(), mesh.index_count)
                    };
                    let texture_handle = inst.texture.unwrap_or(self.default_white_texture);
                    let Some(material_set) = self.get_or_create_material_set(
                        inst.renderable.material,
                        texture_handle,
                        inst.texture_filtering,
                        inst.quant_steps,
                    )?
                    else {
                        continue;
                    };
                    let pipeline = pipe!(self, is_xr_multiview, pipeline_stencil_decr);
                    cbb.bind_pipeline_graphics(pipeline.clone())?;
                    cbb.bind_descriptor_sets(
                        PipelineBindPoint::Graphics,
                        pipeline.layout().clone(),
                        0,
                        (global_set.clone(), material_set, rig_set.clone()),
                    )?;
                    cbb.set_stencil_reference(StencilFaces::FrontAndBack, *ref_value as u32)?;
                    cbb.bind_vertex_buffers(0, (mesh_verts, instance_buffer.clone()))?;
                    cbb.bind_index_buffer(mesh_indices)?;
                    unsafe { cbb.draw_indexed(mesh_index_count, 1, 0, 0, slot as u32)? };
                    bound_material = None;
                }
            }
        }

        Ok(())
    }

    pub(super) fn record_opaque_draws(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        visual_world: &VisualWorld,
        global_set: &Arc<DescriptorSet>,
        rig_set: &Arc<DescriptorSet>,
        instance_buffer: &Subbuffer<[InstanceData]>,
        instance_count: usize,
        is_xr_multiview: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if instance_count == 0 {
            return Ok(());
        }

        let (ops, stream_instances) = visual_world.opaque_stream();
        self.record_phase_stream_draws(
            cbb,
            visual_world,
            global_set,
            rig_set,
            instance_buffer,
            ops,
            stream_instances,
            pipe!(self, is_xr_multiview, pipeline_toon_mesh),
            pipe!(self, is_xr_multiview, pipeline_emissive_toon_mesh),
            pipe!(self, is_xr_multiview, pipeline_skinned_toon_mesh),
            pipe!(self, is_xr_multiview, pipeline_skinned_emissive_toon_mesh),
            pipe!(self, is_xr_multiview, pipeline_opaque_clipped),
            pipe!(self, is_xr_multiview, pipeline_emissive_opaque_clipped),
            is_xr_multiview,
        )
    }

    pub(super) fn record_cutout_draws(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        visual_world: &VisualWorld,
        global_set: &Arc<DescriptorSet>,
        rig_set: &Arc<DescriptorSet>,
        instance_buffer: &Subbuffer<[InstanceData]>,
        instance_count: usize,
        is_xr_multiview: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if instance_count == 0 {
            return Ok(());
        }

        let (ops, stream_instances) = visual_world.cutout_stream();
        self.record_phase_stream_draws(
            cbb,
            visual_world,
            global_set,
            rig_set,
            instance_buffer,
            ops,
            stream_instances,
            pipe!(self, is_xr_multiview, pipeline_toon_mesh_cutout),
            pipe!(self, is_xr_multiview, pipeline_emissive_toon_mesh_cutout),
            pipe!(self, is_xr_multiview, pipeline_skinned_toon_mesh_cutout),
            pipe!(self, is_xr_multiview, pipeline_skinned_emissive_toon_mesh_cutout),
            pipe!(self, is_xr_multiview, pipeline_toon_mesh_cutout_clipped),
            pipe!(self, is_xr_multiview, pipeline_emissive_toon_mesh_cutout_clipped),
            is_xr_multiview,
        )
    }

    pub(super) fn record_overlay_draws(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        visual_world: &VisualWorld,
        global_set: &Arc<DescriptorSet>,
        rig_set: &Arc<DescriptorSet>,
        instance_buffer: &Subbuffer<[InstanceData]>,
        instance_count: usize,
        is_xr_multiview: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if instance_count == 0 {
            return Ok(());
        }

        // Overlay depth-tests with itself (depth write ON). Depth gets cleared right
        // before the overlay phase so it still draws on top of the scene.
        let (ops, stream_instances) = visual_world.overlay_stream();
        self.record_phase_stream_draws(
            cbb,
            visual_world,
            global_set,
            rig_set,
            instance_buffer,
            ops,
            stream_instances,
            pipe!(self, is_xr_multiview, pipeline_toon_mesh),
            pipe!(self, is_xr_multiview, pipeline_emissive_toon_mesh),
            pipe!(self, is_xr_multiview, pipeline_skinned_toon_mesh),
            pipe!(self, is_xr_multiview, pipeline_skinned_emissive_toon_mesh),
            pipe!(self, is_xr_multiview, pipeline_overlay_clipped),
            pipe!(self, is_xr_multiview, pipeline_emissive_overlay_clipped),
            is_xr_multiview,
        )
    }

    pub(super) fn record_transparent_single_draws(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        visual_world: &VisualWorld,
        global_set: &Arc<DescriptorSet>,
        rig_set: &Arc<DescriptorSet>,
        _eye: usize,
        is_xr_multiview: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (ops, stream_instances) = visual_world.transparent_single_stream();
        let transparent_single_instance_count = stream_instances.len();

        if transparent_single_instance_count == 0 {
            return Ok(());
        }

        // Build instance buffer in transparent-single stream order.
        let transparent_single_instance_buffer = self.build_instance_buffer_for_order_or_dummy(
            visual_world,
            stream_instances,
        )?;

        self.record_phase_stream_draws(
            cbb,
            visual_world,
            global_set,
            rig_set,
            &transparent_single_instance_buffer,
            ops,
            stream_instances,
            pipe!(self, is_xr_multiview, pipeline_toon_mesh_transparent),
            pipe!(self, is_xr_multiview, pipeline_emissive_toon_mesh_transparent),
            pipe!(self, is_xr_multiview, pipeline_skinned_toon_mesh_transparent),
            pipe!(self, is_xr_multiview, pipeline_skinned_emissive_toon_mesh_transparent),
            pipe!(self, is_xr_multiview, pipeline_toon_mesh_transparent_clipped),
            pipe!(self, is_xr_multiview, pipeline_emissive_toon_mesh_transparent_clipped),
            is_xr_multiview,
        )
    }

    pub(super) fn record_transparent_multi_draws(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        visual_world: &mut VisualWorld,
        global_set: &Arc<DescriptorSet>,
        rig_set: &Arc<DescriptorSet>,
        camera_target: crate::engine::graphics::CameraTarget,
        eye: usize,
        is_xr_multiview: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // --- Transparent pass (multi-layer, sorted) ---
        visual_world.prepare_transparent_multi_draw_cache_for_eye(camera_target, eye);
        let transparent_multi_instance_count = visual_world.transparent_multi_draw_order().len();

        if transparent_multi_instance_count == 0 {
            return Ok(());
        }

        // Build transparent instance buffer in transparent-multi draw order.
        let transparent_multi_instance_buffer = self.build_instance_buffer_for_order_or_dummy(
            &*visual_world,
            visual_world.transparent_multi_draw_order(),
        )?;

        // Bind pipeline/descriptor sets per (material, texture).
        let mut bound_material: Option<crate::engine::graphics::MaterialHandle> = None;
        let mut bound_texture: Option<TextureHandle> = None;
        let mut bound_filtering: Option<TextureFiltering> = None;
        let mut bound_quant: Option<u32> = None;

        for batch in visual_world.transparent_multi_draw_batches() {
            let texture_handle = batch.texture.unwrap_or(self.default_white_texture);
            let filtering = batch.texture_filtering;
            let quant_bits = batch.quant_steps.to_bits();

            if bound_material != Some(batch.material)
                || bound_texture != Some(texture_handle)
                || bound_filtering != Some(filtering)
                || bound_quant != Some(quant_bits)
            {
                let Some(material_set) = self.get_or_create_material_set(
                    batch.material,
                    texture_handle,
                    filtering,
                    batch.quant_steps,
                )?
                else {
                    continue;
                };

                let pipeline = self.pipeline_for_material(
                    batch.material,
                    pipe!(self, is_xr_multiview, pipeline_toon_mesh_transparent),
                    pipe!(self, is_xr_multiview, pipeline_emissive_toon_mesh_transparent),
                    pipe!(self, is_xr_multiview, pipeline_skinned_toon_mesh_transparent),
                    pipe!(self, is_xr_multiview, pipeline_skinned_emissive_toon_mesh_transparent),
                );

                cbb.bind_pipeline_graphics(pipeline.clone())?;
                cbb.bind_descriptor_sets(
                    PipelineBindPoint::Graphics,
                    pipeline.layout().clone(),
                    0,
                    (global_set.clone(), material_set, rig_set.clone()),
                )?;

                bound_material = Some(batch.material);
                bound_texture = Some(texture_handle);
                bound_filtering = Some(filtering);
                bound_quant = Some(quant_bits);
            }

            let Some(mesh) = self.meshes.get(&batch.mesh) else {
                continue;
            };
            if Self::is_skinned_material(batch.material) {
                let Some(skin) = mesh.skin_vertices.as_ref() else {
                    continue;
                };
                cbb.bind_vertex_buffers(
                    0,
                    (
                        mesh.vertices.clone(),
                        skin.clone(),
                        transparent_multi_instance_buffer.clone(),
                    ),
                )?;
            } else {
                cbb.bind_vertex_buffers(
                    0,
                    (
                        mesh.vertices.clone(),
                        transparent_multi_instance_buffer.clone(),
                    ),
                )?;
            }
            cbb.bind_index_buffer(mesh.indices.clone())?;

            // IMPORTANT: for correct alpha blending order, draw transparent instances
            // one-by-one in sorted order (do not rely on instancing order).
            for j in batch.start..(batch.start + batch.count) {
                unsafe {
                    cbb.draw_indexed(mesh.index_count, 1, 0, 0, j as u32)?;
                }
            }
        }

        Ok(())
    }
}
