use super::*;
#[cfg(test)]
use crate::engine::update_cpu_profile::{Phase, Timer};

impl SystemWorld {
    pub fn tick(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        render_assets: &mut crate::engine::graphics::RenderAssets,
        input: &InputState,
        queue: &mut crate::engine::ecs::CommandQueue,
        dt_sec: f32,
    ) {
        self.tick_with_runtime_session(world, visuals, render_assets, input, queue, None, dt_sec);
    }

    pub(crate) fn tick_with_runtime_session(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        render_assets: &mut crate::engine::graphics::RenderAssets,
        input: &InputState,
        queue: &mut crate::engine::ecs::CommandQueue,
        runtime_session: Option<&mut crate::scripting::runner::RuntimeSpecSession>,
        dt_sec: f32,
    ) {
        if self.vr_perf_enabled {
            self.vr_perf_pre_xr = VrPerfPreXrCpu::default();
        }
        #[cfg(test)]
        let mut cpu_profile = Timer::new();
        visuals.set_window_frame_dt_sec(dt_sec);

        // Drain-point signal graph setup.
        // Per-frame caches are reset here; global handlers are installed idempotently.
        // (Per-gizmo scoped handlers are installed when the gizmo is registered.)
        self.rx.begin_frame();
        SecondaryMotionSystem::install_handlers(&mut self.rx);
        crate::engine::ecs::system::JointBasisRetargetingSystem::install_handlers(&mut self.rx);
        crate::engine::ecs::system::HumanoidBoneMapSystem::install_handlers(&mut self.rx);
        self.gesture.install_handlers(&mut self.rx);
        self.gesture.begin_frame();
        self.text_input.install_handlers(&mut self.rx);
        self.selection.install_handlers(&mut self.rx);
        self.toggle.install_handlers(&mut self.rx);
        self.slider.install_handlers(&mut self.rx);
        self.grabbable.install_handlers(&mut self.rx);
        self.draggable.install_handlers(&mut self.rx);

        // Gameplay keyboard callbacks are delivered in platform order before held-state
        // movement and FrameTick. Text input owns the keyboard while it has focus.
        self.keyboard_input
            .process_input(input, self.text_input.has_focus(), &mut self.rx);
        let _ = self.process_signals(world, visuals, render_assets, queue, 100_000);
        queue.flush(world, self, visuals, render_assets);

        #[cfg(test)]
        cpu_profile.mark(Phase::SystemInput);
        // Process input first - it may queue commands
        let arrows_captured_by_ui =
            self.text_input.has_focus() || self.slider.has_keyboard_focus(world);
        self.input
            .process_input_with_capture(world, input, queue, dt_sec, arrows_captured_by_ui);

        // Spawn any GLTF component trees. This may queue component registrations.
        self.gltf.tick_with_queue(
            world,
            visuals,
            &mut self.skinned_mesh,
            &mut self.renderable,
            queue,
            dt_sec,
        );

        // Bounds, collision inference, and other simulation systems need imported
        // CPU geometry independently of renderer/GPU readiness. Register it as soon
        // as GLTF loading has produced it; prepare_render still owns texture upload,
        // GPU mesh upload, and VisualWorld insertion.
        self.gltf.flush_mesh_imports_only(render_assets);

        // Flush queued registrations/transform updates *before* systems that need current
        // world matrices / acceleration structures (e.g. raycasting).
        queue.flush(world, self, visuals, render_assets);

        // Capture authored bind poses before FrameTick/animation can pose joints.
        self.rigging.reconcile(
            world,
            visuals,
            render_assets,
            &mut self.renderable,
            &mut self.skinned_mesh,
        );
        for owner in self.rigging.take_initialized() {
            self.secondary_motion.armature_initialized(world, owner);
        }

        #[cfg(test)]
        cpu_profile.mark(Phase::GltfImports);
        // Grid hit surfaces participate in raycasting only while Paint owns scene input.
        // Run this every frame so mode transitions and grid create/delete/visibility changes
        // are reflected before the BVH is built and the next pointer activation is sampled.
        let paint_mode = self
            .editor_context
            .shared_state()
            .lock()
            .expect("editor context state mutex poisoned")
            .interaction_mode
            == crate::engine::ecs::component::EditorInteractionMode::Paint;
        self.grid
            .sync_paint_raycast_targets(world, queue, paint_mode);
        queue.flush(world, self, visuals, render_assets);

        #[cfg(test)]
        cpu_profile.mark(Phase::PreFrameFlush);
        // Movement observers see input-driven local transforms from this frame, and run
        // before animation sampling. Avoid allocating/dispatching a per-frame event when
        // no script or system asked for it.
        if self.rx.has_global_handlers(SignalKind::FrameTick) {
            self.rx.push_event(
                ComponentId::default(),
                crate::engine::ecs::EventSignal::FrameTick { dt_sec },
            );
            let _ = self.process_signals(world, visuals, render_assets, queue, 100_000);
            queue.flush(world, self, visuals, render_assets);
        }

        #[cfg(test)]
        cpu_profile.mark(Phase::FrameDispatch);
        self.armature_visualization
            .tick_with_queue(world, &self.gltf, visuals, queue, dt_sec);
        let mesh_bounds_visible = self
            .editor_context
            .shared_state()
            .lock()
            .expect("editor context state mutex poisoned")
            .bounds_visible;
        self.bounds_visualization.tick_with_queue(
            world,
            &mut self.gltf_bounds_visualization,
            &self.gltf,
            &self.mesh_bounds,
            mesh_bounds_visible,
            visuals,
            render_assets,
            queue,
        );
        self.zone_visualization
            .tick_with_queue(world, render_assets, queue);
        self.camera_visualization
            .tick_with_queue(world, &self.camera, queue);
        queue.flush(world, self, visuals, render_assets);

        // Audio clock takeover: once audio output is active, use it as the ClockDriver.
        if let Some(driver) = self.audio.driver() {
            if self.clock.driver_name() != driver.name() {
                self.clock.set_driver(driver);
            }
        }

        self.clock.tick(world, visuals, input, dt_sec);

        // Provide tempo + transport to the audio thread scheduler.
        // ClockSystem may be using AudioClockDriver, so this keeps both timelines aligned.
        self.audio
            .update_transport_from_clock(self.clock.beat_now(), self.clock.bpm());
        self.audio_input.tick(world, &mut self.amplitude);

        #[cfg(test)]
        cpu_profile.mark(Phase::VisualsAudioClock);
        let profile_systems = Self::profile_systems_enabled();
        let phase_started = profile_systems.then(Instant::now);
        if let Some(session) = runtime_session {
            let mut executor =
                crate::engine::ecs::system::animation_keyframe_evaluator::SessionCallbackExecutor::new(
                    session,
                    render_assets,
                    queue,
                    &mut self.benchmarks,
                    &mut self.graphics_settings,
                );
            self.animation.tick_with_beat_and_executor(
                world,
                self.clock.beat_now(),
                self.clock.bpm(),
                &mut self.rx,
                Some(&mut executor),
            );
        } else {
            self.animation.tick_with_beat(
                world,
                self.clock.beat_now(),
                self.clock.bpm(),
                &mut self.rx,
            );
        }
        if let Some(started) = phase_started {
            self.phase_profile.animation += started.elapsed();
        }

        #[cfg(test)]
        cpu_profile.mark(Phase::Animation);
        // Execute/dispatch any signals emitted by AnimationSystem before downstream systems run.
        let _ = self.process_signals(world, visuals, render_assets, queue, 100_000);
        queue.flush(world, self, visuals, render_assets);
        self.tick_transition_runtime(world, visuals);

        #[cfg(test)]
        cpu_profile.mark(Phase::AnimationFlush);
        self.transform_operator.tick(world, visuals, input, dt_sec);

        #[cfg(test)]
        cpu_profile.mark(Phase::TransformOperators);
        // Ensure transforms are propagated before any camera systems consume world matrices.
        let phase_started = profile_systems.then(Instant::now);
        self.transform.tick(world, visuals, input, dt_sec);
        if let Some(started) = phase_started {
            self.phase_profile.transform += started.elapsed();
        }

        #[cfg(test)]
        cpu_profile.mark(Phase::TransformTick);
        // Compute joint palettes from cached world transforms.
        let phase_started = profile_systems.then(Instant::now);
        self.skinned_mesh.tick(world, visuals, input, dt_sec);
        if let Some(started) = phase_started {
            self.phase_profile.skinning += started.elapsed();
        }

        #[cfg(test)]
        cpu_profile.mark(Phase::Skinning);
        // Spatial acceleration structure built from latest world transforms.
        self.bvh.tick(world, visuals, input, dt_sec);

        #[cfg(test)]
        cpu_profile.mark(Phase::Bvh);
        // Integrate outer motion before camera/OpenXR publish this frame's eye
        // poses. Click commands capture the previous published eye at dispatch
        // and become motion on the next fixed substep.
        // Seed sweep history before the first proposed fixed-step pose.
        self.prepare_avatar_capsule_contact(world, visuals);
        for target in self.static_contact.tick_excluding(world, &self.attachment) {
            self.transform_changed(world, visuals, target);
        }
        self.grabbable.prepare_climbing_velocity(world);
        #[cfg(test)]
        cpu_profile.mark(Phase::ContactBeforeVelocity);
        let velocity_steps = self.velocity.take_steps(dt_sec);
        #[cfg(test)]
        crate::engine::update_cpu_profile::velocity_steps(velocity_steps);
        for _ in 0..velocity_steps {
            self.velocity.step(world, queue);
            #[cfg(test)]
            cpu_profile.mark(Phase::VelocityIntegrate);
            queue.flush(world, self, visuals, render_assets);
            #[cfg(test)]
            cpu_profile.mark(Phase::VelocityFlush);
            self.prepare_avatar_capsule_contact(world, visuals);
            for target in self.static_contact.tick_substep(world, &self.attachment) {
                self.transform_changed(world, visuals, target);
            }
            #[cfg(test)]
            cpu_profile.mark(Phase::VelocityContact);
        }

        #[cfg(test)]
        cpu_profile.mark(Phase::VelocityContact);
        // Update window camera + select active XR camera rig before OpenXR consumes it.
        self.camera.tick(world, visuals, input, dt_sec);
        // Locate one presentation sample before head/hand poses and avatar solving.
        // Rendering consumes the prepared frame rather than locating a second sample.
        self.xr
            .prepare_frame(world, visuals, self.xr_settings.floor_correction_meters());
        let phase_started = profile_systems.then(Instant::now);
        self.xr
            .tick_with_queue(world, visuals, input, queue, dt_sec);
        if let Some(started) = phase_started {
            self.phase_profile.xr += started.elapsed();
        }
        #[cfg(test)]
        cpu_profile.mark(Phase::CameraXr);
        // Controller pose updates should be visible to raycasting/gestures this frame.
        queue.flush(world, self, visuals, render_assets);
        self.tick_transition_runtime(world, visuals);
        self.input_xr_gamepad
            .tick_with_queue(world, visuals, &self.xr, queue, dt_sec);
        queue.flush(world, self, visuals, render_assets);
        self.tick_transition_runtime(world, visuals);
        self.prepare_avatar_capsule_contact(world, visuals);
        for target in self.static_contact.tick_excluding(world, &self.attachment) {
            self.transform_changed(world, visuals, target);
        }

        #[cfg(test)]
        cpu_profile.mark(Phase::XrInputContact);
        // Camera-dependent gizmo transforms must settle before pointer rays query the BVH.
        self.xr.publish_eye_cameras(world, visuals);
        // XR wins whenever an active rig has published at least one eye.
        let stereo_active = visuals.active_xr_camera().is_some()
            && visuals
                .visual_camera(crate::engine::graphics::CameraTarget::Xr)
                .is_some_and(|c| !c.eyes.is_empty());
        self.transform_operator
            .set_stereoscopic_active(stereo_active);
        let gizmo_anchors = self.transform_gizmo.update_camera_scales(
            world,
            visuals,
            self.camera.has_active_window_camera(),
        );
        for anchor in gizmo_anchors {
            self.transform_changed(world, visuals, anchor);
        }
        self.bvh.tick(world, visuals, input, dt_sec);

        let activations = self
            .pointer
            .build_activations(world, input, self.xr.xr_input_state());

        self.raycast.tick_with_queue(
            world,
            visuals,
            input,
            &mut self.rx,
            &self.bvh,
            &activations,
            &self.pointer,
            dt_sec,
        );

        // Execute/dispatch any signals produced by raycast immediately (e.g. RayIntersected).
        let _ = self.process_signals(world, visuals, render_assets, queue, 100_000);

        // Gestures interpret ray hits + input into drag events.
        self.attachment.begin_frame();
        self.gesture.tick_with_rx_and_attachments(
            world,
            input,
            &activations,
            &self.pointer,
            &self.raycast,
            &mut self.attachment,
            &mut self.rx,
        );

        // Execute/dispatch gesture-produced signals immediately (e.g. DragStart/DragMove/DragEnd).
        let _ = self.process_signals(world, visuals, render_assets, queue, 100_000);

        self.tick_grip_motion(world, visuals, render_assets, queue, dt_sec);

        // Gizmos consume drag events and apply transform changes.
        self.transform_gizmo
            .tick_with_queue(world, input, queue, &mut self.rx);

        // Execute/dispatch gizmo-produced signals immediately (if any).
        let _ = self.process_signals(world, visuals, render_assets, queue, 100_000);

        // Bridge buffered platform text input after gesture-driven focus changes have landed.
        self.text_input.tick_with_queue(world, input, queue);
        let _ = self.process_signals(world, visuals, render_assets, queue, 100_000);

        // Apply gizmo transform updates immediately so visuals reflect the drag this frame.
        queue.flush(world, self, visuals, render_assets);
        self.tick_transition_runtime(world, visuals);

        self.xr_eye_tracking.tick(world, &mut self.rx);
        let _ = self.process_signals(world, visuals, render_assets, queue, 100_000);

        #[cfg(test)]
        cpu_profile.mark(Phase::RayGestureInput);
        // Avatar body yaw: smoothly rotate body to follow head when yaw diverges.
        // Runs after OpenXR + raycasts + gestures so avatar_driven_t.matrix_world is current.
        let phase_started = profile_systems.then(Instant::now);
        self.avatar_body_yaw.tick(world, queue, dt_sec);
        self.avatar_control.tick(
            world,
            input,
            render_assets,
            &mut self.joint_basis_retargeting,
            &mut self.humanoid_bone_map,
            queue,
            dt_sec,
            &self.transform_operator,
        );
        // AVC can queue head-restoration and first-tick splice transforms. Commit
        // them before body-follow samples the displaced head: body-follow must
        // never consume queued/stale AVC transforms.
        self.lower_body_ik
            .prepare_bindings(world, &self.humanoid_bone_map, queue);
        queue.flush(world, self, visuals, render_assets);
        self.tick_transition_runtime(world, visuals);
        // Native head/neck/body geometry replaces legacy follow for eligible
        // rigs. Publish the coordinated root/neck/head pose before arm IK reads
        // shoulder worlds; unsupported rigs use the mutually exclusive fallback.
        self.head_neck_body_constraint
            .tick(world, queue, &self.attachment);
        queue.flush(world, self, visuals, render_assets);
        self.lower_body_ik.tick(
            world,
            queue,
            &mut self.static_contact,
            &self.attachment,
            &self.humanoid_bone_map,
            dt_sec,
        );
        queue.flush(world, self, visuals, render_assets);
        self.ik.tick(world, queue, dt_sec);
        if let Some(started) = phase_started {
            self.phase_profile.avatar_ik += started.elapsed();
        }
        queue.flush(world, self, visuals, render_assets);
        self.tick_transition_runtime(world, visuals);

        #[cfg(test)]
        cpu_profile.mark(Phase::AvatarIk);
        self.hand_tracking
            .set_openxr_frame(self.xr.hand_tracking_frame());
        self.hand_tracking.tick(
            world,
            &mut self.humanoid_bone_map,
            &self.joint_basis_retargeting,
            queue,
            dt_sec,
        );
        queue.flush(world, self, visuals, render_assets);

        #[cfg(test)]
        cpu_profile.mark(Phase::HandTracking);
        // Post-pose stage: propagate primary pose, solve tails, explicitly propagate
        // the affected spring subtrees, then refresh skin palettes for the frame that
        // will actually be rendered. TransformSystem::tick is event-driven/no-op, so
        // secondary-motion writes must identify the roots that need propagation.
        self.transform.tick(world, visuals, input, dt_sec);
        let vr_stage_started = self.vr_perf_enabled.then(Instant::now);
        let secondary_motion_enabled = self.graphics_settings.secondary_motion_enabled();
        let secondary_motion_paused = self.graphics_settings.secondary_motion_paused();
        if secondary_motion_enabled && !secondary_motion_paused {
            self.secondary_motion_constraint
                .tick(world, &self.secondary_motion, dt_sec);
        } else if !secondary_motion_enabled {
            self.secondary_motion_constraint.reset_motion_history();
        }
        let surfaces = self.static_contact.support_index(world);
        let dirty_chain_transform_roots = self.secondary_motion.tick_with_collisions(
            world,
            dt_sec,
            self.secondary_motion_constraint.directives(),
            secondary_motion_enabled,
            secondary_motion_paused,
            Some(surfaces),
        );
        if let Some(started) = vr_stage_started {
            self.vr_perf_pre_xr.secondary_motion = started.elapsed();
        }
        #[cfg(test)]
        cpu_profile.mark(Phase::SecondaryMotion);
        let vr_stage_started = self.vr_perf_enabled.then(Instant::now);
        for root in dirty_chain_transform_roots {
            self.transform_changed(world, visuals, root);
        }
        if let Some(started) = vr_stage_started {
            self.vr_perf_pre_xr.spring_transform_propagation = started.elapsed();
        }
        #[cfg(test)]
        cpu_profile.mark(Phase::SpringPropagation);
        let vr_stage_started = self.vr_perf_enabled.then(Instant::now);
        if secondary_motion_enabled {
            self.spring_bone_visualization.tick_with_queue(
                world,
                &self.secondary_motion,
                render_assets,
                queue,
            );
        } else {
            self.spring_bone_visualization.suspend(queue);
        }
        if let Some(started) = vr_stage_started {
            self.vr_perf_pre_xr.spring_visualization = started.elapsed();
        }
        let vr_stage_started = self.vr_perf_enabled.then(Instant::now);
        self.skinned_mesh.tick(world, visuals, input, dt_sec);
        if let Some(started) = vr_stage_started {
            self.vr_perf_pre_xr.post_secondary_skinning = started.elapsed();
        }

        if profile_systems {
            self.finish_phase_profile_frame();
        }

        #[cfg(test)]
        cpu_profile.mark(Phase::SpringVisualsSkin);
        // Flex-column position pass: emit UpdateTransform for dirty LayoutComponent subtrees.
        // Runs after transforms are propagated so initial world matrices are valid.
        self.layout.tick(world, queue);
        let vr_stage_started = self.vr_perf_enabled.then(Instant::now);
        queue.flush(world, self, visuals, render_assets);
        if crate::engine::ecs::system::selection_system::refresh_all_existing_selection_overlays(
            world, queue,
        ) {
            queue.flush(world, self, visuals, render_assets);
        }
        if let Some(started) = vr_stage_started {
            self.vr_perf_pre_xr.post_pose_command_flush = started.elapsed();
        }

        self.fit_bounds.tick(world, render_assets, queue);

        // Remeasure any pending preview shells whose styled content now has
        // layout-generated background quads (RenderableComponents) available.
        self.asset_system
            .remeasure_pending_previews(world, render_assets, queue);
        // Flush immediately so the UpdateTransform intent (centering + scale)
        // takes effect on this frame, not the next.
        queue.flush(world, self, visuals, render_assets);

        #[cfg(test)]
        cpu_profile.mark(Phase::LayoutTail);
        self.renderable.tick(world, visuals, input, dt_sec);

        self.renderer_stats
            .tick_with_queue(world, visuals, queue, dt_sec);

        self.text.tick(world, visuals, input, dt_sec);

        self.zone_observation
            .tick(world, &self.attachment, &mut self.rx);
        // Deliver observer events before the runtime services this frame's callbacks.
        let _ = self.process_signals(world, visuals, render_assets, queue, 100_000);

        self.light.tick(world, visuals, input, dt_sec);
        self.graphics_settings.publish(visuals);
        // Mount/AVC/ancestor changes above are reflected before mirror captures.
        self.xr.publish_eye_cameras(world, visuals);
        self.mirror.tick(world, visuals, input, dt_sec);
        for texture_component in self.mirror.take_pending_texture_registrations() {
            self.register_texture(world, visuals, texture_component);
        }
        #[cfg(test)]
        cpu_profile.mark(Phase::VisualPublication);
    }
}
