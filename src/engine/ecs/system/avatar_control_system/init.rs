//! One-shot pose-driver initialization for a newly bound avatar.
use super::*;

/// Pose-driven embodiment: create body/head targets and optional hand IK.
/// Both Input and InputXR supply the authoritative spliced head target.
///
/// Controllers are discovered by topology: any `ControllerXRComponent` that is a
/// **direct child** of this `AvatarControlComponent` is treated as a hand driver.
/// Its `hand` field (`Left` / `Right`) determines which hand bone it drives.
///
/// Body operator created here reads `driven_t`'s world matrix, strips pitch/roll via `YawFollow`,
/// and writes the result to `model_root` (which is re-parented under the operator output).
pub(super) fn try_init_pose_drivers(
    id: ComponentId,
    world: &mut World,
    retargeting: &mut JointBasisRetargetingSystem,
    humanoid_map: &HumanoidBoneMapReport,
    emit: &mut dyn SignalEmitter,
) {
    let (
        left_two_bone_ik,
        right_two_bone_ik,
        body_yaw_threshold,
        body_yaw_rate,
        authored_forward_plus_z,
        forward_plus_z_overridden,
        authored_initial_body_yaw,
        initial_body_yaw_overridden,
        skip_body_operator,
        avatar_height_override,
        eye_height_from_head_bone,
        head_ik_eye_height,
        neck_pin_enabled,
    ) = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
            return;
        };
        (
            c.left_two_bone_ik
                .clone()
                .unwrap_or_else(crate::engine::ecs::component::ArmTwoBoneIkConfig::left_default),
            c.right_two_bone_ik
                .clone()
                .unwrap_or_else(crate::engine::ecs::component::ArmTwoBoneIkConfig::right_default),
            c.body_yaw_threshold,
            c.body_yaw_rate,
            c.forward_plus_z,
            c.forward_plus_z_overridden,
            c.initial_body_yaw,
            c.initial_body_yaw_overridden,
            c.skip_body_operator,
            c.avatar_height,
            c.eye_height_from_head_bone,
            c.head_ik_eye_height,
            c.neck_pin_enabled,
        )
    };

    let Some(model_root_id) = model_root(world, id) else {
        return;
    };
    bind_shared_rig(world, id, model_root_id, humanoid_map);

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

    // Eye offset mapped from head-local into driven_t-local space.
    // This remains the source for the head target offset. It no longer owns
    // body/root placement; the native constraint owns supported rigs and the
    // legacy follow calculation handles its mutually exclusive fallback.
    let head_ik_offset_yaw = if resolved_head_target_forward_plus_z {
        0.0
    } else {
        std::f32::consts::PI
    };

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

    // Store runtime IDs (body_operator_id stored after operator creation below).
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

    for ((binding, _, _), correction) in [left, right].into_iter().flatten() {
        ensure_hand_grab_attachment(world, binding.controller, binding.hand, correction, emit);
    }

    // -----------------------------------------------------------------------
    // Body operator: created as a child of AVC; model_root re-parented under it.
    //
    // Topology:
    //   AVC
    //     └── body_operator  (TransformOperatorComponent)
    //           rotation_ops: YawFollow { threshold, rate, initial_yaw, forward_plus_z }
    //           model_root  ← re-parented here
    // -----------------------------------------------------------------------
    if !skip_body_operator {
        use crate::engine::ecs::component::transform_operator::TransformQuatOp;
        let mut body_operator = TransformOperatorComponent::fork_trs();
        body_operator.push_rotation(TransformQuatOp::YawFollow {
            threshold: body_yaw_threshold,
            rate: body_yaw_rate,
            initial_yaw: resolved_initial_body_yaw,
            forward_plus_z: resolved_body_forward_plus_z,
        });
        let body_operator_id = world.add_component(body_operator);
        let body_operator_serialize_id = world.add_component(SerializeComponent::off());
        let _ = world.set_parent(body_operator_serialize_id, Some(body_operator_id));

        if let Some(c) = world.get_component_by_id_as_mut::<AvatarControlComponent>(id) {
            c.body_operator_id = Some(body_operator_id);
        }

        emit_attach(emit, id, body_operator_id);
        emit_attach(emit, body_operator_id, model_root_id);
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
    for (hand_opt, ik_config, side_label) in [
        (left, left_two_bone_ik, "left"),
        (right, right_two_bone_ik, "right"),
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
                pole_direction: ik_config.pole_direction,
                copy_end_rotation: ik_config.copy_end_rotation,
            },
            hand_driver,
            hand_bone,
        );
        chain.weight = ik_config.weight;
        chain.forbidden_bend_normal_z_degrees = ik_config.forbidden_bend_normal_z_degrees;
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
