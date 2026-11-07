// Per-view resource preparation and scene command recording.
impl VulkanoState {
    fn build_draw_batches_command_buffer(
        &mut self,
        visual_world: &mut VisualWorld,
        render_view: &RenderView,
        view_target_slot: usize,
        view_target_slots_total: usize,
        color_attachment_view: Arc<ImageView>,
        color_resolve_view: Option<Arc<ImageView>>,
        depth_view: Arc<ImageView>,
        extent: [u32; 2],
        post_process: Option<PostProcessInvocation>,
        runtime_texture_publication: Option<(TextureHandle, Arc<ImageView>)>,
        scene_snapshot: Option<RefractionSnapshotViews>,
        rough_transmission_pyramid: Option<RoughTransmissionPyramidViews>,
    ) -> Result<ViewCommands, Box<dyn std::error::Error>> {
        #[cfg(test)]
        let mut cpu_stage_clock = self.profile.as_ref().map(|_| std::time::Instant::now());

        self.retained_uploads
            .borrow_mut()
            .begin_view(&render_view.kind, view_target_slot);

        // Captures can publish replacement image views earlier this frame.
        self.retire_runtime_resources(visual_world);
        self.apply_pending_runtime_texture_updates();
        self.sync_material_instances(visual_world);
        self.refresh_command_dependencies(visual_world);
        self.view_mirror_bindings
            .prepare(visual_world, &render_view.kind);

        profile_cpu_stage!(self, cpu_stage_clock, "material_sync");

        let (camera_target, eye) = match render_view.kind {
            RenderViewKind::Window => (crate::engine::graphics::CameraTarget::Window, 0),
            RenderViewKind::XrEye { eye } => (crate::engine::graphics::CameraTarget::Xr, eye),
            RenderViewKind::Mirror {
                family, view_index, ..
            } => match family {
                crate::engine::graphics::visual_world::MirrorViewerFamily::Monoscopic => {
                    (crate::engine::graphics::CameraTarget::Window, view_index)
                }
                crate::engine::graphics::visual_world::MirrorViewerFamily::Stereoscopic => {
                    (crate::engine::graphics::CameraTarget::Xr, view_index)
                }
            },
        };
        let excluded_instance = match &render_view.kind {
            RenderViewKind::Mirror {
                excluded_instance, ..
            } => *excluded_instance,
            _ => None,
        }
        .filter(|&handle| visual_world.instance(handle).is_some());

        let instance_order_changed = self.last_instance_exclusion != excluded_instance;
        self.last_instance_exclusion = excluded_instance;
        let queue = self.context.graphics_queue().clone();
        let mesh_vertex_counts: HashMap<_, _> = self
            .meshes
            .iter()
            .map(|(handle, mesh)| (*handle, mesh.vertex_count))
            .collect();
        visual_world.sync_deformation_ranges(&mesh_vertex_counts);

        // Reuse draw topology until its structural inputs change.
        let draw_cache_rebuilt = visual_world.prepare_draw_cache();
        if draw_cache_rebuilt {
            self.command_reuse.scenes.retain(|_, _| false);
        }

        // Multi-layer transparency sort is always per-view.
        visual_world.prepare_transparent_multi_draw_cache_for_view(render_view.view);

        // Consume dirty flags so they reflect "changed since last render".
        // For multi-eye (XR) rendering, only consume on the first eye.
        let instance_data_dirty = if eye == 0 {
            visual_world.take_instance_data_dirty()
        } else {
            visual_world.instance_data_dirty()
        };

        // Shared deformation is consumed by the first command buffer, regardless of view.
        let dirty_bones_ranges = visual_world.take_dirty_bones_ranges();
        let _bones_palette_dirty = !dirty_bones_ranges.is_empty();

        profile_cpu_stage!(self, cpu_stage_clock, "draw_cache_and_transparency_sort");

        // --- Opaque pass ---
        // Buffer indexed by opaque_stream().1; use its length for cache invalidation.
        let owned_opaque_stream =
            excluded_instance.map(|excluded| visual_world.opaque_stream_excluding(Some(excluded)));
        let (opaque_ops, opaque_instances) =
            if let Some((ops, instances)) = owned_opaque_stream.as_ref() {
                (&ops[..], &instances[..])
            } else {
                visual_world.opaque_stream()
            };
        let instance_count = opaque_instances.len();

        // --- Toon outline pass ---
        let owned_outline_stream =
            excluded_instance.map(|excluded| visual_world.outline_stream_excluding(Some(excluded)));
        let (outline_ops, outline_instances) =
            if let Some((ops, instances)) = owned_outline_stream.as_ref() {
                (&ops[..], &instances[..])
            } else {
                visual_world.outline_stream()
            };
        let outline_instance_count = outline_instances.len();

        // --- Background pass ---
        // Background instances are stored in their own draw order/batches.
        let background_instance_count = visual_world.background_order().len();
        let background_occluded_lit_instance_count =
            visual_world.background_occluded_lit_order().len();
        let any_background =
            background_instance_count > 0 || background_occluded_lit_instance_count > 0;

        // --- Cutout pass ---
        let owned_cutout_stream =
            excluded_instance.map(|excluded| visual_world.cutout_stream_excluding(Some(excluded)));
        let (cutout_ops, cutout_instances) =
            if let Some((ops, instances)) = owned_cutout_stream.as_ref() {
                (&ops[..], &instances[..])
            } else {
                visual_world.cutout_stream()
            };
        let cutout_instance_count = cutout_instances.len();

        // --- Sharp scene-color refraction pass ---
        let owned_refraction_stream = excluded_instance
            .map(|excluded| visual_world.refraction_stream_excluding(Some(excluded)));
        let (refraction_ops, refraction_instances) =
            if let Some((ops, instances)) = owned_refraction_stream.as_ref() {
                (&ops[..], &instances[..])
            } else {
                visual_world.refraction_stream()
            };
        let refraction_instance_count = refraction_instances.len();
        let refraction_instance_buffer = self.build_instance_buffer_for_order_opt(
            &*visual_world,
            InstanceUpload::Refraction,
            refraction_instances,
        )?;

        // --- Rough scene-color transmission pass ---
        let owned_rough_transmission_stream = excluded_instance
            .map(|excluded| visual_world.rough_transmission_stream_excluding(Some(excluded)));
        let (rough_transmission_ops, rough_transmission_instances) =
            if let Some((ops, instances)) = owned_rough_transmission_stream.as_ref() {
                (&ops[..], &instances[..])
            } else {
                visual_world.rough_transmission_stream()
            };
        let rough_transmission_instance_count = rough_transmission_instances.len();
        let rough_transmission_instance_buffer = self.build_instance_buffer_for_order_opt(
            &*visual_world,
            InstanceUpload::RoughTransmission,
            rough_transmission_instances,
        )?;
        let scene_color_source = scene_snapshot.as_ref().and_then(|_| {
            post_process
                .as_ref()
                .map(|post_process| post_process.targets.main_color.clone())
        });
        let sample_scene_color = scene_color_source.is_some();

        // --- Overlay pass ---
        // Buffer indexed by overlay_stream().1; use its length for cache invalidation.
        let owned_overlay_stream =
            excluded_instance.map(|excluded| visual_world.overlay_stream_excluding(Some(excluded)));
        let (overlay_ops, overlay_instances) =
            if let Some((ops, instances)) = owned_overlay_stream.as_ref() {
                (&ops[..], &instances[..])
            } else {
                visual_world.overlay_stream()
            };
        let overlay_instance_count = overlay_instances.len();

        let stencil_clip_debug_requested = camera_target
            == crate::engine::graphics::CameraTarget::Window
            && eye == 0
            && visual_world.stencil_clip_debug_requested();
        let stencil_clip_debug_handle = if stencil_clip_debug_requested {
            visual_world.runtime_texture_handle(INTERNAL_RENDERER_STENCIL_CLIP_DEBUG_SELECTOR)
        } else {
            None
        };
        let stencil_clip_debug_enabled =
            stencil_clip_debug_requested && stencil_clip_debug_handle.is_some();
        let stencil_clip_debug_batches = if stencil_clip_debug_enabled {
            Self::build_stencil_clip_debug_batches(visual_world)
        } else {
            Vec::new()
        };
        let stencil_clip_debug_instance_count = stencil_clip_debug_batches.len();

        // --- Emissive-only post-process source passes ---
        let emissive_instance_count = visual_world.emissive_draw_order().len();
        let emissive_cutout_instance_count = visual_world.emissive_cutout_order().len();

        profile_cpu_stage!(
            self,
            cpu_stage_clock,
            "draw_streams_and_transmission_buffers"
        );

        let need_instance_buffer = instance_data_dirty
            || draw_cache_rebuilt
            || instance_order_changed
            || self.cached_instance_buffer.is_none()
            || self.cached_instance_count != instance_count;

        // Opaque instance buffer indexed by opaque_stream().1 (not draw_order) so the
        // stream's batch.start offsets address the correct slot in the buffer.
        let instance_buffer: Subbuffer<[InstanceData]> = if !need_instance_buffer {
            self.cached_instance_buffer
                .as_ref()
                .expect("cached_instance_buffer")
                .clone()
        } else {
            let buf = self.build_instance_buffer_for_order_or_dummy(
                &*visual_world,
                InstanceUpload::Opaque,
                &opaque_instances,
            )?;

            self.cached_instance_count = instance_count;
            self.cached_instance_buffer = Some(buf.clone());
            buf
        };

        let need_outline_instance_buffer = instance_data_dirty
            || draw_cache_rebuilt
            || instance_order_changed
            || self.cached_outline_instance_buffer.is_none()
            || self.cached_outline_instance_count != outline_instance_count;
        let outline_instance_buffer = if !need_outline_instance_buffer {
            self.cached_outline_instance_buffer.clone()
        } else {
            let buffer = self.build_instance_buffer_for_order_opt(
                &*visual_world,
                InstanceUpload::Outline,
                outline_instances,
            )?;
            self.cached_outline_instance_count = outline_instance_count;
            self.cached_outline_instance_buffer = buffer.clone();
            buffer
        };

        let need_background_instance_buffer = instance_data_dirty
            || draw_cache_rebuilt
            || instance_order_changed
            || self.cached_background_instance_count != background_instance_count;
        let background_instance_buffer = if !need_background_instance_buffer {
            self.cached_background_instance_buffer.clone()
        } else {
            let buf = self.build_instance_buffer_for_order_opt(
                &*visual_world,
                InstanceUpload::Background,
                visual_world.background_order(),
            )?;
            self.cached_background_instance_count = background_instance_count;
            self.cached_background_instance_buffer = buf.clone();
            buf
        };

        let need_background_occluded_lit_instance_buffer = instance_data_dirty
            || draw_cache_rebuilt
            || instance_order_changed
            || self.cached_background_occluded_lit_instance_count
                != background_occluded_lit_instance_count;
        let background_occluded_lit_instance_buffer =
            if !need_background_occluded_lit_instance_buffer {
                self.cached_background_occluded_lit_instance_buffer.clone()
            } else {
                let buf = self.build_instance_buffer_for_order_opt(
                    &*visual_world,
                    InstanceUpload::LitBackground,
                    visual_world.background_occluded_lit_order(),
                )?;
                self.cached_background_occluded_lit_instance_count =
                    background_occluded_lit_instance_count;
                self.cached_background_occluded_lit_instance_buffer = buf.clone();
                buf
            };

        let need_cutout_instance_buffer = instance_data_dirty
            || draw_cache_rebuilt
            || instance_order_changed
            || self.cached_cutout_instance_count != cutout_instance_count;
        let cutout_instance_buffer = if !need_cutout_instance_buffer {
            self.cached_cutout_instance_buffer.clone()
        } else {
            let buf = self.build_instance_buffer_for_order_opt(
                &*visual_world,
                InstanceUpload::Cutout,
                &cutout_instances,
            )?;
            self.cached_cutout_instance_count = cutout_instance_count;
            self.cached_cutout_instance_buffer = buf.clone();
            buf
        };

        let need_overlay_instance_buffer = instance_data_dirty
            || draw_cache_rebuilt
            || instance_order_changed
            || self.cached_overlay_instance_count != overlay_instance_count;
        // Overlay buffer indexed by overlay_stream().1 so batch.start offsets are correct.
        let overlay_instance_buffer = if !need_overlay_instance_buffer {
            self.cached_overlay_instance_buffer.clone()
        } else {
            let buf = self.build_instance_buffer_for_order_opt(
                &*visual_world,
                InstanceUpload::Overlay,
                &overlay_instances,
            )?;
            self.cached_overlay_instance_count = overlay_instance_count;
            self.cached_overlay_instance_buffer = buf.clone();
            buf
        };

        let stencil_clip_debug_instance_buffer = if stencil_clip_debug_enabled {
            self.build_stencil_clip_debug_instance_buffer(
                &*visual_world,
                visual_world.stencil_clip_order(),
            )?
        } else {
            None
        };
        let emissive_instance_buffer = self.build_instance_buffer_for_order_opt(
            &*visual_world,
            InstanceUpload::Emission,
            visual_world.emissive_draw_order(),
        )?;
        let background_occluded_lit_emissive_instance_count =
            visual_world.background_occluded_lit_emissive_order().len();
        let background_occluded_lit_emissive_instance_buffer =
            if background_occluded_lit_emissive_instance_count > 0 {
                self.build_instance_buffer_for_order_opt(
                    &*visual_world,
                    InstanceUpload::LitEmission,
                    visual_world.background_occluded_lit_emissive_order(),
                )?
            } else {
                None
            };

        let emissive_cutout_instance_buffer = self.build_instance_buffer_for_order_opt(
            &*visual_world,
            InstanceUpload::CutoutEmission,
            visual_world.emissive_cutout_order(),
        )?;

        let single_owned = excluded_instance
            .map(|excluded| visual_world.transparent_single_stream_excluding(Some(excluded)));
        let single_order = single_owned.as_ref().map_or_else(
            || visual_world.transparent_single_stream().1,
            |(_, order)| order.as_slice(),
        );
        let transparent_single_instance_buffer = self.build_instance_buffer_for_order_opt(
            visual_world,
            InstanceUpload::Transparent,
            single_order,
        )?;

        profile_cpu_stage!(self, cpu_stage_clock, "instance_buffers");

        #[cfg(test)]
        if let Some(profile) = self.profile.as_mut() {
            profile.view_cache_decisions.push(ViewCacheDecision {
                eye,
                draw_cache_rebuilt,
                instance_data_dirty,
                buffer_refresh_requested: [
                    need_instance_buffer,
                    need_outline_instance_buffer,
                    need_background_instance_buffer,
                    need_background_occluded_lit_instance_buffer,
                    need_cutout_instance_buffer,
                    need_overlay_instance_buffer,
                ],
            });
        }

        let prime_bloom = post_process
            .as_ref()
            .filter(|post| {
                post.config
                    .bloom
                    .as_ref()
                    .is_some_and(|b| b.intensity > 0.0)
                    && (emissive_instance_count
                        + emissive_cutout_instance_count
                        + background_occluded_lit_emissive_instance_count
                        > 0)
            })
            .map(Self::bloom_attachments);
        // Bloom-only reuse is retained as an opt-in experiment: its measured
        // cross-pass boundaries regressed total frame time. Production falls back
        // to fresh recording when complete graphics reuse is ineligible.
        let split_bloom = self.command_reuse.enabled
            && !self.command_reuse.scene_enabled
            && post_process
                .as_ref()
                .is_some_and(|post| self.bloom_can_split(post));
        let scene_candidate = self.command_reuse.enabled
            && self.command_reuse.scene_enabled
            && !matches!(render_view.kind, RenderViewKind::Mirror { .. })
            && excluded_instance.is_none()
            && !sample_scene_color
            && runtime_texture_publication.is_none()
            && !stencil_clip_debug_requested
            && visual_world.transparent_multi_draw_order().is_empty()
            && post_process.as_ref().is_none_or(|post| {
                post.config
                    .emissive_pass
                    .as_ref()
                    .is_none_or(|cfg| cfg.output_texture.is_none())
                    && post
                        .config
                        .bloom
                        .as_ref()
                        .is_none_or(|cfg| cfg.output_texture.is_none())
            });
        let scene_attachments = Self::scene_attachments(
            &color_attachment_view,
            color_resolve_view.as_ref(),
            &depth_view,
            post_process.as_ref(),
            prime_bloom.is_some(),
        );
        let clear_color = visual_world.clear_color();
        let defer_overlay_until_before_final_composite =
            post_process.is_some() && overlay_instance_count > 0;
        let composite_bloom_before_refraction = sample_scene_color
            && post_process
                .as_ref()
                .is_some_and(|post| post.config.bloom.is_some());

        let mut color_attachment_clear = RenderingAttachmentInfo {
            load_op: AttachmentLoadOp::Clear,
            store_op: AttachmentStoreOp::Store,
            clear_value: Some(ClearValue::from(clear_color)),
            ..RenderingAttachmentInfo::image_view(color_attachment_view.clone())
        };

        if let Some(resolve_view) = color_resolve_view.clone() {
            color_attachment_clear.resolve_info =
                Some(RenderingAttachmentResolveInfo::image_view(resolve_view));
            // The multisampled attachment doesn't need to be stored when resolve is used,
            // except when post-process is active and we plan to reopen the scene color
            // attachment for a deferred overlay pass before final composite.
            color_attachment_clear.store_op =
                if defer_overlay_until_before_final_composite || sample_scene_color {
                    AttachmentStoreOp::Store
                } else {
                    AttachmentStoreOp::DontCare
                };
        }

        let depth_resolve = scene_snapshot
            .as_ref()
            .filter(|_| self.msaa_samples != SampleCount::Sample1)
            .map(|snapshot| RenderingAttachmentResolveInfo {
                mode: ResolveMode::SampleZero,
                ..RenderingAttachmentResolveInfo::image_view(snapshot.depth_attachment.clone())
            });
        let mut depth_attachment_clear = RenderingAttachmentInfo {
            load_op: AttachmentLoadOp::Clear,
            store_op: if post_process.is_some() {
                AttachmentStoreOp::Store
            } else {
                AttachmentStoreOp::DontCare
            },
            clear_value: Some(ClearValue::Depth(1.0)),
            ..RenderingAttachmentInfo::image_view(depth_view.clone())
        };
        depth_attachment_clear.resolve_info = depth_resolve.clone();

        let mut stencil_attachment_clear = RenderingAttachmentInfo {
            load_op: AttachmentLoadOp::Clear,
            store_op: if defer_overlay_until_before_final_composite || sample_scene_color {
                AttachmentStoreOp::Store
            } else {
                AttachmentStoreOp::DontCare
            },
            clear_value: Some(ClearValue::Stencil(0)),
            ..RenderingAttachmentInfo::image_view(depth_view.clone())
        };
        stencil_attachment_clear.resolve_info = depth_resolve;

        let rendering_info_clear_color_and_depth = RenderingInfo {
            render_area_offset: [0, 0],
            render_area_extent: [extent[0], extent[1]],
            layer_count: 1,
            color_attachments: vec![Some(color_attachment_clear)],
            depth_attachment: Some(depth_attachment_clear.clone()),
            stencil_attachment: Some(stencil_attachment_clear),
            ..Default::default()
        };

        // Engine convention: +Y is up in clip space.
        // Vulkan's default viewport maps NDC Y with opposite direction, so we flip the
        // viewport by using a negative height.
        let viewport = Viewport {
            offset: [0.0, extent[1] as f32],
            extent: [extent[0] as f32, -(extent[1] as f32)],
            depth_range: 0.0..=1.0,
            ..Default::default()
        };

        profile_cpu_stage!(self, cpu_stage_clock, "attachment_setup");

        // Camera uniform buffer (set=0, binding=0) for foreground.
        let camera_ubo_fg = CameraUBO {
            view: render_view.view,
            proj: render_view.proj,
            camera2d: visual_world.camera_2d(),
            viewport: [extent[0] as f32, extent[1] as f32],
            _pad0: [0.0, 0.0],

            ambient_light: visual_world.ambient_light(),
            renderer_flags: u32::from(visual_world.transmission_depth_compare()),
        };

        let camera_buffer_fg: Subbuffer<CameraUBO> = self.retained_uploads.borrow_mut().camera(
            self.context.memory_allocator().clone(),
            false,
            camera_ubo_fg,
        )?;

        // Lights storage buffer (set=0, binding=1).
        let lights_ssbo = LightsSSBO::from_lights(visual_world.lights());

        let lights_buffer: Subbuffer<LightsSSBO> = self
            .retained_uploads
            .borrow_mut()
            .lights(self.context.memory_allocator().clone(), lights_ssbo)?;

        // Global descriptor set (set=0): per-frame camera + lights.
        //
        // `global_set_fg` is the *foreground* variant used for normal scene rendering
        // (opaque + transparent passes). Its camera UBO uses the full view matrix,
        // so camera translation causes normal parallax.
        let fallback_scene_color = self
            .textures
            .get(&self.default_white_texture)
            .ok_or("missing default scene-color texture")?
            .view
            .clone();
        let global_set_fg = self.retained_global_set(
            camera_buffer_fg.clone(),
            lights_buffer.clone(),
            fallback_scene_color.clone(),
        )?;

        let global_set_refraction = if let Some(snapshot) = scene_snapshot.as_ref() {
            let rough_pyramid = rough_transmission_pyramid.as_ref();
            Some(DescriptorSet::new(
                self.descriptor_set_allocator.clone(),
                self.set_layouts.global.clone(),
                [
                    WriteDescriptorSet::buffer(0, camera_buffer_fg.clone()),
                    WriteDescriptorSet::buffer(1, lights_buffer.clone()),
                    WriteDescriptorSet::image_view_sampler(
                        2,
                        snapshot.color.clone(),
                        self.sampler_scene_color.clone(),
                    ),
                    WriteDescriptorSet::image_view_sampler(
                        3,
                        snapshot.depth_sampled.clone(),
                        self.sampler_scene_depth.clone(),
                    ),
                    WriteDescriptorSet::image_view_sampler(
                        4,
                        rough_pyramid
                            .map(|pyramid| pyramid.half.clone())
                            .unwrap_or_else(|| snapshot.color.clone()),
                        self.sampler_scene_color.clone(),
                    ),
                    WriteDescriptorSet::image_view_sampler(
                        5,
                        rough_pyramid
                            .map(|pyramid| pyramid.quarter.clone())
                            .unwrap_or_else(|| snapshot.color.clone()),
                        self.sampler_scene_color.clone(),
                    ),
                    WriteDescriptorSet::image_view_sampler(
                        6,
                        rough_pyramid
                            .map(|pyramid| pyramid.eighth.clone())
                            .unwrap_or_else(|| snapshot.color.clone()),
                        self.sampler_scene_color.clone(),
                    ),
                    WriteDescriptorSet::image_view_sampler(
                        7,
                        rough_pyramid
                            .map(|pyramid| pyramid.sixteenth.clone())
                            .unwrap_or_else(|| snapshot.color.clone()),
                        self.sampler_scene_color.clone(),
                    ),
                    WriteDescriptorSet::image_view_sampler(
                        8,
                        rough_pyramid
                            .map(|pyramid| pyramid.thirtysecond.clone())
                            .unwrap_or_else(|| snapshot.color.clone()),
                        self.sampler_scene_color.clone(),
                    ),
                ],
                [],
            )?)
        } else {
            None
        };

        // Background global set: same layout + lights, but view translation removed.
        //
        // This makes backgrounds behave like a skybox: they rotate with the camera but
        // do not appear to move when the camera translates.
        let global_set_bg: Option<Arc<DescriptorSet>> = if !any_background {
            None
        } else {
            let mut view_bg = render_view.view;
            view_bg[3] = [0.0, 0.0, 0.0, 1.0];

            let camera_ubo_bg = CameraUBO {
                view: view_bg,
                proj: render_view.proj,
                camera2d: visual_world.camera_2d(),
                viewport: [extent[0] as f32, extent[1] as f32],
                _pad0: [0.0, 0.0],

                ambient_light: visual_world.ambient_light(),
                renderer_flags: u32::from(visual_world.transmission_depth_compare()),
            };

            let camera_buffer_bg: Subbuffer<CameraUBO> = self
                .retained_uploads
                .borrow_mut()
                .camera(self.context.memory_allocator().clone(), true, camera_ubo_bg)?;

            let set = self.retained_global_set(
                camera_buffer_bg,
                lights_buffer.clone(),
                fallback_scene_color,
            )?;
            Some(set)
        };

        profile_cpu_stage!(self, cpu_stage_clock, "camera_lights_global_descriptors");

        let mut preceding_commands = Vec::with_capacity(2);
        let (mut cbb, rig_set) = self.record_deformation_commands(
            visual_world,
            &dirty_bones_ranges,
            #[cfg(test)]
            eye,
            #[cfg(test)]
            &mut cpu_stage_clock,
        )?;

        let view_key = self.retained_uploads.borrow().active.unwrap();
        let scene_view_key = (
            view_key,
            post_process.as_ref().map_or_else(
                || Arc::as_ptr(&color_attachment_view) as usize,
                |post| Arc::as_ptr(&post.final_output_view) as usize,
            ),
        );
        let scene_warmed = self
            .command_reuse
            .scene_primed
            .get(&scene_view_key)
            .is_some_and(|old| *old == scene_attachments);
        let mut scene_key = None;
        #[cfg(test)]
        let mut scene_draws_before = None;
        if scene_candidate && scene_warmed {
            // Bone uploads and deformation are always recorded, independently of graphics.
            preceding_commands.push(cbb.build()?);
            profile_cpu_stage!(self, cpu_stage_clock, "command_buffer_build");
            let key = self.scene_identity(
                visual_world,
                extent,
                &scene_attachments,
                post_process.as_ref(),
                &global_set_fg,
                global_set_bg.as_ref(),
                &rig_set,
                [
                    Some(&instance_buffer),
                    outline_instance_buffer.as_ref(),
                    background_instance_buffer.as_ref(),
                    background_occluded_lit_instance_buffer.as_ref(),
                    cutout_instance_buffer.as_ref(),
                    overlay_instance_buffer.as_ref(),
                    refraction_instance_buffer.as_ref(),
                    rough_transmission_instance_buffer.as_ref(),
                    emissive_instance_buffer.as_ref(),
                    emissive_cutout_instance_buffer.as_ref(),
                    background_occluded_lit_emissive_instance_buffer.as_ref(),
                    transparent_single_instance_buffer.as_ref(),
                ],
            );
            if let Some(cached) = self
                .command_reuse
                .scenes
                .get(&scene_view_key)
                .filter(|entry| entry.key == key)
            {
                let commands = cached.commands.clone();
                #[cfg(test)]
                if let (Some(profile), Some(draws)) = (self.profile.as_mut(), cached.draws.as_ref())
                {
                    profile.replay_draws(draws);
                }
                self.command_reuse.stats.hits += 1;
                self.command_reuse.scene_scratch = key.resources;
                visual_world.mark_deformations_clean();
                profile_cpu_stage!(self, cpu_stage_clock, "graphics_cache_lookup");
                return Ok(ViewCommands {
                    preceding: preceding_commands,
                    final_commands: commands,
                });
            }
            scene_key = Some(key);
            #[cfg(test)]
            {
                scene_draws_before = self.profile.as_ref().map(|p| p.snapshot_draws());
            }
            cbb = AutoCommandBufferBuilder::primary(
                self.command_buffer_allocator.clone(),
                queue.queue_family_index(),
                CommandBufferUsage::SimultaneousUse,
            )?;
        }

        cbb = self.record_graphics_commands(
            cbb,
            visual_world,
            GraphicsRecording {
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
            },
            &mut preceding_commands,
            #[cfg(test)]
            &mut cpu_stage_clock,
        )?;
        visual_world.mark_deformations_clean();
        profile_cpu_stage!(self, cpu_stage_clock, "record_finish");

        let cb = cbb.build()?;
        if scene_candidate {
            self.command_reuse
                .scene_primed
                .insert(scene_view_key, scene_attachments);
        }
        if let Some(key) = scene_key {
            #[cfg(test)]
            let draws = self
                .profile
                .as_ref()
                .zip(scene_draws_before)
                .map(|(p, before)| p.draw_difference(before));
            self.command_reuse.scenes.insert(
                scene_view_key,
                RetainedScene {
                    key: key.clone(),
                    commands: cb.clone(),
                    #[cfg(test)]
                    draws,
                },
            );
            self.command_reuse.scene_scratch = key.resources;
            self.command_reuse.stats.rebuilds += 1;
        }
        if let Some(images) = prime_bloom {
            let view = self.retained_uploads.borrow().active.unwrap();
            self.command_reuse.primed.insert(view, images);
        }

        profile_cpu_stage!(self, cpu_stage_clock, "command_buffer_build");

        Ok(ViewCommands {
            preceding: preceding_commands,
            final_commands: cb,
        })
    }
}
