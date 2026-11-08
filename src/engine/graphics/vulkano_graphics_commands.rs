// Prepared view resources are moved once into the recorder. Borrowed streams
// retain their existing batching and ordering; there is no per-draw allocation.
struct GraphicsRecording<'a> {
    rendering_info_clear_color_and_depth: RenderingInfo,
    viewport: Viewport,
    extent: [u32; 2],
    any_background: bool,
    sample_scene_color: bool,
    composite_bloom_before_refraction: bool,
    defer_overlay_until_before_final_composite: bool,
    split_bloom: bool,
    scene_candidate: bool,
    view_target_slot: usize,
    view_target_slots_total: usize,
    #[cfg(test)]
    eye: usize,
    global_set_fg: Arc<DescriptorSet>,
    rig_set: Arc<DescriptorSet>,
    global_set_bg: Option<Arc<DescriptorSet>>,
    global_set_refraction: Option<Arc<DescriptorSet>>,
    instance_buffer: Subbuffer<[InstanceData]>,
    instance_count: usize,
    outline_instance_buffer: Option<Subbuffer<[InstanceData]>>,
    outline_instance_count: usize,
    background_instance_buffer: Option<Subbuffer<[InstanceData]>>,
    background_instance_count: usize,
    background_occluded_lit_instance_buffer: Option<Subbuffer<[InstanceData]>>,
    background_occluded_lit_instance_count: usize,
    cutout_instance_buffer: Option<Subbuffer<[InstanceData]>>,
    cutout_instance_count: usize,
    refraction_instance_buffer: Option<Subbuffer<[InstanceData]>>,
    refraction_instance_count: usize,
    rough_transmission_instance_buffer: Option<Subbuffer<[InstanceData]>>,
    rough_transmission_instance_count: usize,
    overlay_instance_buffer: Option<Subbuffer<[InstanceData]>>,
    overlay_instance_count: usize,
    emissive_instance_buffer: Option<Subbuffer<[InstanceData]>>,
    emissive_instance_count: usize,
    emissive_cutout_instance_buffer: Option<Subbuffer<[InstanceData]>>,
    emissive_cutout_instance_count: usize,
    background_occluded_lit_emissive_instance_buffer: Option<Subbuffer<[InstanceData]>>,
    background_occluded_lit_emissive_instance_count: usize,
    stencil_clip_debug_instance_buffer: Option<Subbuffer<[InstanceData]>>,
    stencil_clip_debug_instance_count: usize,
    transparent_single_instance_buffer: Option<Subbuffer<[InstanceData]>>,
    opaque_ops: &'a [crate::engine::graphics::visual_world::RenderOp],
    opaque_instances: &'a [u32],
    outline_ops: &'a [crate::engine::graphics::visual_world::RenderOp],
    outline_instances: &'a [u32],
    cutout_ops: &'a [crate::engine::graphics::visual_world::RenderOp],
    cutout_instances: &'a [u32],
    refraction_ops: &'a [crate::engine::graphics::visual_world::RenderOp],
    refraction_instances: &'a [u32],
    rough_transmission_ops: &'a [crate::engine::graphics::visual_world::RenderOp],
    rough_transmission_instances: &'a [u32],
    overlay_ops: &'a [crate::engine::graphics::visual_world::RenderOp],
    overlay_instances: &'a [u32],
    excluded_instance: Option<crate::engine::graphics::primitives::InstanceHandle>,
    color_attachment_view: Arc<ImageView>,
    depth_view: Arc<ImageView>,
    color_resolve_view: Option<Arc<ImageView>>,
    scene_color_source: Option<Arc<ImageView>>,
    scene_snapshot: Option<RefractionSnapshotViews>,
    rough_transmission_pyramid: Option<RoughTransmissionPyramidViews>,
    post_process: Option<PostProcessInvocation>,
    runtime_texture_publication: Option<(TextureHandle, Arc<ImageView>)>,
    stencil_clip_debug_handle: Option<TextureHandle>,
    stencil_clip_debug_batches: Vec<crate::engine::graphics::visual_world::DrawBatch>,
}

impl VulkanoState {
    fn record_graphics_commands(
        &mut self,
        mut cbb: AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        visual_world: &VisualWorld,
        recording: GraphicsRecording<'_>,
        preceding_commands: &mut Vec<PrimaryCommands>,
        #[cfg(test)] cpu_stage_clock: &mut Option<std::time::Instant>,
    ) -> Result<
        AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        Box<dyn std::error::Error>,
    > {
        let GraphicsRecording {
            rendering_info_clear_color_and_depth,
            viewport,
            extent,
            any_background,
            sample_scene_color,
            composite_bloom_before_refraction,
            defer_overlay_until_before_final_composite,
            split_bloom,
            scene_candidate,
            view_target_slot,
            view_target_slots_total,
            #[cfg(test)]
            eye,
            global_set_fg,
            rig_set,
            global_set_bg,
            global_set_refraction,
            instance_buffer,
            instance_count,
            outline_instance_buffer,
            outline_instance_count,
            background_instance_buffer,
            background_instance_count,
            background_occluded_lit_instance_buffer,
            background_occluded_lit_instance_count,
            cutout_instance_buffer,
            cutout_instance_count,
            refraction_instance_buffer,
            refraction_instance_count,
            rough_transmission_instance_buffer,
            rough_transmission_instance_count,
            overlay_instance_buffer,
            overlay_instance_count,
            emissive_instance_buffer,
            emissive_instance_count,
            emissive_cutout_instance_buffer,
            emissive_cutout_instance_count,
            background_occluded_lit_emissive_instance_buffer,
            background_occluded_lit_emissive_instance_count,
            stencil_clip_debug_instance_buffer,
            stencil_clip_debug_instance_count,
            transparent_single_instance_buffer,
            opaque_ops,
            opaque_instances,
            outline_ops,
            outline_instances,
            cutout_ops,
            cutout_instances,
            refraction_ops,
            refraction_instances,
            rough_transmission_ops,
            rough_transmission_instances,
            overlay_ops,
            overlay_instances,
            excluded_instance,
            color_attachment_view,
            depth_view,
            color_resolve_view,
            scene_color_source,
            scene_snapshot,
            rough_transmission_pyramid,
            post_process,
            runtime_texture_publication,
            stencil_clip_debug_handle,
            stencil_clip_debug_batches,
        } = recording;
        #[cfg(test)]
        if let Some(profile) = self.profile.as_ref() {
            profile.gpu_pass_timestamp(&mut cbb, eye, 0)?;
        }
        let mut blurred_bloom: Option<Arc<ImageView>> = None;
        #[cfg(test)]
        if let Some(profile) = self.profile.as_ref() {
            profile.material_diagnostic.reset(&mut cbb, eye)?;
        }
        // Single dynamic-rendering scope. This keeps MSAA resolve straightforward.
        cbb.begin_rendering(rendering_info_clear_color_and_depth)?;

        cbb.set_viewport(0, vec![viewport.clone()].into())?;
        cbb.set_scissor(
            0,
            vec![Scissor {
                offset: [0, 0],
                extent: [extent[0], extent[1]],
                ..Default::default()
            }]
            .into(),
        )?;

        if any_background {
            // Background phase: draw:
            // 1) plain background (no depth write)
            // 2) occluded+lit background (depth write ON for self-occlusion)
            if let Some(global_set_bg) = global_set_bg.as_ref() {
                if let Some(background_instance_buffer) = background_instance_buffer.as_ref() {
                    self.record_background_draws(
                        &mut cbb,
                        visual_world,
                        global_set_bg,
                        &rig_set,
                        background_instance_buffer,
                        background_instance_count,
                    )?;
                }

                if let Some(background_occluded_lit_instance_buffer) =
                    background_occluded_lit_instance_buffer.as_ref()
                {
                    self.record_background_occluded_lit_draws(
                        &mut cbb,
                        visual_world,
                        global_set_bg,
                        &rig_set,
                        background_occluded_lit_instance_buffer,
                        background_occluded_lit_instance_count,
                    )?;
                }
            }

            // Foreground phase: clear depth so background doesn't occlude.
            // NOTE: `clear_attachments` requires a bound graphics pipeline.
            cbb.bind_pipeline_graphics(self.pipeline_toon_mesh.clone())?;
            cbb.clear_attachments(
                smallvec::smallvec![ClearAttachment::Depth(1.0)],
                smallvec::smallvec![ClearRect {
                    offset: [0, 0],
                    extent: [extent[0], extent[1]],
                    array_layers: 0..1,
                }],
            )?;
        }

        #[cfg(test)]
        if let Some(profile) = self.profile.as_ref() {
            profile.gpu_pass_timestamp(&mut cbb, eye, 1)?;
        }
        profile_cpu_stage!(self, cpu_stage_clock, "background_record");

        if let Some(outline_instance_buffer) = outline_instance_buffer.as_ref() {
            self.record_outline_draws(
                &mut cbb,
                visual_world,
                &global_set_fg,
                &rig_set,
                outline_instance_buffer,
                outline_instance_count,
                Some((outline_ops, outline_instances)),
            )?;
        }

        #[cfg(test)]
        if let Some(profile) = self.profile.as_ref() {
            profile.gpu_pass_timestamp(&mut cbb, eye, 2)?;
        }
        profile_cpu_stage!(self, cpu_stage_clock, "outline_record");

        #[cfg(test)]
        let diagnostic_ops = if let Some(profile) = self.profile.as_ref() {
            profile.material_diagnostic.begin(&mut cbb, eye)?;
            profile.material_diagnostic.reorder(opaque_ops, visual_world, opaque_instances)
        } else {
            None
        };
        #[cfg(test)]
        let opaque_ops = diagnostic_ops.as_deref().unwrap_or(opaque_ops);
        self.record_opaque_draws(
            &mut cbb,
            visual_world,
            &global_set_fg,
            &rig_set,
            &instance_buffer,
            instance_count,
            Some((opaque_ops, opaque_instances)),
        )?;

        #[cfg(test)]
        if let Some(profile) = self.profile.as_ref() {
            profile.material_diagnostic.end(&mut cbb, eye)?;
            profile.gpu_pass_timestamp(&mut cbb, eye, 3)?;
        }
        profile_cpu_stage!(self, cpu_stage_clock, "opaque_record");

        if let Some(cutout_instance_buffer) = cutout_instance_buffer.as_ref() {
            self.record_cutout_draws(
                &mut cbb,
                visual_world,
                &global_set_fg,
                &rig_set,
                cutout_instance_buffer,
                cutout_instance_count,
                Some((cutout_ops, cutout_instances)),
            )?;
        }

        #[cfg(test)]
        if let Some(profile) = self.profile.as_ref() {
            profile.gpu_pass_timestamp(&mut cbb, eye, 4)?;
        }
        profile_cpu_stage!(self, cpu_stage_clock, "cutout_record");

        if sample_scene_color {
            cbb.end_rendering()?;
            let snapshot = scene_snapshot
                .as_ref()
                .expect("scene-color snapshot")
                .clone();

            if self.msaa_samples == SampleCount::Sample1 {
                cbb.copy_image(CopyImageInfo::images(
                    depth_view.image().clone(),
                    snapshot.depth_attachment.image().clone(),
                ))?;
            }

            if composite_bloom_before_refraction {
                let post = post_process
                    .as_ref()
                    .expect("Bloom-before-refraction requires post-processing");
                blurred_bloom = self.record_bloom_passes(
                    &mut cbb,
                    visual_world,
                    post,
                    extent,
                    &global_set_fg,
                    global_set_bg.as_ref(),
                    &rig_set,
                    emissive_instance_buffer.as_ref(),
                    emissive_instance_count,
                    emissive_cutout_instance_buffer.as_ref(),
                    emissive_cutout_instance_count,
                    background_occluded_lit_emissive_instance_buffer.as_ref(),
                    background_occluded_lit_emissive_instance_count,
                )?;
                self.post_processing_renderer.record_final_pass(
                    &mut cbb,
                    post.final_color_format,
                    snapshot.color.clone(),
                    extent,
                    post.targets.main_color.clone(),
                    blurred_bloom.clone(),
                    &post.config,
                )?;
                self.post_processing_renderer.record_copy_pass(
                    &mut cbb,
                    post.final_color_format,
                    color_attachment_view.clone(),
                    extent,
                    snapshot.color.clone(),
                    self.msaa_samples,
                )?;
            } else {
                cbb.copy_image(CopyImageInfo::images(
                    scene_color_source
                        .as_ref()
                        .expect("scene-color source")
                        .image()
                        .clone(),
                    snapshot.color.image().clone(),
                ))?;
            }

            if let Some(pyramid) = rough_transmission_pyramid.as_ref() {
                self.record_rough_transmission_pyramid(
                    &mut cbb,
                    snapshot.color.image().format(),
                    &snapshot,
                    pyramid,
                    extent,
                )?;
            }

            let mut color_attachment_load = RenderingAttachmentInfo {
                load_op: AttachmentLoadOp::Load,
                store_op: AttachmentStoreOp::Store,
                ..RenderingAttachmentInfo::image_view(color_attachment_view.clone())
            };
            if let Some(resolve_view) = color_resolve_view.clone() {
                color_attachment_load.resolve_info =
                    Some(RenderingAttachmentResolveInfo::image_view(resolve_view));
                color_attachment_load.store_op = if defer_overlay_until_before_final_composite {
                    AttachmentStoreOp::Store
                } else {
                    AttachmentStoreOp::DontCare
                };
            }

            cbb.begin_rendering(RenderingInfo {
                render_area_offset: [0, 0],
                render_area_extent: [extent[0], extent[1]],
                layer_count: 1,
                color_attachments: vec![Some(color_attachment_load)],
                depth_attachment: Some(RenderingAttachmentInfo {
                    load_op: AttachmentLoadOp::Load,
                    store_op: if post_process.is_some() {
                        AttachmentStoreOp::Store
                    } else {
                        AttachmentStoreOp::DontCare
                    },
                    ..RenderingAttachmentInfo::image_view(depth_view.clone())
                }),
                stencil_attachment: Some(RenderingAttachmentInfo {
                    load_op: AttachmentLoadOp::Load,
                    store_op: if defer_overlay_until_before_final_composite {
                        AttachmentStoreOp::Store
                    } else {
                        AttachmentStoreOp::DontCare
                    },
                    ..RenderingAttachmentInfo::image_view(depth_view.clone())
                }),
                ..Default::default()
            })?;
            cbb.set_viewport(0, vec![viewport.clone()].into())?;
            cbb.set_scissor(
                0,
                vec![Scissor {
                    offset: [0, 0],
                    extent: [extent[0], extent[1]],
                    ..Default::default()
                }]
                .into(),
            )?;
        }

        if let Some(refraction_instance_buffer) = refraction_instance_buffer.as_ref() {
            self.record_refraction_draws(
                &mut cbb,
                visual_world,
                global_set_refraction.as_ref().unwrap_or(&global_set_fg),
                &rig_set,
                refraction_instance_buffer,
                refraction_instance_count,
                Some((refraction_ops, refraction_instances)),
                sample_scene_color,
            )?;
        }

        if let Some(rough_transmission_instance_buffer) =
            rough_transmission_instance_buffer.as_ref()
        {
            self.record_rough_transmission_draws(
                &mut cbb,
                visual_world,
                global_set_refraction.as_ref().unwrap_or(&global_set_fg),
                &rig_set,
                rough_transmission_instance_buffer,
                rough_transmission_instance_count,
                Some((rough_transmission_ops, rough_transmission_instances)),
                sample_scene_color,
            )?;
        }

        self.record_transparent_single_draws(
            &mut cbb,
            visual_world,
            &global_set_fg,
            &rig_set,
            transparent_single_instance_buffer.as_ref(),
            excluded_instance,
        )?;

        self.record_transparent_multi_draws(
            &mut cbb,
            visual_world,
            &global_set_fg,
            &rig_set,
            excluded_instance,
        )?;

        // Overlay phase: when post-process is disabled, clear depth here so overlay draws on
        // top of the scene immediately. When post-process is enabled, defer overlay until
        // after emissive extraction so opaque/cutout depth can occlude the emissive pass.
        if overlay_instance_count > 0 && !defer_overlay_until_before_final_composite {
            // NOTE: `clear_attachments` requires a bound graphics pipeline.
            cbb.bind_pipeline_graphics(self.pipeline_toon_mesh.clone())?;
            cbb.clear_attachments(
                smallvec::smallvec![ClearAttachment::Depth(1.0)],
                smallvec::smallvec![ClearRect {
                    offset: [0, 0],
                    extent: [extent[0], extent[1]],
                    array_layers: 0..1,
                }],
            )?;

            if let Some(overlay_instance_buffer) = overlay_instance_buffer.as_ref() {
                self.record_overlay_draws(
                    &mut cbb,
                    visual_world,
                    &global_set_fg,
                    &rig_set,
                    overlay_instance_buffer,
                    overlay_instance_count,
                    Some((overlay_ops, overlay_instances)),
                )?;
            }
        }

        cbb.end_rendering()?;

        #[cfg(test)]
        if let Some(profile) = self.profile.as_ref() {
            profile.gpu_pass_timestamp(&mut cbb, eye, 5)?;
        }
        if let Some((handle, src_view)) = runtime_texture_publication {
            let dst_view = self.ensure_runtime_texture_target(handle, &src_view)?;
            cbb.copy_image(CopyImageInfo::images(
                src_view.image().clone(),
                dst_view.image().clone(),
            ))?;
        }

        profile_cpu_stage!(
            self,
            cpu_stage_clock,
            "transmission_transparency_overlay_record"
        );

        if let Some(post_process) = post_process {
            if !composite_bloom_before_refraction && split_bloom && !scene_candidate {
                // End the dynamic scene primary before the reusable extraction/filter chain.
                // Future chaining retains Vulkano's automatic cross-primary synchronization.
                preceding_commands.push(cbb.build()?);
                profile_cpu_stage!(self, cpu_stage_clock, "command_buffer_build");
                let (commands, output) = self.retained_bloom_commands(
                    visual_world,
                    &post_process,
                    extent,
                    &global_set_fg,
                    global_set_bg.as_ref(),
                    &rig_set,
                    emissive_instance_buffer.as_ref(),
                    emissive_instance_count,
                    emissive_cutout_instance_buffer.as_ref(),
                    emissive_cutout_instance_count,
                    background_occluded_lit_emissive_instance_buffer.as_ref(),
                    background_occluded_lit_emissive_instance_count,
                )?;
                preceding_commands.push(commands);
                blurred_bloom = output;
                cbb = AutoCommandBufferBuilder::primary(
                    self.command_buffer_allocator.clone(),
                    self.context.graphics_queue().queue_family_index(),
                    CommandBufferUsage::OneTimeSubmit,
                )?;
            } else if !composite_bloom_before_refraction {
                blurred_bloom = self.record_bloom_passes(
                    &mut cbb,
                    visual_world,
                    &post_process,
                    extent,
                    &global_set_fg,
                    global_set_bg.as_ref(),
                    &rig_set,
                    emissive_instance_buffer.as_ref(),
                    emissive_instance_count,
                    emissive_cutout_instance_buffer.as_ref(),
                    emissive_cutout_instance_count,
                    background_occluded_lit_emissive_instance_buffer.as_ref(),
                    background_occluded_lit_emissive_instance_count,
                )?;
            }

            let final_output_view = post_process.final_output_view.clone();

            if overlay_instance_count > 0 {
                cbb.begin_rendering(RenderingInfo {
                    render_area_offset: [0, 0],
                    render_area_extent: [extent[0], extent[1]],
                    layer_count: 1,
                    color_attachments: vec![Some({
                        let mut color_attachment_load = RenderingAttachmentInfo {
                            load_op: AttachmentLoadOp::Load,
                            store_op: AttachmentStoreOp::Store,
                            ..RenderingAttachmentInfo::image_view(color_attachment_view.clone())
                        };
                        if let Some(resolve_view) = color_resolve_view.clone() {
                            color_attachment_load.resolve_info =
                                Some(RenderingAttachmentResolveInfo::image_view(resolve_view));
                            color_attachment_load.store_op = AttachmentStoreOp::DontCare;
                        }
                        color_attachment_load
                    })],
                    depth_attachment: Some(RenderingAttachmentInfo {
                        load_op: AttachmentLoadOp::Clear,
                        store_op: AttachmentStoreOp::DontCare,
                        clear_value: Some(ClearValue::Depth(1.0)),
                        ..RenderingAttachmentInfo::image_view(depth_view.clone())
                    }),
                    stencil_attachment: Some(RenderingAttachmentInfo {
                        load_op: AttachmentLoadOp::Load,
                        store_op: AttachmentStoreOp::DontCare,
                        ..RenderingAttachmentInfo::image_view(depth_view.clone())
                    }),
                    ..Default::default()
                })?;

                cbb.set_viewport(0, vec![viewport.clone()].into())?;
                cbb.set_scissor(
                    0,
                    vec![Scissor {
                        offset: [0, 0],
                        extent: [extent[0], extent[1]],
                        ..Default::default()
                    }]
                    .into(),
                )?;

                if let Some(overlay_instance_buffer) = overlay_instance_buffer.as_ref() {
                    self.record_overlay_draws(
                        &mut cbb,
                        visual_world,
                        &global_set_fg,
                        &rig_set,
                        overlay_instance_buffer,
                        overlay_instance_count,
                        Some((overlay_ops, overlay_instances)),
                    )?;
                }
                cbb.end_rendering()?;
            }

            self.post_processing_renderer.record_final_pass(
                &mut cbb,
                post_process.final_color_format,
                final_output_view.clone(),
                extent,
                post_process.targets.main_color.clone(),
                if composite_bloom_before_refraction {
                    None
                } else {
                    blurred_bloom
                },
                &post_process.config,
            )?;

            for (handle, src_view) in
                self.collect_runtime_texture_publications(visual_world, &post_process)
            {
                let dst_view = self.ensure_runtime_texture_target(handle, &src_view)?;
                cbb.copy_image(CopyImageInfo::images(
                    src_view.image().clone(),
                    dst_view.image().clone(),
                ))?;
            }
        }

        #[cfg(test)]
        if let Some(profile) = self.profile.as_ref() {
            profile.gpu_pass_timestamp(&mut cbb, eye, 6)?;
        }
        profile_cpu_stage!(self, cpu_stage_clock, "postprocess_record");

        if let (Some(handle), Some(debug_instance_buffer)) = (
            stencil_clip_debug_handle,
            stencil_clip_debug_instance_buffer.as_ref(),
        ) {
            self.ensure_window_runtime_debug_targets(
                view_target_slots_total,
                extent,
                color_attachment_view.image().format(),
            )?;

            let debug_view = self
                .window_runtime_debug_targets
                .as_ref()
                .and_then(|targets| targets.color_views.get(view_target_slot))
                .cloned()
                .ok_or("missing window runtime debug target")?;

            let debug_msaa_view = self
                .window_runtime_debug_targets
                .as_ref()
                .and_then(|targets| targets.msaa_color_views.get(view_target_slot))
                .cloned();

            let mut debug_color_attachment = RenderingAttachmentInfo {
                load_op: AttachmentLoadOp::Clear,
                store_op: AttachmentStoreOp::Store,
                clear_value: Some(ClearValue::from([1.0, 0.0, 1.0, 1.0])),
                ..RenderingAttachmentInfo::image_view(
                    debug_msaa_view
                        .clone()
                        .unwrap_or_else(|| debug_view.clone()),
                )
            };
            if debug_msaa_view.is_some() {
                debug_color_attachment.resolve_info = Some(
                    RenderingAttachmentResolveInfo::image_view(debug_view.clone()),
                );
                debug_color_attachment.store_op = AttachmentStoreOp::DontCare;
            }

            cbb.begin_rendering(RenderingInfo {
                render_area_offset: [0, 0],
                render_area_extent: [extent[0], extent[1]],
                layer_count: 1,
                color_attachments: vec![Some(debug_color_attachment)],
                depth_attachment: Some(RenderingAttachmentInfo {
                    load_op: AttachmentLoadOp::Clear,
                    store_op: AttachmentStoreOp::DontCare,
                    clear_value: Some(ClearValue::Depth(1.0)),
                    ..RenderingAttachmentInfo::image_view(depth_view.clone())
                }),
                stencil_attachment: Some(RenderingAttachmentInfo {
                    load_op: AttachmentLoadOp::Clear,
                    store_op: AttachmentStoreOp::DontCare,
                    clear_value: Some(ClearValue::Stencil(0)),
                    ..RenderingAttachmentInfo::image_view(depth_view.clone())
                }),
                ..Default::default()
            })?;

            cbb.set_viewport(0, vec![viewport.clone()].into())?;
            cbb.set_scissor(
                0,
                vec![Scissor {
                    offset: [0, 0],
                    extent: [extent[0], extent[1]],
                    ..Default::default()
                }]
                .into(),
            )?;

            self.record_instanced_draws_for_batches(
                &mut cbb,
                &global_set_fg,
                &rig_set,
                debug_instance_buffer,
                stencil_clip_debug_instance_count,
                &stencil_clip_debug_batches,
                vulkano_cbb::MaterialDrawPass::Surface,
                self.pipeline_toon_mesh.clone(),
                self.pipeline_unlit_mesh.clone(),
                self.pipeline_mirror_mesh.clone(),
                self.pipeline_grid_mesh.clone(),
                self.pipeline_emissive_toon_mesh.clone(),
                self.pipeline_skinned_toon_mesh.clone(),
                self.pipeline_skinned_emissive_toon_mesh.clone(),
            )?;

            cbb.end_rendering()?;

            let dst_view = self.ensure_runtime_texture_target(handle, &debug_view)?;
            cbb.copy_image(CopyImageInfo::images(
                debug_view.image().clone(),
                dst_view.image().clone(),
            ))?;
        }

        #[cfg(test)]
        if let Some(profile) = self.profile.as_ref() {
            profile.gpu_pass_timestamp(&mut cbb, eye, 7)?;
        }
        profile_cpu_stage!(self, cpu_stage_clock, "debug_record");

        #[cfg(test)]
        if let Some(profile) = self.profile.as_ref() {
            profile.timestamp(&mut cbb, eye, 2)?;
        }
        Ok(cbb)
    }
}
