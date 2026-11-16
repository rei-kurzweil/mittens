//! Eye gaze: direct trackers, head-motion freeze policy, limits, rest restore.
use super::*;

/// Apply the newest direct-child eye-tracker sample to each independently
/// mapped eye. Eye gaze is an absolute rest-relative pose, never a delta from
/// the pose written on the preceding frame.
pub(super) fn update_eye_tracking(
    avc_id: ComponentId,
    world: &mut World,
    humanoid_maps: &mut HumanoidBoneMapSystem,
    emit: &mut dyn SignalEmitter,
    dt_sec: f32,
) {
    let report = humanoid_maps
        .request_for_avc(world, avc_id)
        .and_then(|gltf| humanoid_maps.report(gltf))
        .cloned();
    let (left_target, right_target) = report.as_ref().map_or((None, None), |report| {
        (
            map_target(report, HumanoidSlot::LeftEye),
            map_target(report, HumanoidSlot::RightEye),
        )
    });
    let (left_gaze, right_gaze) = newest_direct_eye_gaze(world, avc_id);
    let (left_gaze, right_gaze) = (
        left_gaze.map(clamp_eye_gaze_rotation),
        right_gaze.map(clamp_eye_gaze_rotation),
    );
    let (left_gaze, right_gaze) = apply_head_motion_gaze_policy(
        avc_id,
        world,
        left_target,
        right_target,
        left_gaze,
        right_gaze,
        dt_sec,
    );
    update_one_eye_tracking(avc_id, world, emit, true, left_target, left_gaze);
    update_one_eye_tracking(avc_id, world, emit, false, right_target, right_gaze);
    update_eye_blink(avc_id, world, humanoid_maps, dt_sec);
}

pub(super) const EYE_GAZE_FREEZE_ENTER_RAD_PER_SEC: f32 = 30.0_f32.to_radians();
pub(super) const EYE_GAZE_FREEZE_EXIT_RAD_PER_SEC: f32 = 25.0_f32.to_radians();
pub(super) const EYE_GAZE_FREEZE_RELEASE_SEC: f32 = 0.10;

/// Freeze the last good head-relative gaze while the mapped eye-parent basis
/// rotates quickly. This deliberately suppresses intentional eye movement
/// during rapid head turns; it is a visual fallback for unstable tracking, not
/// timestamped sample alignment.
pub(super) fn apply_head_motion_gaze_policy(
    avc_id: ComponentId,
    world: &mut World,
    left_target: Option<ComponentId>,
    right_target: Option<ComponentId>,
    left: Option<ResolvedEyeGaze>,
    right: Option<ResolvedEyeGaze>,
    dt_sec: f32,
) -> (Option<ResolvedEyeGaze>, Option<ResolvedEyeGaze>) {
    let basis = [left_target, right_target]
        .into_iter()
        .flatten()
        .find_map(|eye| {
            world.parent_of(eye).and_then(|parent| {
                world
                    .get_component_by_id_as::<TransformComponent>(parent)
                    .map(|transform| mat_to_quat(transform.transform.matrix_world))
            })
        });
    let Some(basis) = basis else {
        return (left, right);
    };
    let Some(avc) = world.get_component_by_id_as_mut::<AvatarControlComponent>(avc_id) else {
        return (left, right);
    };
    if avc.head_motion_gaze_policy != HeadMotionGazePolicy::Freeze {
        avc.last_eye_gaze_basis_rotation = Some(basis);
        avc.eye_gaze_frozen = false;
        avc.eye_gaze_still_time_sec = 0.0;
        avc.frozen_left_eye_gaze = left.map(|gaze| gaze.direction);
        avc.frozen_right_eye_gaze = right.map(|gaze| gaze.direction);
        return (left, right);
    }

    let angular_speed = avc.last_eye_gaze_basis_rotation.map_or(0.0, |previous| {
        let delta = quat_mul(quat_conjugate(previous), basis);
        // q and -q encode the same orientation. Use |w| so a harmless sign
        // choice from matrix-to-quaternion conversion never appears as a
        // 360° head turn.
        let angle = 2.0 * delta[3].abs().clamp(0.0, 1.0).acos();
        angle / dt_sec.max(1e-4)
    });
    avc.last_eye_gaze_basis_rotation = Some(basis);

    if !avc.eye_gaze_frozen && angular_speed >= EYE_GAZE_FREEZE_ENTER_RAD_PER_SEC {
        avc.eye_gaze_frozen = true;
        avc.eye_gaze_still_time_sec = 0.0;
    }
    if !avc.eye_gaze_frozen {
        avc.frozen_left_eye_gaze = left.map(|gaze| gaze.direction);
        avc.frozen_right_eye_gaze = right.map(|gaze| gaze.direction);
        return (left, right);
    }

    if angular_speed <= EYE_GAZE_FREEZE_EXIT_RAD_PER_SEC {
        avc.eye_gaze_still_time_sec += dt_sec.max(0.0);
        if avc.eye_gaze_still_time_sec >= EYE_GAZE_FREEZE_RELEASE_SEC {
            avc.eye_gaze_frozen = false;
            avc.eye_gaze_still_time_sec = 0.0;
            avc.frozen_left_eye_gaze = left.map(|gaze| gaze.direction);
            avc.frozen_right_eye_gaze = right.map(|gaze| gaze.direction);
            return (left, right);
        }
    } else {
        avc.eye_gaze_still_time_sec = 0.0;
    }

    (
        freeze_gaze(left, avc.frozen_left_eye_gaze),
        freeze_gaze(right, avc.frozen_right_eye_gaze),
    )
}

pub(super) fn freeze_gaze(
    live: Option<ResolvedEyeGaze>,
    frozen_direction: Option<[f32; 3]>,
) -> Option<ResolvedEyeGaze> {
    frozen_direction.map(|direction| ResolvedEyeGaze {
        direction,
        // A frozen vector is the already-limited, head-relative gaze from the
        // prior frame. Do not apply absolute-basis compensation to it.
        compensation: HeadRotationCompensation::Off,
        rotation_limits: live.and_then(|gaze| gaze.rotation_limits),
        sequence: live.map_or(0, |gaze| gaze.sequence),
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ResolvedEyeGaze {
    pub(super) direction: [f32; 3],
    pub(super) compensation: HeadRotationCompensation,
    pub(super) rotation_limits: Option<EyeRotationLimits>,
    pub(super) sequence: u64,
}

/// Clamp raw head-local gaze with independent yaw and pitch caps. Transport
/// samples intentionally remain unchanged; this is an AVC rig-pose policy.
pub(super) fn clamp_eye_gaze_rotation(mut gaze: ResolvedEyeGaze) -> ResolvedEyeGaze {
    let Some(limits) = gaze.rotation_limits else {
        return gaze;
    };
    let [x, y, z] = gaze.direction;
    let horizontal = (x * x + z * z).sqrt();
    let yaw = x.atan2(-z);
    let pitch = y.atan2(horizontal);
    let yaw = if yaw < 0.0 {
        yaw.max(-limits.left)
    } else {
        yaw.min(limits.right)
    };
    let pitch = if pitch < 0.0 {
        pitch.max(-limits.down)
    } else {
        pitch.min(limits.up)
    };
    let cos_pitch = pitch.cos();
    gaze.direction = [yaw.sin() * cos_pitch, pitch.sin(), -yaw.cos() * cos_pitch];
    gaze
}

pub(super) fn newest_direct_eye_gaze(
    world: &mut World,
    avc_id: ComponentId,
) -> (Option<ResolvedEyeGaze>, Option<ResolvedEyeGaze>) {
    let mut left = None::<ResolvedEyeGaze>;
    let mut right = None::<ResolvedEyeGaze>;
    for child in world.children_of(avc_id).iter().copied() {
        let sample = world
            .get_component_by_id_as::<XREyeTrackingComponent>(child)
            .map(|tracker| {
                (
                    tracker.enable_pupil_direction_tracking,
                    tracker.gaze_sample,
                    tracker.head_rotation_compensation,
                    tracker.rotation_limits,
                    tracker.rotation_limits_per_eye,
                )
            })
            .or_else(|| {
                world
                    .get_component_by_id_as::<VRChatOSCEyeTrackingComponent>(child)
                    .map(|tracker| {
                        (
                            tracker.enable_pupil_direction_tracking,
                            tracker.gaze_sample,
                            tracker.head_rotation_compensation,
                            tracker.rotation_limits,
                            tracker.rotation_limits_per_eye,
                        )
                    })
            })
            .or_else(|| {
                world
                    .get_component_by_id_as::<XREyeTrackingHtcComponent>(child)
                    .map(|tracker| {
                        (
                            tracker.enable_pupil_direction_tracking,
                            tracker.gaze_sample,
                            tracker.head_rotation_compensation,
                            tracker.rotation_limits,
                            tracker.rotation_limits_per_eye,
                        )
                    })
            });
        let Some((enabled, sample, compensation, shared_limits, per_eye_limits)) = sample else {
            continue;
        };
        // This gates only AVC's eye-bone ownership. Closure samples remain
        // independently available to `update_eye_blink` above.
        if !enabled {
            continue;
        }
        if let Some(gaze) = sample.left.filter(valid_gaze) {
            if left.is_none_or(|current| sample.sequence > current.sequence) {
                left = Some(ResolvedEyeGaze {
                    direction: gaze,
                    compensation,
                    rotation_limits: crate::engine::ecs::component::combined_eye_rotation_limits(
                        shared_limits,
                        per_eye_limits[0],
                    ),
                    sequence: sample.sequence,
                });
            }
        }
        if let Some(gaze) = sample.right.filter(valid_gaze) {
            if right.is_none_or(|current| sample.sequence > current.sequence) {
                right = Some(ResolvedEyeGaze {
                    direction: gaze,
                    compensation,
                    rotation_limits: crate::engine::ecs::component::combined_eye_rotation_limits(
                        shared_limits,
                        per_eye_limits[1],
                    ),
                    sequence: sample.sequence,
                });
            }
        }
    }
    (left, right)
}

pub(super) fn valid_gaze(gaze: &[f32; 3]) -> bool {
    gaze.iter().all(|value| value.is_finite())
        && gaze.iter().map(|value| value * value).sum::<f32>() > 1e-12
}

pub(super) fn update_one_eye_tracking(
    avc_id: ComponentId,
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    left: bool,
    target: Option<ComponentId>,
    gaze: Option<ResolvedEyeGaze>,
) {
    let previous = world
        .get_component_by_id_as::<AvatarControlComponent>(avc_id)
        .and_then(|avc| {
            if left {
                avc.left_eye_tracking_bone_id
            } else {
                avc.right_eye_tracking_bone_id
            }
        });
    // A map refresh can replace a target. Restore the old bone before giving
    // the new one ownership, even if the new tracker sample remains valid.
    if previous.is_some() && previous != target {
        restore_eye_rest(world, emit, previous.unwrap());
    }
    let active = target.zip(gaze);
    if let Some((bone, gaze)) = active {
        let (_, rest_rotation, _) = read_bone_rest_pose(world, bone);
        let gaze = gaze_in_eye_parent_basis(world, bone, gaze);
        let correction = shortest_arc_quat([0.0, 0.0, -1.0], gaze);
        update_local_rotation(world, emit, bone, quat_mul(correction, rest_rotation));
    } else if let Some(bone) = previous {
        restore_eye_rest(world, emit, bone);
    }
    if let Some(avc) = world.get_component_by_id_as_mut::<AvatarControlComponent>(avc_id) {
        let owned = active.map(|(bone, _)| bone);
        if left {
            avc.left_eye_tracking_bone_id = owned;
        } else {
            avc.right_eye_tracking_bone_id = owned;
        }
    }
}

/// Convert a world-relative tracker direction into the local coordinates in
/// which the eye-bone rotation is written.  For head-relative transports the
/// raw direction remains byte-for-byte unchanged.
pub(super) fn gaze_in_eye_parent_basis(
    world: &World,
    bone: ComponentId,
    gaze: ResolvedEyeGaze,
) -> [f32; 3] {
    if gaze.compensation == HeadRotationCompensation::Off {
        return gaze.direction;
    }
    let Some(parent) = world.parent_of(bone) else {
        return gaze.direction;
    };
    let Some(parent_transform) = world.get_component_by_id_as::<TransformComponent>(parent) else {
        return gaze.direction;
    };
    quat_rotate_vec3(
        quat_conjugate(mat_to_quat(parent_transform.transform.matrix_world)),
        gaze.direction,
    )
}

pub(super) fn restore_eye_rest(world: &World, emit: &mut dyn SignalEmitter, bone: ComponentId) {
    let (_, rotation, _) = read_bone_rest_pose(world, bone);
    update_local_rotation(world, emit, bone, rotation);
}
