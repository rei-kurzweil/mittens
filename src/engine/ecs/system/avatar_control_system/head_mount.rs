//! Head mount, camera discovery, eye offset, and model-root height calibration.
use super::*;

/// Discover camera children (direct, or one wrapper deep) with their authored
/// eye offsets. The bool marks the desktop (`Camera3D`) path.
pub(super) fn discover_camera_children(
    world: &World,
    id: ComponentId,
) -> Vec<(ComponentId, [f32; 3], bool)> {
    world
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
        .collect()
}

/// The eye origin in head-bone local space. It feeds the head target offset.
pub(super) fn eye_offset_head_local(
    world: &World,
    id: ComponentId,
    camera_children: &[(ComponentId, [f32; 3], bool)],
    camera_bone_id: Option<ComponentId>,
    head_bone_id: ComponentId,
    eye_height_from_head_bone: Option<f32>,
) -> [f32; 3] {
    let explicit_eye_offset = world
        .get_component_by_id_as::<AvatarControlComponent>(id)
        .and_then(|avatar| avatar.eye_offset);
    let authored_eye_offset = explicit_eye_offset.unwrap_or_else(|| {
        camera_children
            .iter()
            .map(|&(_, off, _)| off)
            .find(|off| off != &[0.0, 0.0, 0.0])
            .unwrap_or([0.0, eye_height_from_head_bone.unwrap_or(0.0), 0.0])
    });
    // The wrapper is attached beneath the mapped camera anchor, not directly
    // beneath the head. Include that rest path (including helper rotations)
    // before converting the eye origin back to the displaced head pivot.
    let anchor_in_head = camera_bone_id
        .and_then(|anchor| rest_model_relative_to(world, head_bone_id, anchor))
        .unwrap_or_else(mat4_identity);
    let eye = mat4_mul_vec4(
        anchor_in_head,
        [
            authored_eye_offset[0],
            authored_eye_offset[1],
            authored_eye_offset[2],
            1.0,
        ],
    );
    let eye_offset_head_local = [eye[0], eye[1], eye[2]];
    eye_offset_head_local
}

/// Place `model_root` from the avatar height override or the rest-pose camera
/// anchor height, and return the translation it was given.
pub(super) fn calibrate_model_root(
    model_root_id: ComponentId,
    world: &World,
    camera_bone_id: Option<ComponentId>,
    avatar_height_override: Option<f32>,
    emit: &mut dyn SignalEmitter,
) -> Option<[f32; 3]> {
    // Body Y is anchored to `displaced_head.world.y` (which already has
    // -eye_offset.y baked in via the head_target chain) in
    // the legacy fallback, so model_root.y must NOT also include an
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
    model_root_translation
}

pub(super) fn neck_rest(
    world: &World,
    humanoid_map: &HumanoidBoneMapReport,
    neck_pin_enabled: bool,
) -> (Option<ComponentId>, Option<[f32; 3]>) {
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
    (neck_bone_id, neck_rest_t)
}

/// Inputs for the visible-head mount and head/neck/body constraint binding.
pub(super) struct HeadMountPlan {
    pub(super) model_root_id: ComponentId,
    pub(super) driven_t_id: ComponentId,
    pub(super) head_bone_id: ComponentId,
    pub(super) neck_bone_id: Option<ComponentId>,
    pub(super) head_rest_rot: [f32; 4],
    pub(super) head_rest_s: [f32; 3],
    pub(super) eye_offset_head_local: [f32; 3],
    pub(super) head_ik_eye_height: Option<f32>,
    pub(super) head_ik_offset_yaw: f32,
    pub(super) resolved_initial_body_yaw: f32,
}

/// Bind the head/neck/body constraint, then splice the head bone under a
/// calibrated mount beneath `driven_t`.
pub(super) fn mount_head(
    world: &mut World,
    id: ComponentId,
    plan: HeadMountPlan,
    humanoid_map: &HumanoidBoneMapReport,
    emit: &mut dyn SignalEmitter,
) {
    let HeadMountPlan {
        model_root_id,
        driven_t_id,
        head_bone_id,
        neck_bone_id,
        head_rest_rot,
        head_rest_s,
        eye_offset_head_local,
        head_ik_eye_height,
        head_ik_offset_yaw,
        resolved_initial_body_yaw,
    } = plan;
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
    let scaled_neg_eye = std::array::from_fn(|i| neg_eye[i] * head_rest_s[i]);
    let head_target_offset = quat_rotate_vec3(
        quat_mul(quat_rotation_y(head_ik_offset_yaw), head_rest_rot),
        scaled_neg_eye,
    );

    // Capture the original head FK segment before the attach intents splice it.
    // The constraint replaces legacy follow only after this calibration validates.
    let binding = neck_bone_id
        .ok_or("mapped neck/rest pin unavailable")
        .and_then(|neck| {
            super::super::ik::HeadNeckBodyConstraint::bind(
                world,
                model_root_id,
                neck,
                head_bone_id,
                driven_t_id,
                head_target_offset,
                quat_rotation_y(resolved_initial_body_yaw),
                humanoid_map.generation,
            )
        });
    if let Some(avc) = world.get_component_by_id_as_mut::<AvatarControlComponent>(id) {
        match binding {
            Ok(constraint) => {
                avc.head_neck_body_constraint = Some(constraint);
                avc.head_neck_body_bind_error = None;
            }
            Err(reason) => avc.head_neck_body_bind_error = Some(reason),
        }
    }

    // Calibrated visible-head mount under driven_t. Native desktop pose solving
    // can move this mount along the connected head arc without moving Input.
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
}

/// Move discovered cameras under the camera anchor so they inherit its world
/// transform each tick.
pub(super) fn reparent_cameras(
    world: &mut World,
    camera_bone_id: Option<ComponentId>,
    camera_children: &[(ComponentId, [f32; 3], bool)],
    emit: &mut dyn SignalEmitter,
) {
    // -----------------------------------------------------------------------
    // Camera re-parenting: move discovered Camera3D/CameraXR children of AVC
    // under the camera bone so they inherit its world transform each tick.
    // -----------------------------------------------------------------------
    if let Some(cam_bone_id) = camera_bone_id {
        for &(cam, _eye_offset, is_desktop_camera_path) in camera_children {
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
