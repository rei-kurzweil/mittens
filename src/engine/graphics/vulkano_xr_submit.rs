// XR target selection and stereo queue submission.
impl VulkanoState {
    // Separate recording from swapchain acquisition so the complete scene
    // path can also be exercised with owned stereo images in GPU tests.
    fn submit_xr_eye_to_view(
        &mut self,
        visual_world: &mut VisualWorld,
        eye: usize,
        extent: [u32; 2],
        view_count: usize,
        output_view: Arc<ImageView>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(test)]
        let mut cpu_stage_clock = self.profile.as_ref().map(|_| std::time::Instant::now());
        let color_format = output_view.format();
        if color_format != self.scene_color_format() {
            return Err("XR color format does not match the scene pipelines".into());
        }

        let post_process_config = visual_world.effective_post_processing();
        let post_process = if post_process_config.is_active() {
            self.post_processing_renderer.ensure_xr_targets(
                view_count,
                extent,
                color_format,
                self.msaa_samples,
                &post_process_config,
            )?;
            let targets = self
                .post_processing_renderer
                .xr_frame_targets(eye)
                .ok_or("missing XR post-processing targets")?
                .clone();
            Some(PostProcessInvocation {
                final_output_view: output_view.clone(),
                final_color_format: color_format,
                config: post_process_config,
                targets,
            })
        } else {
            self.ensure_xr_eye_attachments(view_count, extent)?;
            None
        };

        let (color_attachment_view, color_resolve_view, depth_view) =
            if let Some(post) = post_process.as_ref() {
                (
                    post.targets
                        .main_msaa_color
                        .clone()
                        .unwrap_or_else(|| post.targets.main_color.clone()),
                    post.targets
                        .main_msaa_color
                        .as_ref()
                        .map(|_| post.targets.main_color.clone()),
                    post.targets.depth.clone(),
                )
            } else {
                let targets = self
                    .xr_eye_attachments
                    .as_ref()
                    .ok_or("XR eye attachments missing")?;
                let depth_view = targets
                    .depth_views
                    .get(eye)
                    .ok_or("XR depth eye out of range")?
                    .clone();
                if self.msaa_samples != SampleCount::Sample1 {
                    let msaa_view = targets
                        .msaa_color_views
                        .get(eye)
                        .ok_or("XR MSAA color eye out of range")?
                        .clone();
                    (msaa_view, Some(output_view), depth_view)
                } else {
                    (output_view, None, depth_view)
                }
            };

        let xr_camera = visual_world
            .visual_camera(crate::engine::graphics::CameraTarget::Xr)
            .ok_or("missing XR camera")?;
        let eye_data = xr_camera
            .eyes
            .get(eye)
            .ok_or("XR camera eye out of range")?;
        let render_view = RenderView {
            view: eye_data.view,
            proj: eye_data.proj,
            viewport: [extent[0] as f32, extent[1] as f32],
            kind: RenderViewKind::XrEye { eye },
        };

        let window_slots = self.window_frame_slots();
        let bones_slots_total = window_slots + view_count;
        let bones_slot = window_slots + eye;

        profile_cpu_stage!(self, cpu_stage_clock, "xr_target_camera_setup");

        let cb = self.build_draw_batches_command_buffer(
            visual_world,
            &render_view,
            bones_slot,
            bones_slots_total,
            color_attachment_view,
            color_resolve_view,
            depth_view,
            extent,
            post_process,
            None,
            None,
            None,
        )?;

        #[cfg(test)]
        if let Some(clock) = cpu_stage_clock.as_mut() {
            *clock = std::time::Instant::now();
        }

        let device = self.context.device().clone();
        let queue = self.context.graphics_queue().clone();
        let prior = self
            .submission_future
            .take()
            .unwrap_or_else(|| sync::now(device).boxed());
        let future = prior
            .then_execute(queue, cb)?
            .then_signal_fence_and_flush()?;
        profile_cpu_stage!(self, cpu_stage_clock, "queue_submit_flush");

        self.perf_queue_submissions += 1;
        self.perf_xr_eyes += 1;
        #[cfg(test)]
        if let Some(profile) = self.profile.as_mut() {
            let future = Arc::new(future);
            let completion = future.clone();
            profile.completion = Some(Box::new(move || completion.wait(None).map_err(Into::into)));
            self.submission_future = Some(future.boxed());
            return Ok(());
        }
        self.submission_future = Some(future.boxed());

        Ok(())
    }
}
