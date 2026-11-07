// Bloom extraction and filtering; direct recording shared by fresh and retained paths.
impl VulkanoState {
    #[allow(clippy::too_many_arguments)]
    fn record_bloom_passes(
        &mut self,
        cbb: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        visual_world: &VisualWorld,
        post_process: &PostProcessInvocation,
        extent: [u32; 2],
        global_set_fg: &Arc<DescriptorSet>,
        global_set_bg: Option<&Arc<DescriptorSet>>,
        rig_set: &Arc<DescriptorSet>,
        emissive_instance_buffer: Option<&Subbuffer<[InstanceData]>>,
        emissive_instance_count: usize,
        emissive_cutout_instance_buffer: Option<&Subbuffer<[InstanceData]>>,
        emissive_cutout_instance_count: usize,
        background_emissive_instance_buffer: Option<&Subbuffer<[InstanceData]>>,
        background_emissive_instance_count: usize,
    ) -> Result<Option<Arc<ImageView>>, Box<dyn std::error::Error>> {
        let bloom_radius_pixels = post_process
            .config
            .effective_blur_radius_pixels(post_process.targets.bloom_extent[0]);
        let (
            Some(bloom_cfg),
            Some(bloom_source),
            Some(bloom_a),
            Some(bloom_b),
            Some(radius_pixels),
        ) = (
            post_process.config.bloom.as_ref(),
            post_process.targets.bloom_source.clone(),
            post_process.targets.bloom_a.clone(),
            post_process.targets.bloom_b.clone(),
            bloom_radius_pixels,
        )
        else {
            return Ok(None);
        };

        if bloom_cfg.intensity <= 0.0 {
            return Ok(None);
        }
        let has_foreground_emissive_content =
            emissive_instance_count > 0 || emissive_cutout_instance_count > 0;
        let has_background_emissive_content = background_emissive_instance_count > 0;
        if !has_foreground_emissive_content && !has_background_emissive_content {
            return Ok(None);
        }

        let begin_bloom_extraction = |cbb: &mut AutoCommandBufferBuilder<
            vulkano::command_buffer::PrimaryAutoCommandBuffer,
        >,
                                      load_op: AttachmentLoadOp,
                                      store_msaa_for_followup: bool|
         -> Result<(), Box<dyn std::error::Error>> {
            let mut bloom_attachment = RenderingAttachmentInfo {
                load_op,
                store_op: AttachmentStoreOp::Store,
                clear_value: match load_op {
                    AttachmentLoadOp::Clear => Some(ClearValue::from([0.0, 0.0, 0.0, 0.0])),
                    _ => None,
                },
                ..RenderingAttachmentInfo::image_view(
                    post_process
                        .targets
                        .bloom_source_msaa
                        .clone()
                        .unwrap_or_else(|| bloom_source.clone()),
                )
            };

            if post_process.targets.bloom_source_msaa.is_some() {
                bloom_attachment.resolve_info = Some(RenderingAttachmentResolveInfo::image_view(
                    bloom_source.clone(),
                ));
                bloom_attachment.store_op = if store_msaa_for_followup {
                    AttachmentStoreOp::Store
                } else {
                    AttachmentStoreOp::DontCare
                };
            }

            cbb.begin_rendering(RenderingInfo {
                render_area_offset: [0, 0],
                render_area_extent: extent,
                layer_count: 1,
                color_attachments: vec![Some(bloom_attachment)],
                depth_attachment: Some(RenderingAttachmentInfo {
                    load_op: AttachmentLoadOp::Load,
                    store_op: AttachmentStoreOp::DontCare,
                    ..RenderingAttachmentInfo::image_view(post_process.targets.depth.clone())
                }),
                ..Default::default()
            })?;

            cbb.set_viewport(
                0,
                vec![Viewport {
                    offset: [0.0, extent[1] as f32],
                    extent: [extent[0] as f32, -(extent[1] as f32)],
                    depth_range: 0.0..=1.0,
                    ..Default::default()
                }]
                .into(),
            )?;
            cbb.set_scissor(
                0,
                vec![Scissor {
                    offset: [0, 0],
                    extent,
                    ..Default::default()
                }]
                .into(),
            )?;
            Ok(())
        };

        if has_foreground_emissive_content {
            begin_bloom_extraction(
                cbb,
                AttachmentLoadOp::Clear,
                has_background_emissive_content,
            )?;
            if let Some(instance_buffer) = emissive_instance_buffer {
                self.record_instanced_draws_for_batches(
                    cbb,
                    global_set_fg,
                    rig_set,
                    instance_buffer,
                    emissive_instance_count,
                    visual_world.emissive_draw_batches(),
                    vulkano_cbb::MaterialDrawPass::Emission,
                    self.pipeline_emissive_prepass_toon_mesh.clone(),
                    self.pipeline_emissive_prepass_toon_mesh.clone(),
                    self.pipeline_emissive_prepass_toon_mesh.clone(),
                    self.pipeline_emissive_prepass_toon_mesh.clone(),
                    self.pipeline_emissive_prepass_toon_mesh.clone(),
                    self.pipeline_skinned_emissive_prepass_toon_mesh.clone(),
                    self.pipeline_skinned_emissive_prepass_toon_mesh.clone(),
                )?;
            }
            if let Some(instance_buffer) = emissive_cutout_instance_buffer {
                self.record_instanced_draws_for_batches(
                    cbb,
                    global_set_fg,
                    rig_set,
                    instance_buffer,
                    emissive_cutout_instance_count,
                    visual_world.emissive_cutout_batches(),
                    vulkano_cbb::MaterialDrawPass::Emission,
                    self.pipeline_emissive_prepass_toon_mesh_cutout.clone(),
                    self.pipeline_emissive_prepass_toon_mesh_cutout.clone(),
                    self.pipeline_emissive_prepass_toon_mesh_cutout.clone(),
                    self.pipeline_emissive_prepass_toon_mesh_cutout.clone(),
                    self.pipeline_emissive_prepass_toon_mesh_cutout.clone(),
                    self.pipeline_skinned_emissive_prepass_toon_mesh_cutout
                        .clone(),
                    self.pipeline_skinned_emissive_prepass_toon_mesh_cutout
                        .clone(),
                )?;
            }
            cbb.end_rendering()?;
        }

        if has_background_emissive_content {
            begin_bloom_extraction(
                cbb,
                if has_foreground_emissive_content {
                    AttachmentLoadOp::Load
                } else {
                    AttachmentLoadOp::Clear
                },
                false,
            )?;
            if let Some(instance_buffer) = background_emissive_instance_buffer {
                let global_set_bg =
                    global_set_bg.expect("background emissive extraction requires bg camera set");
                self.record_instanced_draws_for_batches(
                    cbb,
                    global_set_bg,
                    rig_set,
                    instance_buffer,
                    background_emissive_instance_count,
                    visual_world.background_occluded_lit_emissive_batches(),
                    vulkano_cbb::MaterialDrawPass::Emission,
                    self.pipeline_emissive_prepass_depth_write_toon_mesh.clone(),
                    self.pipeline_emissive_prepass_depth_write_toon_mesh.clone(),
                    self.pipeline_emissive_prepass_depth_write_toon_mesh.clone(),
                    self.pipeline_emissive_prepass_depth_write_toon_mesh.clone(),
                    self.pipeline_emissive_prepass_depth_write_toon_mesh.clone(),
                    self.pipeline_skinned_emissive_prepass_depth_write_toon_mesh
                        .clone(),
                    self.pipeline_skinned_emissive_prepass_depth_write_toon_mesh
                        .clone(),
                )?;
            }
            cbb.end_rendering()?;
        }

        let bloom_format = post_process.final_color_format;
        let blur_h_dir = [1.0 / post_process.targets.bloom_extent[0] as f32, 0.0];
        let blur_v_dir = [0.0, 1.0 / post_process.targets.bloom_extent[1] as f32];
        self.post_processing_renderer.record_final_pass(
            cbb,
            bloom_format,
            bloom_a.clone(),
            post_process.targets.bloom_extent,
            bloom_source,
            None,
            &post_process.config,
        )?;
        self.post_processing_renderer.record_blur_pass(
            cbb,
            bloom_format,
            bloom_a.clone(),
            bloom_b.clone(),
            post_process.targets.bloom_extent,
            blur_h_dir,
            radius_pixels,
        )?;
        self.post_processing_renderer.record_blur_pass(
            cbb,
            bloom_format,
            bloom_b,
            bloom_a.clone(),
            post_process.targets.bloom_extent,
            blur_v_dir,
            radius_pixels,
        )?;

        let count = (bloom_cfg.layers.clamp(1, 6) as usize - 1)
            .min(post_process.targets.bloom_pyramid.len());
        let mut filtered = bloom_a.clone();
        for level in post_process.targets.bloom_pyramid.iter().take(count) {
            self.post_processing_renderer.record_copy_pass(
                cbb,
                bloom_format,
                level.a.clone(),
                level.extent,
                filtered,
                SampleCount::Sample1,
            )?;
            self.post_processing_renderer.record_blur_pass(
                cbb,
                bloom_format,
                level.a.clone(),
                level.b.clone(),
                level.extent,
                [1.0 / level.extent[0] as f32, 0.0],
                radius_pixels,
            )?;
            self.post_processing_renderer.record_blur_pass(
                cbb,
                bloom_format,
                level.b.clone(),
                level.a.clone(),
                level.extent,
                [0.0, 1.0 / level.extent[1] as f32],
                radius_pixels,
            )?;
            filtered = level.a.clone();
        }
        // Uniform contribution per scale: changing layer count changes spread,
        // not the total DC gain of a constant emission image.
        for i in (0..count).rev() {
            let (fine, output, size) = if i == 0 {
                (
                    bloom_a.clone(),
                    post_process.targets.bloom_b.clone().unwrap(),
                    post_process.targets.bloom_extent,
                )
            } else {
                let level = &post_process.targets.bloom_pyramid[i - 1];
                (level.a.clone(), level.b.clone(), level.extent)
            };
            let accumulated = (count - i) as f32;
            self.post_processing_renderer.record_bloom_merge(
                cbb,
                bloom_format,
                output.clone(),
                size,
                fine,
                filtered,
                accumulated / (accumulated + 1.0),
            )?;
            filtered = output;
        }
        if count > 0 {
            self.post_processing_renderer.record_copy_pass(
                cbb,
                bloom_format,
                bloom_a.clone(),
                post_process.targets.bloom_extent,
                filtered,
                SampleCount::Sample1,
            )?;
        }

        Ok((bloom_cfg.intensity > 0.0).then_some(bloom_a))
    }
}
