//! One-shot pose-driver initialization for a newly bound avatar. The steps
//! run in order: their queued transform intents depend on it.
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

    let (left_ctrl, right_ctrl) = discover_hand_controllers(world, id);

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

    let left_eligibility = classify_arm_ik_eligibility(world, humanoid_map, left_ctrl, true);
    let right_eligibility = classify_arm_ik_eligibility(world, humanoid_map, right_ctrl, false);
    let Some(left) = prepare_hand(
        world,
        retargeting,
        humanoid_map,
        model_root_id,
        left_eligibility,
        true,
        "left",
    ) else {
        return;
    };
    let Some(right) = prepare_hand(
        world,
        retargeting,
        humanoid_map,
        model_root_id,
        right_eligibility,
        false,
        "right",
    ) else {
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
    let camera_children = discover_camera_children(world, id);
    camera_view::bind_camera_views(world, id, &camera_children);
    let eye_offset_head_local = eye_offset_head_local(
        world,
        id,
        &camera_children,
        camera_bone_id,
        head_bone_id,
        eye_height_from_head_bone,
    );

    // Eye offset mapped from head-local into driven_t-local space.
    // This remains the source for the head target offset. It no longer owns
    // body/root placement; the native constraint owns supported rigs and the
    // legacy follow calculation handles its mutually exclusive fallback.
    let head_ik_offset_yaw = if resolved_head_target_forward_plus_z {
        0.0
    } else {
        std::f32::consts::PI
    };

    let model_root_translation = calibrate_model_root(
        model_root_id,
        world,
        camera_bone_id,
        avatar_height_override,
        emit,
    );
    let (neck_bone_id, neck_rest_t) = neck_rest(world, humanoid_map, neck_pin_enabled);

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
    mount_hand_helpers(world, retargeting, left, right, emit);

    if !skip_body_operator {
        create_body_operator(
            world,
            id,
            model_root_id,
            body_yaw_threshold,
            body_yaw_rate,
            resolved_initial_body_yaw,
            resolved_body_forward_plus_z,
            emit,
        );
    }

    mount_head(
        world,
        id,
        HeadMountPlan {
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
        },
        humanoid_map,
        emit,
    );

    create_arm_chains(
        world,
        id,
        [
            (left, left_two_bone_ik, "left"),
            (right, right_two_bone_ik, "right"),
        ],
        emit,
    );

    reparent_cameras(world, camera_bone_id, &camera_children, emit);
}
