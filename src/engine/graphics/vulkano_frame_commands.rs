// Per-view resource preparation and scene command recording.
impl VulkanoState {
    fn build_draw_batches_command_buffer(
        &mut self,
        visual_world: &mut VisualWorld,
        render_view: &RenderView,
        bones_slot: usize,
        bones_slots_total: usize,
        color_attachment_view: Arc<ImageView>,
        color_resolve_view: Option<Arc<ImageView>>,
        depth_view: Arc<ImageView>,
        extent: [u32; 2],
        post_process: Option<PostProcessInvocation>,
        runtime_texture_publication: Option<(TextureHandle, Arc<ImageView>)>,
        scene_snapshot: Option<RefractionSnapshotViews>,
        rough_transmission_pyramid: Option<RoughTransmissionPyramidViews>,
    ) -> Result<Arc<vulkano::command_buffer::PrimaryAutoCommandBuffer>, Box<dyn std::error::Error>>
    {
        #[cfg(test)]
        let mut cpu_stage_clock = self.profile.as_ref().map(|_| std::time::Instant::now());

        // Captures can publish replacement image views earlier this frame.
        self.retire_runtime_resources(visual_world);
        self.apply_pending_runtime_texture_updates();
        self.sync_material_instances(visual_world);
        Self::retarget_mirror_surface_textures_for_render_view(visual_world, &render_view.kind);

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

        let queue = self.context.graphics_queue().clone();
        let mesh_vertex_counts: HashMap<_, _> = self
            .meshes
            .iter()
            .map(|(handle, mesh)| (*handle, mesh.vertex_count))
            .collect();
        visual_world.sync_deformation_ranges(&mesh_vertex_counts);

        // Always rebuild draw cache cheaply.
        let draw_cache_rebuilt = visual_world.prepare_draw_cache();

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
        let bones_palette_dirty = !dirty_bones_ranges.is_empty();

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
        let refraction_instance_buffer =
            self.build_instance_buffer_for_order_opt(&*visual_world, refraction_instances)?;

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
        let rough_transmission_instance_buffer =
            self.build_instance_buffer_for_order_opt(&*visual_world, rough_transmission_instances)?;
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
            let buf =
                self.build_instance_buffer_for_order_or_dummy(&*visual_world, &opaque_instances)?;

            self.cached_instance_count = instance_count;
            self.cached_instance_buffer = Some(buf.clone());
            buf
        };

        let need_outline_instance_buffer = instance_data_dirty
            || draw_cache_rebuilt
            || self.cached_outline_instance_buffer.is_none()
            || self.cached_outline_instance_count != outline_instance_count;
        let outline_instance_buffer = if !need_outline_instance_buffer {
            self.cached_outline_instance_buffer.clone()
        } else {
            let buffer =
                self.build_instance_buffer_for_order_opt(&*visual_world, outline_instances)?;
            self.cached_outline_instance_count = outline_instance_count;
            self.cached_outline_instance_buffer = buffer.clone();
            buffer
        };

        let need_background_instance_buffer = instance_data_dirty
            || draw_cache_rebuilt
            || self.cached_background_instance_count != background_instance_count;
        let background_instance_buffer = if !need_background_instance_buffer {
            self.cached_background_instance_buffer.clone()
        } else {
            let buf = self.build_instance_buffer_for_order_opt(
                &*visual_world,
                visual_world.background_order(),
            )?;
            self.cached_background_instance_count = background_instance_count;
            self.cached_background_instance_buffer = buf.clone();
            buf
        };

        let need_background_occluded_lit_instance_buffer = instance_data_dirty
            || draw_cache_rebuilt
            || self.cached_background_occluded_lit_instance_count
                != background_occluded_lit_instance_count;
        let background_occluded_lit_instance_buffer =
            if !need_background_occluded_lit_instance_buffer {
                self.cached_background_occluded_lit_instance_buffer.clone()
            } else {
                let buf = self.build_instance_buffer_for_order_opt(
                    &*visual_world,
                    visual_world.background_occluded_lit_order(),
                )?;
                self.cached_background_occluded_lit_instance_count =
                    background_occluded_lit_instance_count;
                self.cached_background_occluded_lit_instance_buffer = buf.clone();
                buf
            };

        let need_cutout_instance_buffer = instance_data_dirty
            || draw_cache_rebuilt
            || self.cached_cutout_instance_count != cutout_instance_count;
        let cutout_instance_buffer = if !need_cutout_instance_buffer {
            self.cached_cutout_instance_buffer.clone()
        } else {
            let buf =
                self.build_instance_buffer_for_order_opt(&*visual_world, &cutout_instances)?;
            self.cached_cutout_instance_count = cutout_instance_count;
            self.cached_cutout_instance_buffer = buf.clone();
            buf
        };

        let need_overlay_instance_buffer = instance_data_dirty
            || draw_cache_rebuilt
            || self.cached_overlay_instance_count != overlay_instance_count;
        // Overlay buffer indexed by overlay_stream().1 so batch.start offsets are correct.
        let overlay_instance_buffer = if !need_overlay_instance_buffer {
            self.cached_overlay_instance_buffer.clone()
        } else {
            let buf =
                self.build_instance_buffer_for_order_opt(&*visual_world, &overlay_instances)?;
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
            visual_world.emissive_draw_order(),
        )?;
        let background_occluded_lit_emissive_instance_count =
            visual_world.background_occluded_lit_emissive_order().len();
        let background_occluded_lit_emissive_instance_buffer =
            if background_occluded_lit_emissive_instance_count > 0 {
                self.build_instance_buffer_for_order_opt(
                    &*visual_world,
                    visual_world.background_occluded_lit_emissive_order(),
                )?
            } else {
                None
            };

        let emissive_cutout_instance_buffer = self.build_instance_buffer_for_order_opt(
            &*visual_world,
            visual_world.emissive_cutout_order(),
        )?;

        profile_cpu_stage!(self, cpu_stage_clock, "instance_buffers");

        let clear_color = visual_world.clear_color();
        let defer_overlay_until_before_final_composite =
            post_process.is_some() && overlay_instance_count > 0;
        let composite_bloom_before_refraction = sample_scene_color
            && post_process
                .as_ref()
                .is_some_and(|post| post.config.bloom.is_some());
        let mut blurred_bloom: Option<Arc<ImageView>> = None;

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

        let camera_buffer_fg: Subbuffer<CameraUBO> = Buffer::from_data(
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
            camera_ubo_fg,
        )?;

        // Lights storage buffer (set=0, binding=1).
        let lights_ssbo = LightsSSBO::from_lights(visual_world.lights());

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
        let global_set_fg = DescriptorSet::new(
            self.descriptor_set_allocator.clone(),
            self.set_layouts.global.clone(),
            [
                WriteDescriptorSet::buffer(0, camera_buffer_fg.clone()),
                WriteDescriptorSet::buffer(1, lights_buffer.clone()),
                WriteDescriptorSet::image_view_sampler(
                    2,
                    fallback_scene_color.clone(),
                    self.sampler_scene_color.clone(),
                ),
            ],
            [],
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

            let camera_buffer_bg: Subbuffer<CameraUBO> = Buffer::from_data(
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
                camera_ubo_bg,
            )?;

            let set = DescriptorSet::new(
                self.descriptor_set_allocator.clone(),
                self.set_layouts.global.clone(),
                [
                    WriteDescriptorSet::buffer(0, camera_buffer_bg),
                    WriteDescriptorSet::buffer(1, lights_buffer.clone()),
                    WriteDescriptorSet::image_view_sampler(
                        2,
                        fallback_scene_color,
                        self.sampler_scene_color.clone(),
                    ),
                ],
                [],
            )?;
            Some(set)
        };

        profile_cpu_stage!(self, cpu_stage_clock, "camera_lights_global_descriptors");

        let mut cbb = AutoCommandBufferBuilder::primary(
            self.command_buffer_allocator.clone(),
            queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )?;
        profile_cpu_stage!(self, cpu_stage_clock, "builder_create");

        #[cfg(test)]
        if let Some(profile) = self.profile.as_ref() {
            profile.timestamp(&mut cbb, eye, 0)?;
        }
        #[cfg(test)]
        let deform_started = self.profile.as_ref().map(|_| std::time::Instant::now());
        let deformation_output =
            self.record_dirty_deformations(&mut cbb, visual_world, &dirty_bones_ranges)?;
        #[cfg(test)]
        if let Some(profile) = self.profile.as_mut() {
            profile.timestamp(&mut cbb, eye, 1)?;
            profile.deformation_cpu_ns += deform_started.unwrap().elapsed().as_nanos() as u64;
        }

        profile_cpu_stage!(self, cpu_stage_clock, "deformation_record");

        // Rig descriptor set (set=2): persistent deformation cache + placeholder lighting.
        // Layout is defined in `PipelineDescriptorSetLayouts::rig`.
        let rig_set: Arc<DescriptorSet> = {
            static DID_LOG_BONES_PALETTE_UPLOAD: AtomicBool = AtomicBool::new(false);

            let want_len = visual_world.bones_palette().len().max(1);

            let want_slots = bones_slots_total.max(1);
            let slot = bones_slot.min(want_slots - 1);

            let needs_realloc = false;

            if needs_realloc {
                let new_cap = want_len.next_power_of_two().max(1);

                let mut buffers = Vec::with_capacity(want_slots);
                for _ in 0..want_slots {
                    let buffer: Subbuffer<[GpuMat4]> = Buffer::new_slice(
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
                        new_cap as DeviceSize,
                    )?;
                    buffers.push(buffer);
                }

                self.cached_bones_buffers = buffers;
                self.cached_bones_slot_valid = vec![false; want_slots];
                self.cached_bones_capacity = new_cap;
            }

            if bones_palette_dirty {
                for v in self.cached_bones_slot_valid.iter_mut() {
                    *v = false;
                }
            }

            // Graphics-stage bones are retired; binding 1 now receives deformation_output.
            let slot_needs_upload = false;

            if slot_needs_upload {
                let bones_src = visual_world.bones_palette();

                if env_flag("CAT_DEBUG_BONES_PALETTE")
                    && bones_src.len() > 1
                    && !DID_LOG_BONES_PALETTE_UPLOAD.swap(true, Ordering::Relaxed)
                {
                    println!(
                        "[VulkanoRenderer] bones palette upload: dirty={} realloc={} want_len={} cached_cap={} src_len={}",
                        bones_palette_dirty,
                        needs_realloc,
                        want_len,
                        self.cached_bones_capacity,
                        bones_src.len()
                    );

                    for (i, m) in bones_src.iter().take(3).enumerate() {
                        println!("  bone[{i:03}]={m:?}");
                    }
                }

                let identity = [
                    [1.0, 0.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0, 0.0],
                    [0.0, 0.0, 1.0, 0.0],
                    [0.0, 0.0, 0.0, 1.0],
                ];

                let mut dst = self.cached_bones_buffers[slot].write()?;

                // Write current palette, then fill remainder with identity.
                if bones_src.is_empty() {
                    dst[0] = GpuMat4 { cols: identity };
                    for slot in dst.iter_mut().skip(1) {
                        *slot = GpuMat4 { cols: identity };
                    }
                } else {
                    for (i, m) in bones_src.iter().copied().enumerate() {
                        dst[i] = GpuMat4 { cols: m };
                    }
                    for slot in dst.iter_mut().skip(bones_src.len()) {
                        *slot = GpuMat4 { cols: identity };
                    }
                }

                if let Some(v) = self.cached_bones_slot_valid.get_mut(slot) {
                    *v = true;
                }
            }

            let per_instance_lighting_buffer: Subbuffer<DummyPerInstanceLightingSSBO> =
                Buffer::from_data(
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
                    DummyPerInstanceLightingSSBO::default(),
                )?;

            DescriptorSet::new(
                self.descriptor_set_allocator.clone(),
                self.set_layouts.rig.clone(),
                [
                    WriteDescriptorSet::buffer(0, per_instance_lighting_buffer),
                    WriteDescriptorSet::buffer(1, deformation_output),
                ],
                [],
            )?
        };

        profile_cpu_stage!(self, cpu_stage_clock, "rig_buffer_descriptor");

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

        profile_cpu_stage!(self, cpu_stage_clock, "outline_record");

        self.record_opaque_draws(
            &mut cbb,
            visual_world,
            &global_set_fg,
            &rig_set,
            &instance_buffer,
            instance_count,
            Some((opaque_ops, opaque_instances)),
        )?;

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
            eye,
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
            if !composite_bloom_before_refraction {
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

        profile_cpu_stage!(self, cpu_stage_clock, "postprocess_record");

        if let (Some(handle), Some(debug_instance_buffer)) = (
            stencil_clip_debug_handle,
            stencil_clip_debug_instance_buffer.as_ref(),
        ) {
            self.ensure_window_runtime_debug_targets(
                bones_slots_total,
                extent,
                color_attachment_view.image().format(),
            )?;

            let debug_view = self
                .window_runtime_debug_targets
                .as_ref()
                .and_then(|targets| targets.color_views.get(bones_slot))
                .cloned()
                .ok_or("missing window runtime debug target")?;

            let debug_msaa_view = self
                .window_runtime_debug_targets
                .as_ref()
                .and_then(|targets| targets.msaa_color_views.get(bones_slot))
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

        profile_cpu_stage!(self, cpu_stage_clock, "debug_record");

        #[cfg(test)]
        if let Some(profile) = self.profile.as_ref() {
            profile.timestamp(&mut cbb, eye, 2)?;
        }
        visual_world.mark_deformations_clean();
        profile_cpu_stage!(self, cpu_stage_clock, "record_finish");

        let cb = cbb.build()?;

        profile_cpu_stage!(self, cpu_stage_clock, "command_buffer_build");

        Ok(cb)
    }
}
