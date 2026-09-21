use crate::engine::ecs::component::HumanoidSlot;
use crate::engine::ecs::component::{
    AvatarControlComponent, BoneRestPoseComponent, Camera3DComponent, CameraXRComponent,
    CollisionComponent, CollisionResponseComponent, CollisionShape, CollisionShapeComponent,
    ControllerHand, ControllerPoseSource, ControllerXRComponent, EyeRotationLimits, GLTFComponent,
    HeadMotionGazePolicy, HeadRotationCompensation, IKChainComponent, IKSolver, InputXRComponent,
    QuatYawFollowComponent, SerializeComponent, TransformComponent, TransformDropComponent,
    TransformForkTRSComponent, TransformMapRotationComponent, TransformMapScaleComponent,
    TransformMapTranslationComponent, VRChatOSCEyeTrackingComponent, XREyeTrackingComponent,
    XREyeTrackingHtcComponent,
};
use crate::engine::ecs::component::{
    QueryRootMode, is_level_provider, resolve_component_ref, retained_level_sample,
};
use crate::engine::ecs::system::bounds_system::{BoundsSystem, RenderableBoundsMeasure};
use crate::engine::ecs::system::collision_shape_inference::infer_upright_capsule;
use crate::engine::ecs::system::input_xr_gamepad_system::xr_locomotion_target_transform;
use crate::engine::ecs::system::{
    HumanoidBoneMapReport, HumanoidBoneMapSystem, JointBasisRetargetingSystem, LandmarkDirection,
    RetargetBasisDefinition, RetargetBasisStatus,
};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::{RenderAssets, primitives::Transform};
use crate::engine::user_input::InputState;
use crate::utils::math::{
    mat_to_quat, mat4_identity, mat4_mul, quat_conjugate, quat_mul, quat_rotate_vec3,
    quat_rotation_y, quat_to_axis_angle, shortest_arc_quat,
};
use std::collections::HashSet;
use std::sync::OnceLock;

#[derive(Debug, Default)]
pub struct AvatarControlSystem {
    avatars: HashSet<ComponentId>,
    pending_capsule_diagnostics: HashSet<ComponentId>,
    alignment_diagnostic_tick: u64,
}

impl AvatarControlSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tick(
        &mut self,
        world: &mut World,
        _input: &InputState,
        render_assets: &RenderAssets,
        retargeting: &mut JointBasisRetargetingSystem,
        humanoid_maps: &mut HumanoidBoneMapSystem,
        emit: &mut dyn SignalEmitter,
        dt_sec: f32,
    ) {
        let ids: Vec<_> = self.avatars.iter().copied().collect();

        self.alignment_diagnostic_tick = self.alignment_diagnostic_tick.wrapping_add(1);
        let log_alignment =
            hand_alignment_debug_enabled() && self.alignment_diagnostic_tick % 120 == 0;
        for id in ids {
            if self.pending_capsule_diagnostics.remove(&id) {
                log_settled_capsule_diagnostics(id, world);
            }
            let capsule_before = world
                .get_component_by_id_as::<AvatarControlComponent>(id)
                .and_then(|avc| avc.capsule_transform_id);
            tick_one(
                id,
                world,
                render_assets,
                retargeting,
                humanoid_maps,
                emit,
                dt_sec,
                log_alignment,
            );
            let capsule_after = world
                .get_component_by_id_as::<AvatarControlComponent>(id)
                .and_then(|avc| avc.capsule_transform_id);
            if capsule_before.is_none() && capsule_after.is_some() {
                self.pending_capsule_diagnostics.insert(id);
            }
        }
    }

    pub fn register(&mut self, component: ComponentId) {
        self.avatars.insert(component);
    }

    pub fn remove(&mut self, component: ComponentId) {
        self.avatars.remove(&component);
        self.pending_capsule_diagnostics.remove(&component);
    }
}

fn tick_one(
    id: ComponentId,
    world: &mut World,
    render_assets: &RenderAssets,
    retargeting: &mut JointBasisRetargetingSystem,
    humanoid_maps: &mut HumanoidBoneMapSystem,
    emit: &mut dyn SignalEmitter,
    dt_sec: f32,
    log_alignment: bool,
) {
    // --- Init phase ---
    let needs_init = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
            return;
        };
        c.head_mount.is_none()
    };

    if needs_init {
        // Runtime splicing reparents and rewrites avatar bones, so it is itself a
        // pose-changing operation. An XR-authored avatar must remain in its authored
        // pose until the headset has supplied a valid pose. Non-XR AVC trees continue
        // to initialize immediately.
        if !ancestor_input_xr_is_ready(world, id) {
            return;
        }
        let Some(gltf) = humanoid_maps.request_for_avc(world, id) else {
            return;
        };
        let Some(report) = humanoid_maps.report(gltf).cloned() else {
            return;
        };
        if !report.head_ready() {
            return;
        }
        try_init_splices(id, world, retargeting, &report, emit);
    }

    refresh_map_camera_anchor(id, world, humanoid_maps, emit);

    try_init_or_route_capsule(id, world, render_assets, emit);

    // Keep the displaced head bone anchored under head_mount. This prevents
    // animation/FK from reintroducing a local head translation that would move
    // the camera wrapper relative to the solved head pivot.
    let displaced_head_id = world
        .get_component_by_id_as::<AvatarControlComponent>(id)
        .and_then(|c| c.displaced_head);
    if let Some(head_bone_id) = displaced_head_id {
        if let Some(head_t) = world.get_component_by_id_as::<TransformComponent>(head_bone_id) {
            if head_t.transform.translation != [0.0, 0.0, 0.0] {
                emit.push_intent_now(
                    head_bone_id,
                    IntentValue::UpdateTransform {
                        component_id: head_bone_id,
                        translation: [0.0, 0.0, 0.0],
                        rotation_quat_xyzw: head_t.transform.rotation,
                        scale: head_t.transform.scale,
                    },
                );
            }
        }
    }

    update_hand_pose_corrections(id, world, emit, log_alignment);
    update_eye_tracking(id, world, humanoid_maps, emit, dt_sec);
    update_amplitude_mouth_open(id, world, humanoid_maps, dt_sec);
}

/// Drive AVC's named amplitude fallback without touching the primary morph
/// driver. A future/live viseme driver therefore has deterministic priority.
fn update_amplitude_mouth_open(
    avc_id: ComponentId,
    world: &mut World,
    humanoid_maps: &mut HumanoidBoneMapSystem,
    dt_sec: f32,
) {
    let (authored, cached, floor, ceiling, amount, smoothing, previous) = {
        let Some(avc) = world.get_component_by_id_as::<AvatarControlComponent>(avc_id) else {
            return;
        };
        (
            avc.mouth_open_amplitude.clone(),
            avc.resolved_mouth_open_amplitude,
            avc.mouth_open_rms_floor,
            avc.mouth_open_rms_ceiling,
            avc.mouth_open_amount,
            avc.mouth_open_smoothing,
            avc.mouth_open_weight,
        )
    };
    let Some(authored) = authored else { return };
    let source = cached
        .filter(|&id| is_level_provider(world, id))
        .or_else(|| {
            resolve_component_ref(world, &authored, Some(avc_id), QueryRootMode::WorldRoot)
        });
    let source = source.filter(|&id| is_level_provider(world, id));
    let target = source
        .and_then(|id| retained_level_sample(world, id))
        .map_or(0.0, |sample| {
            ((sample.rms - floor) / (ceiling - floor)).clamp(0.0, 1.0) * amount
        });
    let alpha = if smoothing <= 0.0 {
        1.0
    } else {
        1.0 - (-smoothing * dt_sec.max(0.0)).exp()
    };
    let weight = previous + (target - previous) * alpha;
    if let Some(avc) = world.get_component_by_id_as_mut::<AvatarControlComponent>(avc_id) {
        avc.resolved_mouth_open_amplitude = source;
        avc.mouth_open_weight = if weight.is_finite() {
            weight.clamp(0.0, 1.0)
        } else {
            0.0
        };
    }

    let Some(gltf_id) = humanoid_maps.request_for_avc(world, avc_id) else {
        return;
    };
    let label = world.children_of(gltf_id).iter().copied().find_map(|id| {
        world
            .get_component_by_id_as::<crate::engine::ecs::component::MorphTargetMapComponent>(id)
            .and_then(|map| map.slots().get("viseme_aa").cloned())
    });
    let matched_keys: Vec<_> = label.as_deref().map_or_else(Vec::new, |label| {
        world
            .get_component_by_id_as::<GLTFComponent>(gltf_id)
            .map(|gltf| {
                gltf.morph_targets
                    .iter()
                    .filter(|info| info.label.as_deref() == Some(label))
                    .map(|info| info.key)
                    .collect()
            })
            .unwrap_or_default()
    });
    if let Some(gltf) = world.get_component_by_id_as_mut::<GLTFComponent>(gltf_id) {
        for state in gltf.morph_factors.values_mut() {
            state.amplitude_mouth_open = None;
        }
        for key in &matched_keys {
            if let Some(state) = gltf.morph_factors.get_mut(key) {
                state.amplitude_mouth_open = Some(weight.clamp(0.0, 1.0));
            }
        }
    }
    if matched_keys.is_empty() {
        let should_log = world
            .get_component_by_id_as::<AvatarControlComponent>(avc_id)
            .is_some_and(|avc| !avc.mouth_open_missing_slot_diagnosed);
        if should_log {
            eprintln!(
                "[AVC][mouth_open_from_amplitude] avc={avc_id:?} missing or unresolved viseme_aa morph slot"
            );
            if let Some(avc) = world.get_component_by_id_as_mut::<AvatarControlComponent>(avc_id) {
                avc.mouth_open_missing_slot_diagnosed = true;
            }
        }
    }
}

/// Apply the newest direct-child eye-tracker sample to each independently
/// mapped eye. Eye gaze is an absolute rest-relative pose, never a delta from
/// the pose written on the preceding frame.
fn update_eye_tracking(
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
    update_eye_blink(avc_id, world, humanoid_maps);
}

const EYE_GAZE_FREEZE_ENTER_RAD_PER_SEC: f32 = 30.0_f32.to_radians();
const EYE_GAZE_FREEZE_EXIT_RAD_PER_SEC: f32 = 25.0_f32.to_radians();
const EYE_GAZE_FREEZE_RELEASE_SEC: f32 = 0.10;

/// Freeze the last good head-relative gaze while the mapped eye-parent basis
/// rotates quickly. This deliberately suppresses intentional eye movement
/// during rapid head turns; it is a visual fallback for unstable tracking, not
/// timestamped sample alignment.
fn apply_head_motion_gaze_policy(
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

fn freeze_gaze(
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

/// The newest normalized closure for each eye owns its mapped blink target
/// while present. The base value is never overwritten, so removing/loss of a
/// tracker restores editor and imported defaults automatically.
fn update_eye_blink(
    avc_id: ComponentId,
    world: &mut World,
    humanoid_maps: &mut HumanoidBoneMapSystem,
) {
    let closure = newest_direct_eye_closure(world, avc_id);
    let Some(gltf_id) = humanoid_maps.request_for_avc(world, avc_id) else {
        return;
    };
    apply_eye_blink_drivers(world, gltf_id, closure);
}

fn newest_direct_eye_closure(world: &World, avc_id: ComponentId) -> (Option<f32>, Option<f32>) {
    let mut left = None::<(u64, f32)>;
    let mut right = None::<(u64, f32)>;
    for id in world.children_of(avc_id).iter().copied() {
        let sample = world
            .get_component_by_id_as::<XREyeTrackingComponent>(id)
            .map(|tracker| tracker.closure_sample)
            .or_else(|| {
                world
                    .get_component_by_id_as::<VRChatOSCEyeTrackingComponent>(id)
                    .map(|tracker| tracker.closure_sample)
            })
            .or_else(|| {
                world
                    .get_component_by_id_as::<XREyeTrackingHtcComponent>(id)
                    .map(|tracker| tracker.closure_sample)
            });
        let Some(sample) = sample else { continue };
        if let Some(value) = sample.left {
            if left.is_none_or(|(sequence, _)| sample.sequence > sequence) {
                left = Some((sample.sequence, value));
            }
        }
        if let Some(value) = sample.right {
            if right.is_none_or(|(sequence, _)| sample.sequence > sequence) {
                right = Some((sample.sequence, value));
            }
        }
    }
    (left.map(|(_, value)| value), right.map(|(_, value)| value))
}

fn apply_eye_blink_drivers(
    world: &mut World,
    gltf_id: ComponentId,
    closure: (Option<f32>, Option<f32>),
) {
    let labels = world.children_of(gltf_id).iter().copied().find_map(|id| {
        world
            .get_component_by_id_as::<crate::engine::ecs::component::MorphTargetMapComponent>(id)
            .map(|map| map.slots().clone())
    });
    let Some(labels) = labels else { return };
    let Some(gltf) =
        world.get_component_by_id_as_mut::<crate::engine::ecs::component::GLTFComponent>(gltf_id)
    else {
        return;
    };
    for (channel, closure) in [
        ("left_eye_blink", closure.0),
        ("right_eye_blink", closure.1),
    ] {
        let Some(label) = labels.get(channel) else {
            continue;
        };
        for info in gltf
            .morph_targets
            .iter()
            .filter(|info| info.label.as_deref() == Some(label.as_str()))
        {
            if let Some(state) = gltf.morph_factors.get_mut(&info.key) {
                state.driver = closure;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedEyeGaze {
    direction: [f32; 3],
    compensation: HeadRotationCompensation,
    rotation_limits: Option<EyeRotationLimits>,
    sequence: u64,
}

/// Clamp raw head-local gaze with independent yaw and pitch caps. Transport
/// samples intentionally remain unchanged; this is an AVC rig-pose policy.
fn clamp_eye_gaze_rotation(mut gaze: ResolvedEyeGaze) -> ResolvedEyeGaze {
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

fn newest_direct_eye_gaze(
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

fn valid_gaze(gaze: &[f32; 3]) -> bool {
    gaze.iter().all(|value| value.is_finite())
        && gaze.iter().map(|value| value * value).sum::<f32>() > 1e-12
}

fn update_one_eye_tracking(
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
fn gaze_in_eye_parent_basis(world: &World, bone: ComponentId, gaze: ResolvedEyeGaze) -> [f32; 3] {
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

fn restore_eye_rest(world: &World, emit: &mut dyn SignalEmitter, bone: ComponentId) {
    let (_, rotation, _) = read_bone_rest_pose(world, bone);
    update_local_rotation(world, emit, bone, rotation);
}

fn refresh_map_camera_anchor(
    avc_id: ComponentId,
    world: &mut World,
    humanoid_maps: &mut HumanoidBoneMapSystem,
    emit: &mut dyn SignalEmitter,
) {
    let Some(gltf) = humanoid_maps.request_for_avc(world, avc_id) else {
        return;
    };
    let Some(report) = humanoid_maps.report(gltf) else {
        return;
    };
    let (old_anchor, seen_generation) = world
        .get_component_by_id_as::<AvatarControlComponent>(avc_id)
        .map(|avc| (avc.splice_camera_bone, avc.humanoid_map_generation))
        .unwrap_or((None, 0));
    if seen_generation == report.generation {
        return;
    }
    let new_anchor = map_target(report, HumanoidSlot::CameraAnchor)
        .or_else(|| map_target(report, HumanoidSlot::Head));
    if let (Some(old), Some(new)) = (old_anchor, new_anchor) {
        if old != new {
            let camera_paths: Vec<_> = world
                .children_of(old)
                .iter()
                .copied()
                .filter(|child| subtree_has_camera(world, *child))
                .collect();
            for path in camera_paths {
                emit_attach(emit, new, path);
            }
        }
    }
    if let Some(avc) = world.get_component_by_id_as_mut::<AvatarControlComponent>(avc_id) {
        avc.splice_camera_bone = new_anchor;
        avc.humanoid_map_gltf = Some(gltf);
        avc.humanoid_map_generation = report.generation;
    }
}

fn subtree_has_camera(world: &World, root: ComponentId) -> bool {
    let mut pending = vec![root];
    while let Some(component) = pending.pop() {
        if world
            .get_component_by_id_as::<Camera3DComponent>(component)
            .is_some()
            || world
                .get_component_by_id_as::<CameraXRComponent>(component)
                .is_some()
        {
            return true;
        }
        pending.extend(world.children_of(component).iter().copied());
    }
    false
}

fn try_init_or_route_capsule(
    avc_id: ComponentId,
    world: &mut World,
    render_assets: &RenderAssets,
    emit: &mut dyn SignalEmitter,
) {
    let Some(avc) = world
        .get_component_by_id_as::<AvatarControlComponent>(avc_id)
        .cloned()
    else {
        return;
    };
    if !avc.collision_enabled {
        return;
    }

    let model_root_id = avc.model_root_id.or_else(|| {
        world.children_of(avc_id).iter().copied().find(|child| {
            world
                .get_component_by_id_as::<TransformComponent>(*child)
                .is_some()
        })
    });
    let Some(model_root_id) = model_root_id else {
        return;
    };
    let movement_target = automatic_avc_movement_target(world, avc_id);

    if let Some(response_id) = avc.capsule_response_id {
        if let Some(response) =
            world.get_component_by_id_as_mut::<CollisionResponseComponent>(response_id)
        {
            response.movement_target_id = movement_target;
            response.movement_target_required = true;
        }
        return;
    }

    let measured_bounds =
        BoundsSystem::measure_renderable_subtree_bounds(world, render_assets, model_root_id);
    let measured_inference = match measured_bounds {
        RenderableBoundsMeasure::Measured(bounds) => {
            infer_upright_capsule(&bounds, avc.capsule_radius).map(|inferred| (bounds, inferred))
        }
        RenderableBoundsMeasure::Unmeasurable => None,
    };
    let (bounds_source, inference_bounds, inferred) =
        if let Some((bounds, inferred)) = measured_inference {
            ("aggregate_render_bounds", bounds, inferred)
        } else {
            let Some(height) = spawned_gltf_exists(world, model_root_id)
                .then(|| fallback_avatar_height(world, avc_id, model_root_id, &avc))
                .flatten()
            else {
                return;
            };
            let bounds = crate::engine::graphics::bounds::Aabb {
                min: [0.0, 0.0, 0.0],
                max: [0.0, height.max(0.0), 0.0],
            };
            let Some(inferred) = infer_upright_capsule(&bounds, avc.capsule_radius) else {
                return;
            };
            ("fallback_avatar_height", bounds, inferred)
        };
    let (radius, half_segment) = match inferred.shape {
        CollisionShape::CapsuleY {
            radius,
            half_segment,
        } => (radius, half_segment),
        _ => unreachable!("upright capsule inference returned a non-capsule shape"),
    };
    let half_height = radius + half_segment;
    eprintln!(
        "[AVC][capsule][inferred] avc={avc_id:?} model_root={model_root_id:?} source={bounds_source} bounds_min={:?} bounds_max={:?} center_y={:.6} radius={radius:.6} half_segment={half_segment:.6} local_bottom={:.6} local_top={:.6} movement_target={movement_target:?}",
        inference_bounds.min,
        inference_bounds.max,
        inferred.center_y,
        inferred.center_y - half_height,
        inferred.center_y + half_height,
    );

    let fork = world.add_component(TransformForkTRSComponent::new());
    let translation = world.add_component(TransformMapTranslationComponent::new());
    let rotation = world.add_component(TransformMapRotationComponent::new());
    let rotation_drop = world.add_component(TransformDropComponent::new());
    let scale = world.add_component(TransformMapScaleComponent::new());
    let scale_drop = world.add_component(TransformDropComponent::new());
    let capsule_t =
        world.add_component(TransformComponent::new().with_position(0.0, inferred.center_y, 0.0));
    let serialize = world.add_component(SerializeComponent::off());
    let collision = world.add_component(CollisionComponent::KINEMATIC());
    let shape = world.add_component(CollisionShapeComponent::new(inferred.shape));
    let response = world.add_component(
        CollisionResponseComponent::slide().with_runtime_movement_target(movement_target),
    );

    let _ = world.set_parent(fork, Some(model_root_id));
    let _ = world.set_parent(translation, Some(fork));
    let _ = world.set_parent(rotation, Some(fork));
    let _ = world.set_parent(rotation_drop, Some(rotation));
    let _ = world.set_parent(scale, Some(fork));
    let _ = world.set_parent(scale_drop, Some(scale));
    let _ = world.set_parent(capsule_t, Some(fork));
    let _ = world.set_parent(serialize, Some(capsule_t));
    let _ = world.set_parent(collision, Some(capsule_t));
    let _ = world.set_parent(shape, Some(collision));
    let _ = world.set_parent(response, Some(collision));

    if let Some(avc) = world.get_component_by_id_as_mut::<AvatarControlComponent>(avc_id) {
        avc.model_root_id = Some(model_root_id);
        avc.capsule_transform_id = Some(capsule_t);
        avc.capsule_response_id = Some(response);
    }
    emit.push_intent_now(
        collision,
        IntentValue::RegisterCollision {
            component_id: collision,
        },
    );
    emit.push_intent_now(
        response,
        IntentValue::RegisterCollisionResponse {
            component_id: response,
        },
    );
}

fn log_settled_capsule_diagnostics(avc_id: ComponentId, world: &World) {
    let Some(avc) = world.get_component_by_id_as::<AvatarControlComponent>(avc_id) else {
        return;
    };
    let Some(capsule_transform) = avc.capsule_transform_id else {
        return;
    };
    let Some(collision) = world
        .children_of(capsule_transform)
        .iter()
        .copied()
        .find(|id| {
            world
                .get_component_by_id_as::<CollisionComponent>(*id)
                .is_some()
        })
    else {
        return;
    };
    let Some(shape) = world.children_of(collision).iter().find_map(|id| {
        world
            .get_component_by_id_as::<CollisionShapeComponent>(*id)
            .map(|shape| shape.shape)
    }) else {
        return;
    };
    let CollisionShape::CapsuleY {
        radius,
        half_segment,
    } = shape
    else {
        return;
    };
    let local_center_y = world
        .get_component_by_id_as::<TransformComponent>(capsule_transform)
        .map(|transform| transform.transform.translation[1]);
    let world_center =
        crate::engine::ecs::system::TransformSystem::world_position(world, capsule_transform);
    let world_extents = world_center.map(|center| {
        let half_height = radius + half_segment;
        (center[1] - half_height, center[1] + half_height)
    });
    eprintln!(
        "[AVC][capsule][settled] avc={avc_id:?} capsule_transform={capsule_transform:?} collision={collision:?} local_center_y={local_center_y:?} world_center={world_center:?} radius={radius:.6} half_segment={half_segment:.6} world_bottom_top={world_extents:?}"
    );
}

fn automatic_avc_movement_target(world: &World, avc_id: ComponentId) -> Option<ComponentId> {
    let mut current = Some(avc_id);
    while let Some(id) = current {
        if world
            .get_component_by_id_as::<InputXRComponent>(id)
            .is_some()
        {
            return xr_locomotion_target_transform(world, id);
        }
        current = world.parent_of(id);
    }
    world.parent_of(avc_id).filter(|id| {
        world
            .get_component_by_id_as::<TransformComponent>(*id)
            .is_some()
    })
}

fn spawned_gltf_exists(world: &World, root: ComponentId) -> bool {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if world
            .get_component_by_id_as::<GLTFComponent>(id)
            .is_some_and(|gltf| gltf.spawned)
        {
            return true;
        }
        stack.extend(world.children_of(id).iter().copied());
    }
    false
}

fn fallback_avatar_height(
    world: &World,
    _avc_id: ComponentId,
    model_root_id: ComponentId,
    avc: &AvatarControlComponent,
) -> Option<f32> {
    if let Some(height) = avc.avatar_height {
        return Some(height.max(0.0));
    }
    let bone = avc.splice_camera_bone.or(avc.displaced_head)?;
    let root_y = world
        .get_component_by_id_as::<TransformComponent>(model_root_id)?
        .transform
        .matrix_world[3][1];
    let bone_y = world
        .get_component_by_id_as::<TransformComponent>(bone)?
        .transform
        .matrix_world[3][1];
    Some((bone_y - root_y).abs())
}

fn ancestor_input_xr_is_ready(world: &World, start: ComponentId) -> bool {
    let mut current = Some(start);
    while let Some(component) = current {
        if let Some(input) = world
            .get_component_by_id_as::<crate::engine::ecs::component::InputXRComponent>(component)
        {
            return input.pose_valid;
        }
        current = world.parent_of(component);
    }
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ArmIkBinding {
    controller: ComponentId,
    raw_target: ComponentId,
    upper_arm: ComponentId,
    lower_arm: ComponentId,
    hand: ComponentId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArmIkEligibility {
    Eligible(ArmIkBinding),
    IncompleteArmMap,
    NoHandDriver,
    MalformedHandDriver,
}

/// Classify one AVC arm without mutating the world.
///
/// A complete humanoid arm is only a capability. AVC-owned arm IK activates when
/// that capability also has an enabled, direct hand pose driver with the tracked
/// transform topology expected by `OpenXRSystem`.
fn classify_arm_ik_eligibility(
    world: &World,
    humanoid_map: &HumanoidBoneMapReport,
    controller: Option<ComponentId>,
    left: bool,
) -> ArmIkEligibility {
    let (upper_slot, lower_slot, hand_slot) = if left {
        (
            HumanoidSlot::LeftUpperArm,
            HumanoidSlot::LeftLowerArm,
            HumanoidSlot::LeftHand,
        )
    } else {
        (
            HumanoidSlot::RightUpperArm,
            HumanoidSlot::RightLowerArm,
            HumanoidSlot::RightHand,
        )
    };
    let (Some(upper_arm), Some(lower_arm), Some(hand)) = (
        map_target(humanoid_map, upper_slot),
        map_target(humanoid_map, lower_slot),
        map_target(humanoid_map, hand_slot),
    ) else {
        return ArmIkEligibility::IncompleteArmMap;
    };
    let Some(controller) = controller else {
        return ArmIkEligibility::NoHandDriver;
    };
    let Some(config) = world.get_component_by_id_as::<ControllerXRComponent>(controller) else {
        return ArmIkEligibility::MalformedHandDriver;
    };
    if !config.enabled {
        return ArmIkEligibility::NoHandDriver;
    }
    let Some(raw_target) = world
        .children_of(controller)
        .iter()
        .copied()
        .find(|&child| {
            world
                .get_component_by_id_as::<TransformComponent>(child)
                .is_some()
        })
    else {
        return ArmIkEligibility::MalformedHandDriver;
    };
    ArmIkEligibility::Eligible(ArmIkBinding {
        controller,
        raw_target,
        upper_arm,
        lower_arm,
        hand,
    })
}

/// First-time setup: splice bones, create body pipeline, and (optionally) hand smoothing pipelines.
///
/// Controllers are discovered by topology: any `ControllerXRComponent` that is a
/// **direct child** of this `AvatarControlComponent` is treated as a hand driver.
/// Its `hand` field (`Left` / `Right`) determines which hand bone it drives.
///
/// Body pipeline created here reads `driven_t`'s world matrix, strips pitch/roll via `YawFollow`,
/// and writes the result to `model_root` (which is re-parented under the pipeline output).
fn try_init_splices(
    id: ComponentId,
    world: &mut World,
    retargeting: &mut JointBasisRetargetingSystem,
    humanoid_map: &HumanoidBoneMapReport,
    emit: &mut dyn SignalEmitter,
) {
    let (
        left_arm_pole_direction,
        right_arm_pole_direction,
        body_yaw_threshold,
        body_yaw_rate,
        authored_forward_plus_z,
        forward_plus_z_overridden,
        authored_initial_body_yaw,
        initial_body_yaw_overridden,
        skip_body_pipeline,
        avatar_height_override,
        eye_height_from_head_bone,
        head_ik_eye_height,
        neck_pin_enabled,
    ) = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
            return;
        };
        (
            c.left_arm_pole_direction,
            c.right_arm_pole_direction,
            c.body_yaw_threshold,
            c.body_yaw_rate,
            c.forward_plus_z,
            c.forward_plus_z_overridden,
            c.initial_body_yaw,
            c.initial_body_yaw_overridden,
            c.skip_body_pipeline,
            c.avatar_height,
            c.eye_height_from_head_bone,
            c.head_ik_eye_height,
            c.neck_pin_enabled,
        )
    };

    // Find model_root: first TransformComponent child of AvatarControlComponent.
    let Some(model_root_id) = world.children_of(id).iter().copied().find(|&ch| {
        world
            .get_component_by_id_as::<TransformComponent>(ch)
            .is_some()
    }) else {
        return;
    };

    // Discover hand controllers by topology: direct ControllerXRComponent children.
    let left_ctrl = world.children_of(id).iter().copied().find(|&ch| {
        world
            .get_component_by_id_as::<ControllerXRComponent>(ch)
            .map(|c| c.hand == ControllerHand::Left)
            .unwrap_or(false)
    });
    let right_ctrl = world.children_of(id).iter().copied().find(|&ch| {
        world
            .get_component_by_id_as::<ControllerXRComponent>(ch)
            .map(|c| c.hand == ControllerHand::Right)
            .unwrap_or(false)
    });

    // driven_t is the parent of AVC and owns the generated visible-head mount.
    let Some(driven_t_id) = world.parent_of(id) else {
        return;
    };
    let resolved_body_forward_plus_z = if forward_plus_z_overridden {
        authored_forward_plus_z
    } else {
        false
    };
    let resolved_head_target_forward_plus_z = if forward_plus_z_overridden {
        authored_forward_plus_z
    } else {
        false
    };
    let resolved_initial_body_yaw = if initial_body_yaw_overridden {
        authored_initial_body_yaw
    } else {
        std::f32::consts::PI
    };

    // Head bone is required — retry next tick if GLTF hasn't spawned yet.
    let Some(head_bone_id) = map_target(humanoid_map, HumanoidSlot::Head) else {
        return;
    };
    // Read head_bone's true bind-pose local TRS via the `BoneRestPoseComponent`
    // sidecar that `GLTFSystem` stamped at node-spawn time.  Falls back to the
    // current `TransformComponent` only if no rest-pose sidecar is present
    // (non-GLTF skeletons).  Reading the live `TransformComponent` would
    // pick up whatever pose `AnimationSystem` wrote earlier this tick, which
    // bakes the current animation frame into `head_rest_rot` and produces a
    // permanently rotated visible head.
    let (_, head_rest_rot, head_rest_s) = read_bone_rest_pose(world, head_bone_id);

    // A mapped arm is only a model capability. Build a target and arm chain only
    // when this AVC also owns a usable per-side hand pose driver.
    let left_eligibility = classify_arm_ik_eligibility(world, humanoid_map, left_ctrl, true);
    let right_eligibility = classify_arm_ik_eligibility(world, humanoid_map, right_ctrl, false);
    let mut prepare_arm = |eligibility: ArmIkEligibility, left: bool, side: &str| {
        let ArmIkEligibility::Eligible(binding) = eligibility else {
            if eligibility == ArmIkEligibility::MalformedHandDriver {
                eprintln!(
                    "[AVC] {side} arm IK disabled: XRHand must have a direct tracked Transform child"
                );
            }
            return Some(None);
        };
        ensure_map_hand_basis(world, retargeting, humanoid_map, left);
        let correction = derive_hand_aim_correction(
            world,
            retargeting,
            model_root_id,
            Some(binding.hand),
            side,
        )?;
        Some(Some((
            resolve_hand_target(world, binding, correction),
            correction,
        )))
    };
    let Some(left) = prepare_arm(left_eligibility, true, "left") else {
        return;
    };
    let Some(right) = prepare_arm(right_eligibility, false, "right") else {
        return;
    };

    // --- Camera bone: auto-calibrate model_root.y + discover camera children ---
    //
    // Priority:
    //   1. avatar_height_override — use directly, skip bone measurement.
    //   2. camera_bone auto-calibration — measure bone local Y in rest pose.
    // Either way, emit UpdateTransform(model_root, y = -height).
    //
    // Any Camera3D or CameraXR direct children of AVC are re-parented under the
    // camera bone so they inherit its world transform each tick.
    let camera_bone_id =
        map_target(humanoid_map, HumanoidSlot::CameraAnchor).or(Some(head_bone_id));

    // Discover camera children + derive eye_offset_head_local FIRST — the
    // model_root xz compensation below needs the offset, and the eye_offset
    // also feeds the head IK target_position_offset (used much later).
    let camera_children: Vec<(ComponentId, [f32; 3], bool)> = world
        .children_of(id)
        .iter()
        .copied()
        .filter_map(|ch| {
            let is_c3d = world
                .get_component_by_id_as::<Camera3DComponent>(ch)
                .is_some();
            let is_cxr = world
                .get_component_by_id_as::<CameraXRComponent>(ch)
                .is_some();
            if is_c3d || is_cxr {
                return Some((ch, [0.0, 0.0, 0.0], is_c3d));
            }
            if let Some(tc) = world.get_component_by_id_as::<TransformComponent>(ch) {
                let wraps_c3d = world.children_of(ch).iter().any(|&gc| {
                    world
                        .get_component_by_id_as::<Camera3DComponent>(gc)
                        .is_some()
                });
                let wraps_cxr = world.children_of(ch).iter().any(|&gc| {
                    world
                        .get_component_by_id_as::<CameraXRComponent>(gc)
                        .is_some()
                });
                let wraps_cam = wraps_c3d || wraps_cxr;
                if wraps_cam {
                    let eye_offset = tc.transform.translation;
                    return Some((ch, eye_offset, wraps_c3d));
                }
            }
            None
        })
        .collect();
    let eye_offset_head_local: [f32; 3] = camera_children
        .iter()
        .map(|&(_, off, _)| off)
        .find(|off| off != &[0.0, 0.0, 0.0])
        .unwrap_or([0.0, eye_height_from_head_bone.unwrap_or(0.0), 0.0]);

    // Eye offset mapped from head-local into driven_t-local space.
    // This remains the source for the head target offset. It no longer owns
    // body/root XZ placement; steady-state body XZ is handled by
    // HeadPoseBodyXzFollowSystem.
    let head_ik_offset_yaw = if resolved_head_target_forward_plus_z {
        0.0
    } else {
        std::f32::consts::PI
    };

    // Body Y is anchored to `displaced_head.world.y` (which already has
    // -eye_offset.y baked in via the head_target chain) in
    // HeadPoseBodyXzFollowSystem, so model_root.y must NOT also include an
    // eye-offset term — that would subtract it twice and stretch the
    // rest-pose neck by `eye_offset.y`.
    let model_root_translation: Option<[f32; 3]> = if let Some(h) = avatar_height_override {
        Some([0.0, -h, 0.0])
    } else if let Some(cam_bone_id) = camera_bone_id {
        // AVC initialization waits for a valid XR pose. At that boundary the
        // XR-driven parent and freshly imported GLTF descendants can have
        // cached world matrices from different propagation stages, so
        // subtracting their world Y values can move the body up to the HMD.
        //
        // Derive avatar height entirely from the immutable GLTF rest-pose
        // locals instead. This is independent of headset position, animation,
        // startup poses, and transform-system tick ordering.
        rest_model_relative_to(world, model_root_id, cam_bone_id)
            .map(|bone_in_model| [0.0, -bone_in_model[3][1], 0.0])
    } else {
        None
    };

    // model_root baseline calibration plus authored eye offset compensation.
    // This moves the whole avatar relative to the fixed XR camera pose.  The
    // initial UpdateTransform sets the body in roughly the right place before
    // SimpleHumanoidSystem takes over translation each tick.
    if let Some(txyz) = model_root_translation {
        emit.push_intent_now(
            model_root_id,
            IntentValue::UpdateTransform {
                component_id: model_root_id,
                translation: txyz,
                rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0, 1.0],
            },
        );
    }

    // Resolve neck bone (for the Phase 2 rest-pin) and cache its rest local
    // translation from the `BoneRestPoseComponent` sidecar — same reasoning
    // as the head_rest read above: the live `TransformComponent` would
    // already carry whatever animation wrote this tick.
    let (neck_bone_id, neck_rest_t) = if neck_pin_enabled {
        match map_target(humanoid_map, HumanoidSlot::Neck) {
            Some(nid) => {
                let (rest_t, _, _) = read_bone_rest_pose(world, nid);
                (Some(nid), Some(rest_t))
            }
            None => (None, None),
        }
    } else {
        (None, None)
    };

    // Y component of model_root.local stashed for the body-follow system's
    // future Step 1 (head-rotation-compensated world XZ target).  Step 0
    // doesn't use it; the body relies on the AVC-init single-shot
    // UpdateTransform plus the parent-chain transform inheritance.
    let model_root_local_y = model_root_translation.map(|t| t[1]).unwrap_or(0.0);

    // Store runtime IDs (body_pipeline_id stored after pipeline creation below).
    if let Some(c) = world.get_component_by_id_as_mut::<AvatarControlComponent>(id) {
        c.displaced_head = Some(head_bone_id);
        c.splice_camera_bone = camera_bone_id;
        c.humanoid_map_gltf = Some(humanoid_map.owning_gltf);
        c.humanoid_map_generation = humanoid_map.generation;
        c.model_root_id = Some(model_root_id);
        c.model_root_local_y = model_root_local_y;
        c.neck_bone_id = neck_bone_id;
        c.neck_rest_translation = neck_rest_t;
        if let Some(((binding, _, _), _)) = left {
            c.left_hand_bone_id = Some(binding.hand);
        }
        if let Some(((binding, _, _), _)) = right {
            c.right_hand_bone_id = Some(binding.hand);
        }
        if let Some(((_, raw_driver, hand_driver), _)) = left {
            c.left_hand_raw_target_id = Some(raw_driver);
            c.left_hand_visual_target_id = Some(hand_driver);
        }
        if let Some(((_, raw_driver, hand_driver), _)) = right {
            c.right_hand_raw_target_id = Some(raw_driver);
            c.right_hand_visual_target_id = Some(hand_driver);
        }
        c.left_hand_aim_correction = left.and_then(|(_, correction)| correction);
        c.right_hand_aim_correction = right.and_then(|(_, correction)| correction);
    }
    // Avatar-finger lasers need the final quaternion-corrected hand targets
    // cached above. Controller registration happens earlier, so retry their
    // runtime mount now that AVC and the imported skeleton are both ready.
    for controller in [left, right]
        .into_iter()
        .flatten()
        .map(|((binding, _, _), _)| binding.controller)
    {
        crate::engine::ecs::system::pointer_system::ensure_xr_hand_laser(
            world,
            retargeting,
            controller,
            emit,
        );
    }

    // -----------------------------------------------------------------------
    // Body pipeline: created as a child of AVC; model_root re-parented under it.
    //
    // Topology:
    //   AVC
    //     └── body_pipeline  (TransformForkTRSComponent)
    //           TransformMapRotationComponent
    //             QuatYawFollowComponent { threshold, rate, initial_yaw, forward_plus_z }
    //           model_root  ← re-parented here
    // -----------------------------------------------------------------------
    if !skip_body_pipeline {
        let body_pipeline_id = world.add_component(TransformForkTRSComponent::new());
        let body_pipeline_serialize_id = world.add_component(SerializeComponent::off());
        let map_rot_id = world.add_component(TransformMapRotationComponent::new());
        let yaw_follow_id = world.add_component(
            QuatYawFollowComponent::new(body_yaw_threshold, body_yaw_rate)
                .with_initial_yaw(resolved_initial_body_yaw)
                .with_forward_plus_z_if(resolved_body_forward_plus_z),
        );

        let _ = world.set_parent(body_pipeline_serialize_id, Some(body_pipeline_id));
        let _ = world.set_parent(map_rot_id, Some(body_pipeline_id));
        let _ = world.set_parent(yaw_follow_id, Some(map_rot_id));

        if let Some(c) = world.get_component_by_id_as_mut::<AvatarControlComponent>(id) {
            c.body_pipeline_id = Some(body_pipeline_id);
        }

        emit_attach(emit, id, body_pipeline_id);
        emit_attach(emit, body_pipeline_id, model_root_id);
    }

    // Head IK target offset: default to authored eye offset (CXR wrapper), with
    // optional Y override for neck-height fine tuning.
    let mut ik_eye_offset_head_local = eye_offset_head_local;
    if let Some(y) = head_ik_eye_height {
        ik_eye_offset_head_local[1] = y;
    }
    let neg_eye = [
        -ik_eye_offset_head_local[0],
        -ik_eye_offset_head_local[1],
        -ik_eye_offset_head_local[2],
    ];
    // Full desired head-pivot offset in driven_t local space.
    let head_target_offset = quat_rotate_vec3(quat_rotation_y(head_ik_offset_yaw), neg_eye);

    // Dedicated fixed visible-head mount under driven_t.
    let head_target_id = world.add_component(
        TransformComponent::new()
            .with_position(
                head_target_offset[0],
                head_target_offset[1],
                head_target_offset[2],
            )
            .with_rotation_quat(quat_rotation_y(head_ik_offset_yaw)),
    );
    let _ = world.set_parent(head_target_id, Some(driven_t_id));
    if let Some(c) = world.get_component_by_id_as_mut::<AvatarControlComponent>(id) {
        c.head_mount = Some(head_target_id);
    }

    emit_attach(emit, driven_t_id, head_target_id);
    emit_attach(emit, head_target_id, head_bone_id);

    // Zero head_bone's local translation — the driven head mount owns its offset.
    // Preserve the authored head rest rotation/scale so the visible
    // head mesh and camera anchor share the same convention across desktop and XR.
    // Emitted *after* the reparent attach so the UpdateTransform lands on
    // head_bone in its new parent without fighting the attach intent's matrix recompute.
    emit.push_intent_now(
        head_bone_id,
        IntentValue::UpdateTransform {
            component_id: head_bone_id,
            translation: [0.0, 0.0, 0.0],
            rotation_quat_xyzw: head_rest_rot,
            scale: head_rest_s,
        },
    );

    // -----------------------------------------------------------------------
    // Arm IK (TwoBoneIK) with explicit joint IDs.
    //
    // For each side: resolve all three arm bones (upper + lower + hand) and
    // hand them to the solver via `IKSolver::TwoBoneIK { root_joint_id,
    // mid_joint_id, .. }` + `IKChainComponent::end_effector_id`. The solver
    // does no topology discovery, so sibling cloth / collider / helper bones
    // under the arm joints (e.g. bisket's `J_Sec_L_TopsUpperArm_*` and
    // `J_Bip_L_UpperArm_collider_*`) are irrelevant.
    //
    // Bone IDs and the tracked target come from the already validated
    // `ArmIkBinding`; there is no name lookup, topology inference, or synthetic
    // origin target in this construction path.
    // -----------------------------------------------------------------------
    for (hand_opt, pole_dir, side_label) in [
        (left, left_arm_pole_direction, "left"),
        (right, right_arm_pole_direction, "right"),
    ] {
        let Some(((binding, raw_driver, hand_driver), _)) = hand_opt else {
            continue;
        };
        let upper_arm = binding.upper_arm;
        let lower_arm = binding.lower_arm;
        let hand_bone = binding.hand;

        let bone_name =
            |id: ComponentId| -> String { world.component_name(id).unwrap_or("?").to_string() };
        let upper_name_s = bone_name(upper_arm);
        let lower_name_s = bone_name(lower_arm);
        let hand_name_s = bone_name(hand_bone);
        println!(
            "[AVC] {} arm IK: root={} (id={:?}), mid={} (id={:?}), hand={} (id={:?}), target=(id={:?})",
            side_label,
            upper_name_s,
            upper_arm,
            lower_name_s,
            lower_arm,
            hand_name_s,
            hand_bone,
            hand_driver,
        );
        let looks_suspicious = |n: &str| {
            n.contains("Twist")
                || n.contains("Roll")
                || n.contains("Helper")
                || n.contains("_collider")
                || n.contains("J_Sec_")
        };
        if looks_suspicious(&upper_name_s) || looks_suspicious(&lower_name_s) {
            println!(
                "[AVC] WARNING: {} arm IK resolved to a helper/cloth/collider bone — \
                override the {}_upper_arm and {}_lower_arm slots in HumanoidBoneMap.",
                side_label, side_label, side_label
            );
        }

        let mut chain = IKChainComponent::new(
            IKSolver::TwoBoneIK {
                root_joint_id: upper_arm,
                mid_joint_id: lower_arm,
                pole_direction: pole_dir,
                copy_end_rotation: true,
            },
            hand_driver,
            hand_bone,
        );
        chain.xr_pose_driver = find_xr_pose_driver(world, hand_driver);
        let chain_id = world.add_component(chain);
        let chain_serialize_id = world.add_component(SerializeComponent::off());
        let _ = world.set_parent(chain_serialize_id, Some(chain_id));
        let _ = raw_driver;
        // Parent under AVC for cleanup; the solver itself ignores the chain's parent.
        emit_attach(emit, id, chain_id);
    }

    // -----------------------------------------------------------------------
    // Camera re-parenting: move discovered Camera3D/CameraXR children of AVC
    // under the camera bone so they inherit its world transform each tick.
    // -----------------------------------------------------------------------
    if let Some(cam_bone_id) = camera_bone_id {
        for &(cam, _eye_offset, is_desktop_camera_path) in &camera_children {
            if is_desktop_camera_path {
                if let Some(tc) = world.get_component_by_id_as_mut::<TransformComponent>(cam) {
                    if tc.transform.rotation != quat_rotation_y(std::f32::consts::PI) {
                        tc.transform.rotation = quat_rotation_y(std::f32::consts::PI);
                        tc.transform.recompute_model();
                    }
                } else {
                    let desktop_camera_mount = world.add_component(
                        TransformComponent::new()
                            .with_rotation_quat(quat_rotation_y(std::f32::consts::PI)),
                    );
                    let desktop_camera_mount_serialize_id =
                        world.add_component(SerializeComponent::off());
                    let _ = world.set_parent(
                        desktop_camera_mount_serialize_id,
                        Some(desktop_camera_mount),
                    );
                    emit_attach(emit, desktop_camera_mount, cam);
                    println!(
                        "[AVC] inserted desktop camera yaw-correction mount {:?} for camera {:?}",
                        desktop_camera_mount, cam
                    );
                    emit_attach(emit, cam_bone_id, desktop_camera_mount);
                    continue;
                }
            }
            println!(
                "[AVC] re-parenting camera {:?} under camera anchor {:?}",
                cam, cam_bone_id
            );
            emit_attach(emit, cam_bone_id, cam);
        }
    } else if !camera_children.is_empty() {
        println!(
            "[AVC] WARNING: camera children found but camera_anchor not resolved — no re-parenting"
        );
    }
}

fn find_xr_pose_driver(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut current = Some(start);
    while let Some(component) = current {
        if world
            .get_component_by_id_as::<ControllerXRComponent>(component)
            .is_some()
            || world
                .get_component_by_id_as::<crate::engine::ecs::component::InputXRComponent>(
                    component,
                )
                .is_some()
        {
            return Some(component);
        }
        current = world.parent_of(component);
    }
    None
}

/// Create the optional rotation-correction child beneath an already classified hand target.
fn resolve_hand_target(
    world: &mut World,
    binding: ArmIkBinding,
    rotation_offset: Option<[f32; 4]>,
) -> (ArmIkBinding, ComponentId, ComponentId) {
    let driver = binding.raw_target;
    let hand_driver = if let Some(offset_q) = rotation_offset {
        let offset = world.add_component(TransformComponent::new().with_rotation_quat(offset_q));
        let offset_serialize_id = world.add_component(SerializeComponent::off());
        let _ = world.set_parent(offset_serialize_id, Some(offset));
        let _ = world.set_parent(offset, Some(driver));
        offset
    } else {
        driver
    };

    (binding, driver, hand_driver)
}

/// Returns `None` only while the imported skeleton or its retained basis is still in flight.
/// The inner option is absent when this target has no usable authored basis.
fn derive_hand_aim_correction(
    _world: &World,
    retargeting: &JointBasisRetargetingSystem,
    _model_root: ComponentId,
    hand_bone: Option<ComponentId>,
    side: &str,
) -> Option<Option<[f32; 4]>> {
    let Some(hand_bone) = hand_bone else {
        return Some(None);
    };
    match retargeting.status_for(hand_bone) {
        Some(RetargetBasisStatus::Ready) => {
            // HumanoidBoneMap will provide this same target hand ID through semantic slots.
            return retargeting
                .basis_for(hand_bone)
                .map(|basis| Some(mat_to_quat(basis.target_rest_to_canonical)));
        }
        Some(RetargetBasisStatus::Invalid(error)) => {
            eprintln!("[AVC][hand-basis] {side} retained definition is invalid: {error}");
            return Some(None);
        }
        Some(RetargetBasisStatus::ConflictingDefinition { .. }) => {
            eprintln!("[AVC][hand-basis] {side} has conflicting retained definitions");
            return Some(None);
        }
        Some(RetargetBasisStatus::WaitingForGltf) => return None,
        None => {}
    }
    Some(None)
}

fn map_target(report: &HumanoidBoneMapReport, slot: HumanoidSlot) -> Option<ComponentId> {
    report.target(slot).map(|target| target.component)
}

fn ensure_map_hand_basis(
    world: &World,
    retargeting: &mut JointBasisRetargetingSystem,
    report: &HumanoidBoneMapReport,
    left: bool,
) {
    let (hand, middle_start, middle_end, little, index) = if left {
        (
            HumanoidSlot::LeftHand,
            HumanoidSlot::LeftMiddleProximal,
            HumanoidSlot::LeftMiddleDistal,
            HumanoidSlot::LeftLittleProximal,
            HumanoidSlot::LeftIndexProximal,
        )
    } else {
        (
            HumanoidSlot::RightHand,
            HumanoidSlot::RightMiddleProximal,
            HumanoidSlot::RightMiddleDistal,
            HumanoidSlot::RightLittleProximal,
            HumanoidSlot::RightIndexProximal,
        )
    };
    let Some(target) = map_target(report, hand) else {
        return;
    };
    // An authored definition is the expert override and is never replaced.
    if retargeting.status_for(target).is_some() {
        return;
    }
    let Some(forward_start) = map_target(report, middle_start) else {
        return;
    };
    let Some(forward_end) = map_target(report, middle_end) else {
        return;
    };
    let Some(up_start) = map_target(report, little) else {
        return;
    };
    let Some(up_end) = map_target(report, index) else {
        return;
    };
    retargeting.replace_definition(
        world,
        target,
        RetargetBasisDefinition {
            target,
            forward: LandmarkDirection {
                start: forward_start,
                end: forward_end,
            },
            up: LandmarkDirection {
                start: up_start,
                end: up_end,
            },
        },
    );
}

fn emit_attach(emit: &mut dyn SignalEmitter, parent: ComponentId, child: ComponentId) {
    emit.push_intent_now(
        parent,
        IntentValue::Attach {
            parent: parent,
            child,
        },
    );
}

fn update_hand_pose_corrections(
    avc_id: ComponentId,
    world: &World,
    emit: &mut dyn SignalEmitter,
    log_alignment: bool,
) {
    let (left_raw, right_raw, left_visual, right_visual, left_correction, right_correction) = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(avc_id) else {
            return;
        };
        (
            c.left_hand_raw_target_id,
            c.right_hand_raw_target_id,
            c.left_hand_visual_target_id,
            c.right_hand_visual_target_id,
            c.left_hand_aim_correction,
            c.right_hand_aim_correction,
        )
    };
    for (side, raw, visual, correction) in [
        (ControllerHand::Left, left_raw, left_visual, left_correction),
        (
            ControllerHand::Right,
            right_raw,
            right_visual,
            right_correction,
        ),
    ] {
        let controller = world.children_of(avc_id).iter().find_map(|id| {
            world
                .get_component_by_id_as::<ControllerXRComponent>(*id)
                .filter(|controller| controller.hand == side)
        });
        let source = controller
            .map(|controller| controller.active_pose_source)
            .unwrap_or(ControllerPoseSource::None);
        let uses_canonical_hand_basis = matches!(
            source,
            ControllerPoseSource::ControllerAim
                | ControllerPoseSource::ControllerGripAim
                | ControllerPoseSource::WristPalm
        );
        let applied = if uses_canonical_hand_basis {
            correction.unwrap_or([0.0, 0.0, 0.0, 1.0])
        } else {
            [0.0, 0.0, 0.0, 1.0]
        };
        if let (Some(raw), Some(visual)) = (raw, visual) {
            if raw != visual {
                update_local_rotation(world, emit, visual, applied);
            }
        }
        if log_alignment && source != ControllerPoseSource::None {
            log_hand_alignment(avc_id, world, side, controller, source, correction, applied);
        }
    }
}

fn log_hand_alignment(
    avc_id: ComponentId,
    _world: &World,
    hand: ControllerHand,
    controller: Option<&ControllerXRComponent>,
    source: ControllerPoseSource,
    correction: Option<[f32; 4]>,
    applied: [f32; 4],
) {
    let aim = controller.and_then(|controller| controller.raw_aim_rotation);
    let grip = controller.and_then(|controller| controller.raw_grip_rotation);
    let basis_mode = if correction.is_some() {
        "retained"
    } else {
        "none"
    };
    let grip_to_aim = match (grip, aim) {
        (Some(grip), Some(aim)) => Some(quat_to_axis_angle(quat_mul(quat_conjugate(grip), aim))),
        _ => None,
    };
    let mount = correction.map(quat_conjugate);
    let predicted = if matches!(
        source,
        ControllerPoseSource::ControllerAim
            | ControllerPoseSource::ControllerGripAim
            | ControllerPoseSource::WristPalm
    ) {
        mount.map(|mount| quat_to_axis_angle(quat_mul(applied, mount)))
    } else {
        None
    };
    let calibration = match source {
        ControllerPoseSource::WristPalm => "synthesized-hand-calibrated",
        ControllerPoseSource::ControllerAim | ControllerPoseSource::ControllerGripAim => {
            "aim-calibrated"
        }
        _ => "identity",
    };
    eprintln!(
        "[AVC][hand-alignment] avc={avc_id:?} hand={hand:?} aim_valid={} grip_valid={} aim_q={aim:?} grip_q={grip:?} source={source:?} mode={calibration} avatar_basis={basis_mode} grip_to_aim_axis_angle={grip_to_aim:?} canonical_to_hand_q={mount:?} applied_correction_q={applied:?} final_basis_to_aim_axis_angle={predicted:?}",
        aim.is_some(),
        grip.is_some(),
    );
}

fn hand_alignment_debug_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("CAT_DEBUG_XR_HAND_ALIGNMENT")
            .ok()
            .is_some_and(|value| value == "1")
    })
}

fn update_local_rotation(
    world: &World,
    emit: &mut dyn SignalEmitter,
    component_id: ComponentId,
    rotation: [f32; 4],
) {
    let Some(transform) = world.get_component_by_id_as::<TransformComponent>(component_id) else {
        return;
    };
    if transform.transform.rotation == rotation {
        return;
    }
    let translation = transform.transform.translation;
    let scale = transform.transform.scale;
    emit.push_intent_now(
        component_id,
        IntentValue::UpdateTransform {
            component_id: component_id,
            translation,
            rotation_quat_xyzw: rotation,
            scale,
        },
    );
}

/// Read a bone's authored bind-pose local TRS via the `BoneRestPoseComponent`
/// sidecar that `GLTFSystem` stamps at node-spawn time.  Falls back to the
/// live `TransformComponent` (then to identity) for non-GLTF skeletons that
/// never had a rest-pose snapshot attached.
fn read_bone_rest_pose(world: &World, bone_id: ComponentId) -> ([f32; 3], [f32; 4], [f32; 3]) {
    if let Some(rest) = world
        .children_of(bone_id)
        .iter()
        .find_map(|&c| world.get_component_by_id_as::<BoneRestPoseComponent>(c))
    {
        return (rest.translation, rest.rotation, rest.scale);
    }
    world
        .get_component_by_id_as::<TransformComponent>(bone_id)
        .map(|t| {
            (
                t.transform.translation,
                t.transform.rotation,
                t.transform.scale,
            )
        })
        .unwrap_or(([0.0; 3], [0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]))
}

/// Compose `descendant`'s authored/rest transform relative to `ancestor`.
///
/// GLTF transforms use their immutable `BoneRestPoseComponent` snapshot.
/// Non-GLTF transforms fall back to their local model matrix. World matrices
/// are deliberately ignored because XR may update an ancestor before the
/// imported subtree has been propagated for the same frame.
fn rest_model_relative_to(
    world: &World,
    ancestor: ComponentId,
    descendant: ComponentId,
) -> Option<[[f32; 4]; 4]> {
    let mut transforms = Vec::new();
    let mut current = Some(descendant);
    let mut found_ancestor = false;

    while let Some(id) = current {
        if id == ancestor {
            found_ancestor = true;
            break;
        }
        if world
            .get_component_by_id_as::<TransformComponent>(id)
            .is_some()
        {
            transforms.push(id);
        }
        current = world.parent_of(id);
    }
    if !found_ancestor {
        return None;
    }

    transforms.reverse();
    Some(transforms.into_iter().fold(mat4_identity(), |model, id| {
        let local = if let Some(rest) = world
            .children_of(id)
            .iter()
            .find_map(|&child| world.get_component_by_id_as::<BoneRestPoseComponent>(child))
        {
            let mut transform = Transform::default();
            transform.translation = rest.translation;
            transform.rotation = rest.rotation;
            transform.scale = rest.scale;
            transform.recompute_model();
            transform.model
        } else {
            world
                .get_component_by_id_as::<TransformComponent>(id)
                .map(|transform| transform.transform.model)
                .unwrap_or_else(mat4_identity)
        };
        mat4_mul(model, local)
    }))
}

#[cfg(test)]
mod hand_pose_correction_tests {
    use super::*;
    use crate::engine::ecs::component::xr_eye_tracking::{EyeClosureSample, EyeGazeSample};
    use crate::engine::ecs::component::{
        AmplitudeComponent, AmplitudeSample, AmplitudeStatus, ComponentRef, EyeRotationLimits,
        MorphFactorState, MorphTargetInfo, MorphTargetKey, MorphTargetMapComponent,
        RestAttachmentComponent, XREyeTrackingComponent, XREyeTrackingHtcComponent,
    };
    use crate::engine::ecs::system::{
        HumanoidSlotProvenance, HumanoidSlotReport, HumanoidSlotStatus, ResolvedHumanoidTarget,
        ResolvedHumanoidTargetKind,
    };
    use crate::engine::ecs::{EventSignal, IntentSignal};
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct RecordingEmitter(Vec<[f32; 4]>);

    impl SignalEmitter for RecordingEmitter {
        fn push_event(&mut self, _scope: ComponentId, _event: EventSignal) {}

        fn push_intent(&mut self, _scope: ComponentId, intent: IntentSignal) {
            if let IntentValue::UpdateTransform {
                rotation_quat_xyzw, ..
            } = intent.value
            {
                self.0.push(rotation_quat_xyzw);
            }
        }
    }

    #[test]
    fn amplitude_binding_maps_current_rms_and_primary_driver_wins() {
        let mut world = World::default();
        let amplitude_id = world.add_component(AmplitudeComponent::rolling_window(0.25).unwrap());
        let amplitude_guid = world.get_component_record(amplitude_id).unwrap().guid;
        {
            let amplitude = world
                .get_component_by_id_as_mut::<AmplitudeComponent>(amplitude_id)
                .unwrap();
            amplitude.retained = AmplitudeSample {
                generation: amplitude.generation,
                sequence: 1,
                timestamp_sec: 1.0,
                valid_frames: 64,
                rms: 0.06,
                peak: 0.1,
                status: AmplitudeStatus::Live,
            };
        }
        let avc_id = world.add_component(
            AvatarControlComponent::new()
                .with_mouth_open_from_amplitude(ComponentRef::Guid(amplitude_guid))
                .with_mouth_open_rms_floor(0.02)
                .unwrap()
                .with_mouth_open_rms_ceiling(0.10)
                .unwrap()
                .with_mouth_open_smoothing(0.0)
                .unwrap(),
        );
        let joint = world.add_component(TransformComponent::new());
        let key = MorphTargetKey {
            node_index: 0,
            primitive_index: 0,
            target_index: 0,
        };
        let mut gltf = GLTFComponent::new("synthetic.glb");
        gltf.armature_joint_transforms.push(joint);
        gltf.morph_targets.push(MorphTargetInfo {
            key,
            label: Some("MouthA".into()),
            base_factor: 0.1,
        });
        gltf.morph_factors.insert(
            key,
            MorphFactorState {
                base: 0.1,
                driver: None,
                amplitude_mouth_open: None,
            },
        );
        let gltf_id = world.add_component(gltf);
        let map_id =
            world.add_component(MorphTargetMapComponent::new().with_slot("viseme_aa", "MouthA"));
        world.add_child(avc_id, gltf_id).unwrap();
        world.add_child(gltf_id, joint).unwrap();
        world.add_child(gltf_id, map_id).unwrap();
        let mut maps = HumanoidBoneMapSystem::default();

        update_amplitude_mouth_open(avc_id, &mut world, &mut maps, 1.0 / 60.0);
        let state = world
            .get_component_by_id_as::<GLTFComponent>(gltf_id)
            .unwrap()
            .morph_factors[&key];
        assert!((state.amplitude_mouth_open.unwrap() - 0.5).abs() < 1e-6);
        assert!((state.effective() - 0.5).abs() < 1e-6);

        world
            .get_component_by_id_as_mut::<GLTFComponent>(gltf_id)
            .unwrap()
            .morph_factors
            .get_mut(&key)
            .unwrap()
            .driver = Some(0.8);
        let state = world
            .get_component_by_id_as::<GLTFComponent>(gltf_id)
            .unwrap()
            .morph_factors[&key];
        assert_eq!(
            state.effective(),
            0.8,
            "primary viseme/animation driver must win"
        );

        world
            .get_component_by_id_as_mut::<AmplitudeComponent>(amplitude_id)
            .unwrap()
            .bump_generation(AmplitudeStatus::Invalid);
        update_amplitude_mouth_open(avc_id, &mut world, &mut maps, 1.0 / 60.0);
        assert_eq!(
            world
                .get_component_by_id_as::<GLTFComponent>(gltf_id)
                .unwrap()
                .morph_factors[&key]
                .amplitude_mouth_open,
            Some(0.0),
        );
    }

    #[test]
    fn amplitude_binding_missing_slot_is_harmless_and_diagnosed_once() {
        let mut world = World::default();
        let amplitude_id = world.add_component(AmplitudeComponent::default());
        let guid = world.get_component_record(amplitude_id).unwrap().guid;
        let avc_id = world.add_component(
            AvatarControlComponent::new()
                .with_mouth_open_from_amplitude(ComponentRef::Guid(guid))
                .with_mouth_open_smoothing(0.0)
                .unwrap(),
        );
        let joint = world.add_component(TransformComponent::new());
        let mut gltf = GLTFComponent::new("synthetic.glb");
        gltf.armature_joint_transforms.push(joint);
        let gltf_id = world.add_component(gltf);
        world.add_child(avc_id, gltf_id).unwrap();
        world.add_child(gltf_id, joint).unwrap();
        let mut maps = HumanoidBoneMapSystem::default();
        update_amplitude_mouth_open(avc_id, &mut world, &mut maps, 1.0 / 60.0);
        update_amplitude_mouth_open(avc_id, &mut world, &mut maps, 1.0 / 60.0);
        assert!(
            world
                .get_component_by_id_as::<AvatarControlComponent>(avc_id)
                .unwrap()
                .mouth_open_missing_slot_diagnosed
        );
    }

    #[test]
    fn amplitude_binding_re_resolves_selector_after_source_replacement() {
        let mut world = World::default();
        let first =
            world.add_component_boxed_named("voice_level", Box::new(AmplitudeComponent::default()));
        let avc_id = world.add_component(
            AvatarControlComponent::new()
                .with_mouth_open_from_amplitude(ComponentRef::Query("#voice_level".into()))
                .with_mouth_open_smoothing(0.0)
                .unwrap(),
        );
        // No rig is required to verify the durable reference cache lifecycle.
        let mut maps = HumanoidBoneMapSystem::default();
        update_amplitude_mouth_open(avc_id, &mut world, &mut maps, 1.0 / 60.0);
        assert_eq!(
            world
                .get_component_by_id_as::<AvatarControlComponent>(avc_id)
                .unwrap()
                .resolved_mouth_open_amplitude,
            Some(first)
        );

        world.remove_component_leaf(first).unwrap();
        update_amplitude_mouth_open(avc_id, &mut world, &mut maps, 1.0 / 60.0);
        assert_eq!(
            world
                .get_component_by_id_as::<AvatarControlComponent>(avc_id)
                .unwrap()
                .resolved_mouth_open_amplitude,
            None
        );

        let replacement =
            world.add_component_boxed_named("voice_level", Box::new(AmplitudeComponent::default()));
        update_amplitude_mouth_open(avc_id, &mut world, &mut maps, 1.0 / 60.0);
        assert_eq!(
            world
                .get_component_by_id_as::<AvatarControlComponent>(avc_id)
                .unwrap()
                .resolved_mouth_open_amplitude,
            Some(replacement)
        );
    }

    #[test]
    fn direct_child_eye_trackers_choose_newest_source_per_eye() {
        let mut world = World::default();
        let avc = world.add_component(AvatarControlComponent::new());
        let standard = world.add_component(XREyeTrackingComponent::on());
        let htc = world.add_component(XREyeTrackingHtcComponent::on());
        let descendant = world.add_component(XREyeTrackingComponent::on());
        let _ = world.add_child(avc, standard);
        let _ = world.add_child(avc, htc);
        let _ = world.add_child(standard, descendant);
        world
            .get_component_by_id_as_mut::<XREyeTrackingComponent>(standard)
            .unwrap()
            .gaze_sample = EyeGazeSample {
            left: Some([1.0, 0.0, 0.0]),
            right: Some([0.0, 1.0, 0.0]),
            sequence: 2,
        };
        world
            .get_component_by_id_as_mut::<XREyeTrackingHtcComponent>(htc)
            .unwrap()
            .gaze_sample = EyeGazeSample {
            left: Some([0.0, 0.0, -1.0]),
            right: None,
            sequence: 3,
        };
        world
            .get_component_by_id_as_mut::<XREyeTrackingComponent>(descendant)
            .unwrap()
            .gaze_sample = EyeGazeSample {
            left: Some([0.0, -1.0, 0.0]),
            right: Some([0.0, -1.0, 0.0]),
            sequence: 99,
        };
        assert_eq!(
            newest_direct_eye_gaze(&mut world, avc),
            (
                Some(ResolvedEyeGaze {
                    direction: [0.0, 0.0, -1.0],
                    compensation: HeadRotationCompensation::Off,
                    rotation_limits: None,
                    sequence: 3,
                }),
                Some(ResolvedEyeGaze {
                    direction: [0.0, 1.0, 0.0],
                    compensation: HeadRotationCompensation::Off,
                    rotation_limits: None,
                    sequence: 2,
                }),
            )
        );
    }

    #[test]
    fn disabled_newer_gaze_does_not_suppress_an_enabled_direct_tracker() {
        let mut world = World::default();
        let avc = world.add_component(AvatarControlComponent::new());
        let enabled = world
            .add_component(XREyeTrackingComponent::on().with_enable_pupil_direction_tracking(true));
        let disabled = world.add_component(
            XREyeTrackingHtcComponent::on().with_enable_pupil_direction_tracking(false),
        );
        world.add_child(avc, enabled).unwrap();
        world.add_child(avc, disabled).unwrap();
        world
            .get_component_by_id_as_mut::<XREyeTrackingComponent>(enabled)
            .unwrap()
            .gaze_sample = EyeGazeSample {
            left: Some([0.0, 0.0, -1.0]),
            right: Some([0.0, 0.0, -1.0]),
            sequence: 1,
        };
        world
            .get_component_by_id_as_mut::<XREyeTrackingHtcComponent>(disabled)
            .unwrap()
            .gaze_sample = EyeGazeSample {
            left: Some([1.0, 0.0, 0.0]),
            right: Some([1.0, 0.0, 0.0]),
            sequence: 2,
        };

        let (left, right) = newest_direct_eye_gaze(&mut world, avc);
        assert_eq!(left.unwrap().sequence, 1);
        assert_eq!(right.unwrap().sequence, 1);
    }

    #[test]
    fn blink_routing_chooses_each_eye_and_drives_its_morph_independently() {
        let mut world = World::default();
        let avc = world.add_component(AvatarControlComponent::new());
        let standard = world.add_component(
            XREyeTrackingComponent::on().with_enable_pupil_direction_tracking(false),
        );
        let htc = world.add_component(
            XREyeTrackingHtcComponent::on().with_enable_pupil_direction_tracking(false),
        );
        let descendant = world.add_component(XREyeTrackingComponent::on());
        world.add_child(avc, standard).unwrap();
        world.add_child(avc, htc).unwrap();
        world.add_child(standard, descendant).unwrap();

        world
            .get_component_by_id_as_mut::<XREyeTrackingComponent>(standard)
            .unwrap()
            .closure_sample = EyeClosureSample {
            left: Some(0.2),
            right: Some(0.3),
            sequence: 2,
        };
        world
            .get_component_by_id_as_mut::<XREyeTrackingHtcComponent>(htc)
            .unwrap()
            .closure_sample = EyeClosureSample {
            left: Some(0.8),
            right: None,
            sequence: 3,
        };
        world
            .get_component_by_id_as_mut::<XREyeTrackingComponent>(descendant)
            .unwrap()
            .closure_sample = EyeClosureSample {
            left: Some(1.0),
            right: Some(1.0),
            sequence: 99,
        };

        let closure = newest_direct_eye_closure(&world, avc);
        assert_eq!(closure, (Some(0.8), Some(0.3)));

        let left_key = MorphTargetKey {
            node_index: 0,
            primitive_index: 0,
            target_index: 0,
        };
        let right_key = MorphTargetKey {
            target_index: 1,
            ..left_key
        };
        let mut gltf = GLTFComponent::new("test.glb");
        gltf.morph_targets = vec![
            MorphTargetInfo {
                key: left_key,
                label: Some("BlinkLeft".into()),
                base_factor: 0.1,
            },
            MorphTargetInfo {
                key: right_key,
                label: Some("BlinkRight".into()),
                base_factor: 0.15,
            },
        ];
        gltf.morph_factors.insert(
            left_key,
            MorphFactorState {
                base: 0.1,
                driver: None,
                amplitude_mouth_open: None,
            },
        );
        gltf.morph_factors.insert(
            right_key,
            MorphFactorState {
                base: 0.15,
                driver: None,
                amplitude_mouth_open: None,
            },
        );
        let gltf_id = world.add_component(gltf);
        let map = world.add_component(
            MorphTargetMapComponent::new()
                .with_slot("left_eye_blink", "BlinkLeft")
                .with_slot("right_eye_blink", "BlinkRight"),
        );
        world.add_child(gltf_id, map).unwrap();

        apply_eye_blink_drivers(&mut world, gltf_id, closure);
        let gltf = world
            .get_component_by_id_as::<GLTFComponent>(gltf_id)
            .unwrap();
        assert_eq!(gltf.morph_factors[&left_key].driver, Some(0.8));
        assert_eq!(gltf.morph_factors[&right_key].driver, Some(0.3));
        assert_eq!(gltf.morph_factors[&left_key].base, 0.1);
        assert_eq!(gltf.morph_factors[&right_key].base, 0.15);
        apply_eye_blink_drivers(&mut world, gltf_id, (None, None));
        let gltf = world
            .get_component_by_id_as::<GLTFComponent>(gltf_id)
            .unwrap();
        assert_eq!(gltf.morph_factors[&left_key].driver, None);
        assert_eq!(gltf.morph_factors[&right_key].driver, None);
        assert_eq!(gltf.morph_factors[&left_key].effective(), 0.1);
        assert_eq!(gltf.morph_factors[&right_key].effective(), 0.15);
    }

    #[test]
    fn shared_rotation_limits_clamp_yaw_and_pitch() {
        let limits = EyeRotationLimits::from_array([0.2, 0.3, 0.4, 0.5]);
        let gaze = clamp_eye_gaze_rotation(ResolvedEyeGaze {
            direction: [0.8, 0.8, -1.0],
            compensation: HeadRotationCompensation::Off,
            rotation_limits: Some(limits),
            sequence: 1,
        });
        assert!((gaze.direction[0].atan2(-gaze.direction[2]) - 0.3).abs() < 1e-5);
        assert!(
            (gaze.direction[1]
                .atan2((gaze.direction[0].powi(2) + gaze.direction[2].powi(2)).sqrt())
                - 0.4)
                .abs()
                < 1e-5
        );
    }

    #[test]
    fn per_eye_rotation_limits_are_independent() {
        let mut world = World::default();
        let avc = world.add_component(AvatarControlComponent::new());
        let tracker = world.add_component(
            XREyeTrackingComponent::on()
                .with_rotation_limits_per_eye([0.1, 0.2, 0.3, 0.4], [0.5, 0.6, 0.7, 0.8]),
        );
        world.add_child(avc, tracker).unwrap();
        world
            .get_component_by_id_as_mut::<XREyeTrackingComponent>(tracker)
            .unwrap()
            .gaze_sample = EyeGazeSample {
            left: Some([1.0, 0.0, -1.0]),
            right: Some([1.0, 0.0, -1.0]),
            sequence: 1,
        };
        let (left, right) = newest_direct_eye_gaze(&mut world, avc);
        let left = clamp_eye_gaze_rotation(left.unwrap());
        let right = clamp_eye_gaze_rotation(right.unwrap());
        assert!((left.direction[0].atan2(-left.direction[2]) - 0.2).abs() < 1e-5);
        assert!((right.direction[0].atan2(-right.direction[2]) - 0.6).abs() < 1e-5);
    }

    #[test]
    fn shared_and_per_eye_rotation_limits_use_tighter_cap() {
        let gaze = clamp_eye_gaze_rotation(ResolvedEyeGaze {
            direction: [1.0, 0.0, -1.0],
            compensation: HeadRotationCompensation::Off,
            rotation_limits: crate::engine::ecs::component::combined_eye_rotation_limits(
                Some(EyeRotationLimits::from_array([0.5, 0.5, 0.5, 0.5])),
                Some(EyeRotationLimits::from_array([0.3, 0.2, 0.3, 0.3])),
            ),
            sequence: 1,
        });
        assert!((gaze.direction[0].atan2(-gaze.direction[2]) - 0.2).abs() < 1e-5);
    }

    #[test]
    fn eye_tracking_is_rest_relative_and_restores_when_inactive() {
        let mut world = World::default();
        let avc = world.add_component(AvatarControlComponent::new());
        let eye = world.add_component(TransformComponent::new());
        let rest = [0.0, 0.5, 0.0, 0.866_025_4];
        let rest_pose = world.add_component(BoneRestPoseComponent::new([0.0; 3], rest, [1.0; 3]));
        let _ = world.add_child(eye, rest_pose);
        let mut emitted = RecordingEmitter::default();
        update_one_eye_tracking(
            avc,
            &mut world,
            &mut emitted,
            true,
            Some(eye),
            Some(ResolvedEyeGaze {
                direction: [1.0, 0.0, 0.0],
                compensation: HeadRotationCompensation::Off,
                rotation_limits: None,
                sequence: 1,
            }),
        );
        let expected = quat_mul(shortest_arc_quat([0.0, 0.0, -1.0], [1.0, 0.0, 0.0]), rest);
        assert_eq!(emitted.0, vec![expected]);
        update_one_eye_tracking(avc, &mut world, &mut emitted, true, Some(eye), None);
        assert_eq!(emitted.0.last(), Some(&rest));
    }

    #[test]
    fn cancel_head_rotation_converts_world_gaze_to_eye_parent_basis() {
        let mut world = World::default();
        let parent = world.add_component(TransformComponent::new().with_rotation_quat([
            0.0,
            0.707_106_77,
            0.0,
            0.707_106_77,
        ]));
        let parent_transform = world
            .get_component_by_id_as_mut::<TransformComponent>(parent)
            .unwrap();
        parent_transform.transform.matrix_world = parent_transform.transform.model;
        let eye = world.add_component(TransformComponent::new());
        world.add_child(parent, eye).unwrap();

        let local = gaze_in_eye_parent_basis(
            &world,
            eye,
            ResolvedEyeGaze {
                direction: [1.0, 0.0, 0.0],
                compensation: HeadRotationCompensation::CancelHeadRotation,
                rotation_limits: None,
                sequence: 1,
            },
        );
        assert!((local[0]).abs() < 1e-5);
        assert!((local[1]).abs() < 1e-5);
        assert!((local[2] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn freeze_head_motion_policy_holds_last_gaze_during_a_rapid_turn() {
        let mut world = World::default();
        let avc = world.add_component(
            AvatarControlComponent::new()
                .with_head_motion_gaze_policy(HeadMotionGazePolicy::Freeze),
        );
        let parent = world.add_component(TransformComponent::new());
        let eye = world.add_component(TransformComponent::new());
        world.add_child(parent, eye).unwrap();
        let old = ResolvedEyeGaze {
            direction: [0.25, 0.0, -1.0],
            compensation: HeadRotationCompensation::Off,
            rotation_limits: None,
            sequence: 1,
        };
        assert_eq!(
            apply_head_motion_gaze_policy(
                avc,
                &mut world,
                Some(eye),
                None,
                Some(old),
                None,
                1.0 / 60.0,
            )
            .0,
            Some(old),
        );

        let rotated = TransformComponent::new()
            .with_rotation_quat([0.0, 0.707_106_77, 0.0, 0.707_106_77])
            .transform
            .model;
        world
            .get_component_by_id_as_mut::<TransformComponent>(parent)
            .unwrap()
            .transform
            .matrix_world = rotated;
        let incoming = ResolvedEyeGaze {
            direction: [-0.5, 0.0, -1.0],
            compensation: HeadRotationCompensation::Off,
            rotation_limits: None,
            sequence: 2,
        };
        let frozen = apply_head_motion_gaze_policy(
            avc,
            &mut world,
            Some(eye),
            None,
            Some(incoming),
            None,
            1.0 / 60.0,
        )
        .0
        .unwrap();
        assert_eq!(frozen.direction, old.direction);
        assert_eq!(frozen.compensation, HeadRotationCompensation::Off);
        assert_eq!(frozen.sequence, incoming.sequence);
    }

    #[test]
    fn freeze_policy_holds_the_already_limited_gaze() {
        let mut world = World::default();
        let avc = world.add_component(
            AvatarControlComponent::new()
                .with_head_motion_gaze_policy(HeadMotionGazePolicy::Freeze),
        );
        let parent = world.add_component(TransformComponent::new());
        let eye = world.add_component(TransformComponent::new());
        world.add_child(parent, eye).unwrap();
        let limits = Some(EyeRotationLimits::from_array([0.2; 4]));
        let limited = clamp_eye_gaze_rotation(ResolvedEyeGaze {
            direction: [1.0, 0.0, -1.0],
            compensation: HeadRotationCompensation::Off,
            rotation_limits: limits,
            sequence: 1,
        });
        apply_head_motion_gaze_policy(
            avc,
            &mut world,
            Some(eye),
            None,
            Some(limited),
            None,
            1.0 / 60.0,
        );
        let rotated = TransformComponent::new()
            .with_rotation_quat([0.0, 0.707_106_77, 0.0, 0.707_106_77])
            .transform
            .model;
        world
            .get_component_by_id_as_mut::<TransformComponent>(parent)
            .unwrap()
            .transform
            .matrix_world = rotated;
        let incoming = clamp_eye_gaze_rotation(ResolvedEyeGaze {
            direction: [-1.0, 0.0, -1.0],
            compensation: HeadRotationCompensation::Off,
            rotation_limits: limits,
            sequence: 2,
        });
        let frozen = apply_head_motion_gaze_policy(
            avc,
            &mut world,
            Some(eye),
            None,
            Some(incoming),
            None,
            1.0 / 60.0,
        )
        .0
        .unwrap();
        assert_eq!(frozen.direction, limited.direction);
        assert!((frozen.direction[0].atan2(-frozen.direction[2]) - 0.2).abs() < 1e-5);
    }

    fn left_arm_report(
        upper: ComponentId,
        lower: ComponentId,
        hand: ComponentId,
    ) -> HumanoidBoneMapReport {
        let mut slots = BTreeMap::new();
        for (slot, component) in [
            (HumanoidSlot::LeftUpperArm, upper),
            (HumanoidSlot::LeftLowerArm, lower),
            (HumanoidSlot::LeftHand, hand),
        ] {
            slots.insert(
                slot,
                HumanoidSlotReport {
                    slot,
                    status: HumanoidSlotStatus::Resolved(ResolvedHumanoidTarget {
                        component,
                        kind: ResolvedHumanoidTargetKind::SkinJoint,
                    }),
                    provenance: HumanoidSlotProvenance::ConventionName,
                    diagnostic: None,
                },
            );
        }
        HumanoidBoneMapReport {
            owning_gltf: ComponentId::default(),
            source: None,
            generation: 1,
            valid: true,
            diagnostics: Vec::new(),
            slots,
        }
    }

    #[test]
    fn automatic_source_switches_apply_absolute_non_accumulating_corrections() {
        let mut world = World::default();
        let avc_id = world.add_component(AvatarControlComponent::new());
        let controller_id = world.add_component(ControllerXRComponent::new(
            true,
            ControllerHand::Left,
            crate::engine::ecs::component::ControllerPoseKind::GripAim,
        ));
        let raw = world.add_component(TransformComponent::new());
        let visual = world.add_component(TransformComponent::new());
        world.add_child(avc_id, controller_id).unwrap();
        world.add_child(controller_id, raw).unwrap();
        world.add_child(raw, visual).unwrap();
        let correction = [0.0, 0.70710677, 0.0, 0.70710677];
        {
            let avc = world
                .get_component_by_id_as_mut::<AvatarControlComponent>(avc_id)
                .unwrap();
            avc.left_hand_raw_target_id = Some(raw);
            avc.left_hand_visual_target_id = Some(visual);
            avc.left_hand_aim_correction = Some(correction);
        }
        let mut emitted = RecordingEmitter::default();
        for (source, expected_rotation) in [
            (ControllerPoseSource::ControllerGripAim, correction),
            (ControllerPoseSource::WristPalm, correction),
            (ControllerPoseSource::ControllerAim, correction),
            (ControllerPoseSource::ControllerGripAim, correction),
            (ControllerPoseSource::None, [0.0, 0.0, 0.0, 1.0]),
            (ControllerPoseSource::WristPalm, correction),
        ] {
            world
                .get_component_by_id_as_mut::<ControllerXRComponent>(controller_id)
                .unwrap()
                .active_pose_source = source;
            update_hand_pose_corrections(avc_id, &world, &mut emitted, false);
            let transform = world
                .get_component_by_id_as_mut::<TransformComponent>(visual)
                .unwrap();
            transform.transform.rotation = expected_rotation;
            transform.transform.recompute_model();
        }
        assert_eq!(
            emitted.0,
            vec![correction, [0.0, 0.0, 0.0, 1.0], correction,]
        );
    }

    #[test]
    fn arm_ik_eligibility_uses_the_xr_hand_direct_tracked_transform() {
        let mut world = World::default();
        let upper = world.add_component(TransformComponent::new());
        let lower = world.add_component(TransformComponent::new());
        let hand = world.add_component(TransformComponent::new());
        let report = left_arm_report(upper, lower, hand);

        let controller = world.add_component(ControllerXRComponent::new(
            true,
            ControllerHand::Left,
            crate::engine::ecs::component::ControllerPoseKind::GripAim,
        ));
        let tracked = world.add_component(TransformComponent::new());
        let attachment = world.add_component(RestAttachmentComponent::new(
            ComponentRef::Query("#hand".into()),
            ComponentRef::Query("#tip".into()),
        ));
        let pointer = world.add_component(crate::engine::ecs::component::PointerComponent::new());
        world.add_child(controller, tracked).unwrap();
        world.add_child(tracked, attachment).unwrap();
        world.add_child(attachment, pointer).unwrap();

        let eligibility = classify_arm_ik_eligibility(&world, &report, Some(controller), true);
        let ArmIkEligibility::Eligible(binding) = eligibility else {
            panic!("expected eligible arm, got {eligibility:?}");
        };
        assert_eq!(binding.raw_target, tracked);
        assert_eq!(binding.hand, hand);
        assert_eq!(binding.upper_arm, upper);
        assert_eq!(binding.lower_arm, lower);
        assert_eq!(world.parent_of(tracked), Some(controller));
    }

    #[test]
    fn complete_arm_without_hand_driver_is_not_eligible() {
        let mut world = World::default();
        let upper = world.add_component(TransformComponent::new());
        let lower = world.add_component(TransformComponent::new());
        let hand = world.add_component(TransformComponent::new());
        let report = left_arm_report(upper, lower, hand);

        assert_eq!(
            classify_arm_ik_eligibility(&world, &report, None, true),
            ArmIkEligibility::NoHandDriver
        );
    }

    #[test]
    fn missing_direct_tracked_transform_is_malformed_not_an_origin_target() {
        let mut world = World::default();
        let upper = world.add_component(TransformComponent::new());
        let lower = world.add_component(TransformComponent::new());
        let hand = world.add_component(TransformComponent::new());
        let report = left_arm_report(upper, lower, hand);
        let controller = world.add_component(ControllerXRComponent::new(
            true,
            ControllerHand::Left,
            crate::engine::ecs::component::ControllerPoseKind::GripAim,
        ));
        let wrapper = world.add_component(RestAttachmentComponent::new(
            ComponentRef::Query("#hand".into()),
            ComponentRef::Query("#tip".into()),
        ));
        let nested = world.add_component(TransformComponent::new());
        world.add_child(controller, wrapper).unwrap();
        world.add_child(wrapper, nested).unwrap();

        assert_eq!(
            classify_arm_ik_eligibility(&world, &report, Some(controller), true),
            ArmIkEligibility::MalformedHandDriver
        );
    }
}

#[cfg(test)]
mod capsule_tests {
    use super::*;
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::component::{CollisionShape, MeshComponent, RenderableComponent};
    use crate::engine::ecs::system::TransformStreamSystem;
    use crate::engine::graphics::mesh::MeshFactory;

    fn attach(world: &mut World, parent: ComponentId, child: ComponentId) {
        world.set_parent(child, Some(parent)).unwrap();
    }

    #[test]
    fn rest_relative_height_ignores_stale_xr_world_matrices_and_live_pose() {
        let mut world = World::default();
        let model_root = world.add_component(TransformComponent::new());
        let hips = world.add_component(
            TransformComponent::new()
                .with_position(0.0, 99.0, 0.0)
                .with_rotation_euler(0.0, 0.4, 0.0),
        );
        let hips_rest = world.add_component(BoneRestPoseComponent::new(
            [0.0, 0.9, 0.0],
            [0.0, 0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
        ));
        let head = world.add_component(TransformComponent::new().with_position(0.0, -42.0, 0.0));
        let head_rest = world.add_component(BoneRestPoseComponent::new(
            [0.0, 0.7, 0.0],
            [0.0, 0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
        ));
        attach(&mut world, model_root, hips);
        attach(&mut world, hips, hips_rest);
        attach(&mut world, hips, head);
        attach(&mut world, head, head_rest);

        world
            .get_component_by_id_as_mut::<TransformComponent>(model_root)
            .unwrap()
            .transform
            .matrix_world[3][1] = 1.75;
        world
            .get_component_by_id_as_mut::<TransformComponent>(head)
            .unwrap()
            .transform
            .matrix_world[3][1] = 0.0;

        let relative = rest_model_relative_to(&world, model_root, head).unwrap();
        assert!((relative[3][1] - 1.6).abs() < 1e-6);
    }

    #[test]
    fn generated_capsule_uses_height_once_and_routes_desktop_movement() {
        let mut world = World::default();
        let assets = RenderAssets::new();
        let driven = world.add_component(TransformComponent::new());
        let avc = world.add_component(AvatarControlComponent::new());
        let model = world.add_component(TransformComponent::new());
        let wide_mesh = world.add_component(
            TransformComponent::new()
                .with_position(0.0, 0.5, 0.0)
                .with_scale(20.0, 3.0, 8.0),
        );
        let renderable = world.add_component(RenderableComponent::cube());
        attach(&mut world, driven, avc);
        attach(&mut world, avc, model);
        attach(&mut world, model, wide_mesh);
        attach(&mut world, wide_mesh, renderable);

        let mut queue = CommandQueue::new();
        try_init_or_route_capsule(avc, &mut world, &assets, &mut queue);
        try_init_or_route_capsule(avc, &mut world, &assets, &mut queue);

        let state = world
            .get_component_by_id_as::<AvatarControlComponent>(avc)
            .unwrap();
        let capsule_t = state.capsule_transform_id.unwrap();
        let response_id = state.capsule_response_id.unwrap();
        let collision = world
            .children_of(capsule_t)
            .iter()
            .copied()
            .find(|id| {
                world
                    .get_component_by_id_as::<CollisionComponent>(*id)
                    .is_some()
            })
            .unwrap();
        let shapes: Vec<_> = world
            .children_of(collision)
            .iter()
            .filter_map(|id| world.get_component_by_id_as::<CollisionShapeComponent>(*id))
            .collect();
        assert_eq!(shapes.len(), 1);
        assert_eq!(shapes[0].shape, CollisionShape::capsule_y(0.28, 1.22));
        let response = world
            .get_component_by_id_as::<CollisionResponseComponent>(response_id)
            .unwrap();
        assert_eq!(response.movement_target_id, Some(driven));

        let fork = world.parent_of(capsule_t).unwrap();
        let arbitrary_pose = TransformComponent::new()
            .with_position(3.0, 4.0, 5.0)
            .with_rotation_euler(0.7, -0.4, 0.9)
            .with_scale(2.0, 3.0, 4.0)
            .transform
            .model;
        let (upright, outputs) = TransformStreamSystem::new()
            .evaluate_stream_node(&world, fork, arbitrary_pose)
            .unwrap();
        assert_eq!(outputs, vec![capsule_t]);
        assert_eq!(
            [upright[3][0], upright[3][1], upright[3][2]],
            [3.0, 4.0, 5.0]
        );
        assert_eq!(upright[0], [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(upright[1], [0.0, 1.0, 0.0, 0.0]);
        assert_eq!(upright[2], [0.0, 0.0, 1.0, 0.0]);
    }

    #[test]
    fn disabled_and_delayed_fallback_behave_deterministically() {
        let assets = RenderAssets::new();
        let mut queue = CommandQueue::new();

        let mut disabled_world = World::default();
        let avc =
            disabled_world.add_component(AvatarControlComponent::new().with_collision_disabled());
        let model = disabled_world.add_component(TransformComponent::new());
        attach(&mut disabled_world, avc, model);
        try_init_or_route_capsule(avc, &mut disabled_world, &assets, &mut queue);
        assert!(
            disabled_world
                .get_component_by_id_as::<AvatarControlComponent>(avc)
                .unwrap()
                .capsule_transform_id
                .is_none()
        );

        let mut world = World::default();
        let avc = world.add_component(AvatarControlComponent::new().with_avatar_height(1.4));
        let model = world.add_component(TransformComponent::new());
        let gltf = world.add_component(GLTFComponent::new("missing.glb"));
        attach(&mut world, avc, model);
        attach(&mut world, model, gltf);
        try_init_or_route_capsule(avc, &mut world, &assets, &mut queue);
        assert!(
            world
                .get_component_by_id_as::<AvatarControlComponent>(avc)
                .unwrap()
                .capsule_transform_id
                .is_none()
        );
        world
            .get_component_by_id_as_mut::<GLTFComponent>(gltf)
            .unwrap()
            .spawned = true;
        try_init_or_route_capsule(avc, &mut world, &assets, &mut queue);
        assert!(
            world
                .get_component_by_id_as::<AvatarControlComponent>(avc)
                .unwrap()
                .capsule_transform_id
                .is_some()
        );
    }

    #[test]
    fn generated_capsule_uses_imported_mesh_bounds() {
        let mut world = World::default();
        let mut assets = RenderAssets::new();
        let mut queue = CommandQueue::new();
        let driven = world.add_component(TransformComponent::new());
        let avc = world.add_component(AvatarControlComponent::new());
        let model = world.add_component(TransformComponent::new());
        let tall_shape = world.add_component(TransformComponent::new().with_scale(1.0, 2.0, 1.0));
        let renderable = world.add_component(RenderableComponent::triangle());
        let mesh = world.add_component(MeshComponent::new("avatar:body:prim0"));
        attach(&mut world, driven, avc);
        attach(&mut world, avc, model);
        attach(&mut world, model, tall_shape);
        attach(&mut world, tall_shape, renderable);
        attach(&mut world, renderable, mesh);

        assets.register_imported_mesh("avatar:body:prim0", MeshFactory::cube());
        try_init_or_route_capsule(avc, &mut world, &assets, &mut queue);

        let capsule_t = world
            .get_component_by_id_as::<AvatarControlComponent>(avc)
            .unwrap()
            .capsule_transform_id
            .expect("capsule after imported mesh registration");
        let collision = world
            .children_of(capsule_t)
            .iter()
            .copied()
            .find(|id| {
                world
                    .get_component_by_id_as::<CollisionComponent>(*id)
                    .is_some()
            })
            .expect("generated collision");
        let shape = world
            .children_of(collision)
            .iter()
            .find_map(|id| world.get_component_by_id_as::<CollisionShapeComponent>(*id))
            .expect("generated collision shape");
        assert_eq!(shape.shape, CollisionShape::capsule_y(0.28, 0.72));
    }

    #[test]
    fn xr_routes_to_transform_above_input_xr() {
        let mut world = World::default();
        let outer = world.add_component(TransformComponent::new());
        let input_xr = world.add_component(InputXRComponent::on());
        let driven = world.add_component(TransformComponent::new());
        let avc = world.add_component(AvatarControlComponent::new());
        attach(&mut world, outer, input_xr);
        attach(&mut world, input_xr, driven);
        attach(&mut world, driven, avc);
        assert_eq!(automatic_avc_movement_target(&world, avc), Some(outer));
    }
}
