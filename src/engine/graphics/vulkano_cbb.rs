use super::*;

impl VulkanoState {
    fn record_instanced_draws_for_batches(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        global_set: &Arc<DescriptorSet>,
        instance_buffer: &Subbuffer<[InstanceData]>,
        instance_count: usize,
        batches: &[crate::engine::graphics::visual_world::DrawBatch],
        pipeline: Arc<GraphicsPipeline>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Bind pipeline/descriptor sets per (material, texture).
        let mut bound_material: Option<crate::engine::graphics::MaterialHandle> = None;
        let mut bound_texture: Option<TextureHandle> = None;
        let mut bound_filtering: Option<TextureFiltering> = None;
        let mut bound_quant: Option<u32> = None;

        for batch in batches {
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
                    (global_set.clone(), material_set),
                )?;

                bound_material = Some(batch.material);
                bound_texture = Some(texture_handle);
                bound_filtering = Some(filtering);
                bound_quant = Some(quant_bits);
            }

            let Some(mesh) = self.meshes.get(&batch.mesh) else {
                continue;
            };
            cbb.bind_vertex_buffers(0, (mesh.vertices.clone(), instance_buffer.clone()))?;
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
        instance_buffer: &Subbuffer<[InstanceData]>,
        instance_count: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if instance_count == 0 {
            return Ok(());
        }

        self.record_instanced_draws_for_batches(
            cbb,
            global_set,
            instance_buffer,
            instance_count,
            visual_world.background_batches(),
            // Plain background: no depth write.
            self.pipeline_toon_mesh_transparent.clone(),
        )
    }

    pub(super) fn record_background_occluded_lit_draws(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        visual_world: &VisualWorld,
        global_set: &Arc<DescriptorSet>,
        instance_buffer: &Subbuffer<[InstanceData]>,
        instance_count: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if instance_count == 0 {
            return Ok(());
        }

        self.record_instanced_draws_for_batches(
            cbb,
            global_set,
            instance_buffer,
            instance_count,
            visual_world.background_occluded_lit_batches(),
            // Occluded+lit background: depth write ON for self-occlusion.
            self.pipeline_toon_mesh.clone(),
        )
    }

    pub(super) fn record_opaque_draws(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        visual_world: &VisualWorld,
        global_set: &Arc<DescriptorSet>,
        instance_buffer: &Subbuffer<[InstanceData]>,
        instance_count: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if instance_count == 0 {
            return Ok(());
        }

        self.record_instanced_draws_for_batches(
            cbb,
            global_set,
            instance_buffer,
            instance_count,
            visual_world.draw_batches(),
            self.pipeline_toon_mesh.clone(),
        )
    }

    pub(super) fn record_cutout_draws(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        visual_world: &VisualWorld,
        global_set: &Arc<DescriptorSet>,
        instance_buffer: &Subbuffer<[InstanceData]>,
        instance_count: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if instance_count == 0 {
            return Ok(());
        }

        self.record_instanced_draws_for_batches(
            cbb,
            global_set,
            instance_buffer,
            instance_count,
            visual_world.cutout_batches(),
            self.pipeline_toon_mesh_cutout.clone(),
        )
    }

    pub(super) fn record_transparent_single_draws(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        visual_world: &VisualWorld,
        global_set: &Arc<DescriptorSet>,
        _eye: usize,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let transparent_single_instance_count = visual_world.transparent_single_draw_order().len();

        if transparent_single_instance_count == 0 {
            return Ok(());
        }

        // Build instance buffer in transparent-single draw order.
        let transparent_single_instance_buffer = self.build_instance_buffer_for_order_or_dummy(
            visual_world,
            visual_world.transparent_single_draw_order(),
        )?;

        // Bind pipeline/descriptor sets per (material, texture).
        let mut bound_material: Option<crate::engine::graphics::MaterialHandle> = None;
        let mut bound_texture: Option<TextureHandle> = None;
        let mut bound_filtering: Option<TextureFiltering> = None;
        let mut bound_quant: Option<u32> = None;

        for batch in visual_world.transparent_single_draw_batches() {
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

                cbb.bind_pipeline_graphics(self.pipeline_toon_mesh_transparent.clone())?;
                cbb.bind_descriptor_sets(
                    PipelineBindPoint::Graphics,
                    self.pipeline_toon_mesh_transparent.layout().clone(),
                    0,
                    (global_set.clone(), material_set),
                )?;

                bound_material = Some(batch.material);
                bound_texture = Some(texture_handle);
                bound_filtering = Some(filtering);
                bound_quant = Some(quant_bits);
            }

            let Some(mesh) = self.meshes.get(&batch.mesh) else {
                continue;
            };
            cbb.bind_vertex_buffers(
                0,
                (
                    mesh.vertices.clone(),
                    transparent_single_instance_buffer.clone(),
                ),
            )?;
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

        Ok(())
    }

    pub(super) fn record_transparent_multi_draws(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        visual_world: &mut VisualWorld,
        global_set: &Arc<DescriptorSet>,
        camera_target: crate::engine::graphics::CameraTarget,
        eye: usize,
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

                cbb.bind_pipeline_graphics(self.pipeline_toon_mesh_transparent.clone())?;
                cbb.bind_descriptor_sets(
                    PipelineBindPoint::Graphics,
                    self.pipeline_toon_mesh_transparent.layout().clone(),
                    0,
                    (global_set.clone(), material_set),
                )?;

                bound_material = Some(batch.material);
                bound_texture = Some(texture_handle);
                bound_filtering = Some(filtering);
                bound_quant = Some(quant_bits);
            }

            let Some(mesh) = self.meshes.get(&batch.mesh) else {
                continue;
            };
            cbb.bind_vertex_buffers(
                0,
                (
                    mesh.vertices.clone(),
                    transparent_multi_instance_buffer.clone(),
                ),
            )?;
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
