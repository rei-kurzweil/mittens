//! Apply one tracked frame to the existing XR component drivers.
use super::*;

impl OpenXRSystem {
    /// Supply an externally owned frame; `None` returns to the engine-owned driver.
    pub fn set_external_frame(
        &mut self,
        frame: Option<super::super::external_xr_frame::ExternalXrFrame>,
    ) {
        self.external_frame = frame;
    }
    /// Like `tick`, but also queues transform updates for registered ControllerXRComponents.
    ///
    /// Head and controller poses share the sample located by `prepare_frame`.
    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        _input: &InputState,
        emit: &mut dyn SignalEmitter,
        _dt_sec: f32,
    ) {
        // Validity is frame-local. Consumers must not solve against stale/default XR transforms.
        for component in self.input_xr_components.iter().copied().collect::<Vec<_>>() {
            if let Some(input) = world.get_component_by_id_as_mut::<InputXRComponent>(component) {
                input.pose_valid = false;
            }
        }
        for component in self
            .controller_components
            .iter()
            .copied()
            .collect::<Vec<_>>()
        {
            if let Some(controller) = world.get_component_by_id_as_mut::<XRHandComponent>(component)
            {
                controller.pose_valid = false;
                controller.active_pose_source = ControllerPoseSource::None;
                controller.raw_aim_rotation = None;
                controller.raw_grip_rotation = None;
            }
        }

        let (head_pose, controllers, hands) = if let Some(frame) = self.external_frame {
            let sample = |pose: Option<openxr::Posef>| {
                pose.map(|pose| ControllerPoseSample {
                    pose,
                    generation: 0,
                })
            };
            (
                frame
                    .views
                    .as_ref()
                    .and_then(|views| Self::derive_head_pose(views)),
                ControllerPoseCache {
                    left_aim: sample(frame.aim[0]),
                    right_aim: sample(frame.aim[1]),
                    left_grip: sample(frame.grip[0]),
                    right_grip: sample(frame.grip[1]),
                },
                HandRootPoseCache::default(),
            )
        } else {
            let Some(sess) = self.state.as_ref().and_then(|state| state.session.as_ref()) else {
                return;
            };
            if !sess.running
                || !matches!(
                    sess.current_state,
                    openxr::SessionState::VISIBLE | openxr::SessionState::FOCUSED
                )
            {
                return;
            }
            (
                sess.head_pose_cache,
                sess.controller_pose_cache,
                sess.hand_root_pose_cache,
            )
        };

        let input_xr_ids: Vec<ComponentId> = self.input_xr_components.iter().copied().collect();
        for input_xr_cid in input_xr_ids {
            let Some(cfg) = world.get_component_by_id_as::<InputXRComponent>(input_xr_cid) else {
                self.input_xr_components.remove(&input_xr_cid);
                continue;
            };

            if !cfg.enabled {
                continue;
            }

            let Some(head_pose) = head_pose else {
                continue;
            };

            Self::drive_head_pose(
                world,
                input_xr_cid,
                head_pose,
                self.floor_correction_meters,
                emit,
            );
        }

        let Some(raw_head) = head_pose else {
            return;
        };
        let Some(rig_world) = Self::tracking_world_frame(world, visuals, raw_head) else {
            return;
        };

        let controller_ids: Vec<ComponentId> = self.controller_components.iter().copied().collect();
        for controller_cid in controller_ids {
            let Some(cfg) = world.get_component_by_id_as::<XRHandComponent>(controller_cid) else {
                self.controller_components.remove(&controller_cid);
                continue;
            };

            if !cfg.enabled {
                continue;
            }

            let (pose, pose_source) =
                Self::preferred_pose(&controllers, &hands, cfg.hand, cfg.pose);
            let (raw_aim, raw_grip) = match cfg.hand {
                ControllerHand::Left => (
                    controllers.left_aim.map(|sample| sample.pose),
                    controllers.left_grip.map(|sample| sample.pose),
                ),
                ControllerHand::Right => (
                    controllers.right_aim.map(|sample| sample.pose),
                    controllers.right_grip.map(|sample| sample.pose),
                ),
            };
            let last_pose_source = self
                .controller_pose_source_last_logged
                .get(&controller_cid)
                .copied();
            if openxr_debug_enabled() && last_pose_source != Some(pose_source) {
                let hand_data_source = match cfg.hand {
                    ControllerHand::Left => hands.left_data_source,
                    ControllerHand::Right => hands.right_data_source,
                };
                eprintln!(
                    "[OpenXR][ctlxr] component={controller_cid:?} hand={:?} pose={:?} source={pose_source:?} hand_data_source={hand_data_source:?}",
                    cfg.hand, cfg.pose
                );
                self.controller_pose_source_last_logged
                    .insert(controller_cid, pose_source);
            }

            if let Some(controller) =
                world.get_component_by_id_as_mut::<XRHandComponent>(controller_cid)
            {
                controller.active_pose_source = pose_source;
                controller.raw_aim_rotation = raw_aim.map(Self::quat_from_posef);
                controller.raw_grip_rotation = raw_grip.map(Self::quat_from_posef);
            }

            let Some(pose) = pose else {
                continue;
            };

            Self::drive_controller_pose(world, controller_cid, pose, rig_world, emit);
        }
    }
}
