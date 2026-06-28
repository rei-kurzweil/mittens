use crate::engine::ecs::component::{
    AvatarControlComponent, BoneRestPoseComponent, Camera3DComponent, CameraXRComponent,
    ControllerHand, ControllerXRComponent, IKChainComponent, IKSolver, QuatYawFollowComponent,
    SerializeComponent, TransformComponent, TransformForkTRSComponent,
    TransformMapRotationComponent,
};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::utils::math::{quat_rotate_vec3, quat_rotation_y};

#[derive(Debug, Default)]
pub struct AvatarControlSystem;

impl AvatarControlSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn tick(&mut self, world: &mut World, emit: &mut dyn SignalEmitter, dt_sec: f32) {
        let ids: Vec<ComponentId> = world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<AvatarControlComponent>(id)
                    .is_some()
            })
            .collect();

        for id in ids {
            tick_one(id, world, emit, dt_sec);
        }
    }
}

fn tick_one(id: ComponentId, world: &mut World, emit: &mut dyn SignalEmitter, _dt_sec: f32) {
    // --- Init phase ---
    let needs_init = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
            return;
        };
        c.splice_head.is_none()
    };

    if needs_init {
        try_init_splices(id, world, emit);
        // Head rotation is handled by IKSystem (AimConstraint on splice_head) after init.
    }

    // Keep the displaced head bone anchored under splice_head. This prevents
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
                        component_ids: vec![head_bone_id],
                        translation: [0.0, 0.0, 0.0],
                        rotation_quat_xyzw: head_t.transform.rotation,
                        scale: head_t.transform.scale,
                    },
                );
            }
        }
    }
}

/// First-time setup: splice bones, create body pipeline, and (optionally) hand smoothing pipelines.
///
/// Controllers are discovered by topology: any `ControllerXRComponent` that is a
/// **direct child** of this `AvatarControlComponent` is treated as a hand driver.
/// Its `hand` field (`Left` / `Right`) determines which hand bone it drives.
///
/// Body pipeline created here reads `driven_t`'s world matrix, strips pitch/roll via `YawFollow`,
/// and writes the result to `model_root` (which is re-parented under the pipeline output).
fn try_init_splices(id: ComponentId, world: &mut World, emit: &mut dyn SignalEmitter) {
    let (
        head_bone_name,
        left_hand_bone,
        right_hand_bone,
        left_upper_arm_bone,
        left_lower_arm_bone,
        right_upper_arm_bone,
        right_lower_arm_bone,
        left_arm_pole_direction,
        right_arm_pole_direction,
        body_yaw_threshold,
        body_yaw_rate,
        authored_forward_plus_z,
        forward_plus_z_overridden,
        authored_initial_body_yaw,
        initial_body_yaw_overridden,
        skip_body_pipeline,
        camera_bone_name,
        avatar_height_override,
        eye_height_from_head_bone,
        head_ik_eye_height,
        neck_bone_name,
        hand_grip_rotation_left,
        hand_grip_rotation_right,
    ) = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
            return;
        };
        (
            c.head_bone.clone(),
            c.left_hand_bone.clone(),
            c.right_hand_bone.clone(),
            c.left_upper_arm_bone.clone(),
            c.left_lower_arm_bone.clone(),
            c.right_upper_arm_bone.clone(),
            c.right_lower_arm_bone.clone(),
            c.left_arm_pole_direction,
            c.right_arm_pole_direction,
            c.body_yaw_threshold,
            c.body_yaw_rate,
            c.forward_plus_z,
            c.forward_plus_z_overridden,
            c.initial_body_yaw,
            c.initial_body_yaw_overridden,
            c.skip_body_pipeline,
            c.camera_bone.clone(),
            c.avatar_height,
            c.eye_height_from_head_bone,
            c.head_ik_eye_height,
            c.neck_bone.clone(),
            c.hand_grip_rotation_left,
            c.hand_grip_rotation_right,
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

    // driven_t is the parent of AVC — needed as IK target for the head AimConstraint.
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
    let head_selector = format!("#{}", head_bone_name);
    let Some(head_bone_id) = world.find_component(model_root_id, &head_selector) else {
        return;
    };
    let Some(head_parent_id) = world.parent_of(head_bone_id) else {
        return;
    };

    // Read head_bone's true bind-pose local TRS via the `BoneRestPoseComponent`
    // sidecar that `GLTFSystem` stamped at node-spawn time.  Falls back to the
    // current `TransformComponent` only if no rest-pose sidecar is present
    // (non-GLTF skeletons).  Reading the live `TransformComponent` would
    // pick up whatever pose `AnimationSystem` wrote earlier this tick, which
    // bakes the current animation frame into `head_rest_rot` and produces a
    // permanently rotated visible head.
    let (head_rest_t, head_rest_rot, head_rest_s) = read_bone_rest_pose(world, head_bone_id);
    let head_splice_id = world.add_component(TransformComponent::new().with_position(
        head_rest_t[0],
        head_rest_t[1],
        head_rest_t[2],
    ));

    // Resolve hand bones + controller drivers for 2-bone arm IK.
    // The hand bone stays in the armature; IKSystem rotates UpperArm + LowerArm
    // each tick so the hand reaches the controller world pose, optionally
    // through a rotated child target that compensates for grip-vs-palm framing.
    let left = resolve_hand_splice(
        world,
        model_root_id,
        left_hand_bone.as_deref(),
        left_ctrl,
        hand_grip_rotation_left,
    );
    let right = resolve_hand_splice(
        world,
        model_root_id,
        right_hand_bone.as_deref(),
        right_ctrl,
        hand_grip_rotation_right,
    );

    // --- Camera bone: auto-calibrate model_root.y + discover camera children ---
    //
    // Priority:
    //   1. avatar_height_override — use directly, skip bone measurement.
    //   2. camera_bone auto-calibration — measure bone local Y in rest pose.
    // Either way, emit UpdateTransform(model_root, y = -height).
    //
    // Any Camera3D or CameraXR direct children of AVC are re-parented under the
    // camera bone so they inherit its world transform each tick.
    let actual_camera_bone_name = camera_bone_name.as_deref().or(Some(&head_bone_name));
    let camera_bone_id: Option<ComponentId> = actual_camera_bone_name.and_then(|name| {
        let sel = format!("#{}", name);
        let found = world.find_component(model_root_id, &sel);
        if found.is_none() && camera_bone_name.is_some() {
            println!(
                "[AVC] camera_bone '{}' not found under model_root {:?}",
                name, model_root_id
            );
        }
        found
    });

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
                println!(
                    "[AVC] found bare camera child {:?} — re-parent to camera_bone (no eye offset)",
                    ch
                );
                return Some((ch, [0.0, 0.0, 0.0], is_c3d));
            }
            if let Some(tc) = world.get_component_by_id_as::<TransformComponent>(ch) {
                let wraps_c3d = world.children_of(ch).iter().any(|&gc| {
                    world.get_component_by_id_as::<Camera3DComponent>(gc).is_some()
                });
                let wraps_cxr = world.children_of(ch).iter().any(|&gc| {
                    world.get_component_by_id_as::<CameraXRComponent>(gc).is_some()
                });
                let wraps_cam = wraps_c3d || wraps_cxr;
                if wraps_cam {
                    let eye_offset = tc.transform.translation;
                    println!(
                        "[AVC] found T-wrapped camera child {:?} — eye_offset = {:?}",
                        ch, eye_offset
                    );
                    return Some((ch, eye_offset, wraps_c3d));
                }
            }
            None
        })
        .collect();
    if camera_children.is_empty() && camera_bone_id.is_some() {
        println!(
            "[AVC] WARNING: camera_bone set but no Camera3D/CameraXR direct children of AVC found"
        );
    }
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
        println!("[AVC] using avatar_height_override = {}", h);
        Some([0.0, -h, 0.0])
    } else if let Some(cam_bone_id) = camera_bone_id {
        let cam_bone_world_y = world
            .get_component_by_id_as::<TransformComponent>(cam_bone_id)
            .map(|t| t.transform.matrix_world[3][1])
            .unwrap_or(0.0);
        let model_root_world_y = world
            .get_component_by_id_as::<TransformComponent>(model_root_id)
            .map(|t| t.transform.matrix_world[3][1])
            .unwrap_or(0.0);
        let bone_local_y = cam_bone_world_y - model_root_world_y;
        println!(
            "[AVC] camera_bone found: cam_bone_world_y={:.4} model_root_world_y={:.4} bone_local_y={:.4} → model_root.y={:.4}",
            cam_bone_world_y, model_root_world_y, bone_local_y, -bone_local_y
        );
        Some([0.0, -bone_local_y, 0.0])
    } else {
        if camera_bone_name.is_some() || actual_camera_bone_name.is_some() {
            println!(
                "[AVC] camera_bone (or fallback) not found and no avatar_height_override — model_root.y unchanged"
            );
        }
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
                component_ids: vec![model_root_id],
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
    let (neck_bone_id, neck_rest_t) = match neck_bone_name.as_deref() {
        Some(name) => {
            let sel = format!("#{}", name);
            match world.find_component(model_root_id, &sel) {
                Some(nid) => {
                    let (rest_t, _, _) = read_bone_rest_pose(world, nid);
                    (Some(nid), Some(rest_t))
                }
                None => {
                    println!(
                        "[AVC] neck bone '{}' not found under model_root — neck pin disabled",
                        name
                    );
                    (None, None)
                }
            }
        }
        None => (None, None),
    };

    // Y component of model_root.local stashed for the body-follow system's
    // future Step 1 (head-rotation-compensated world XZ target).  Step 0
    // doesn't use it; the body relies on the AVC-init single-shot
    // UpdateTransform plus the parent-chain transform inheritance.
    let model_root_local_y = model_root_translation.map(|t| t[1]).unwrap_or(0.0);

    // Store runtime IDs (body_pipeline_id stored after pipeline creation below).
    if let Some(c) = world.get_component_by_id_as_mut::<AvatarControlComponent>(id) {
        c.splice_head = Some(head_splice_id);
        c.displaced_head = Some(head_bone_id);
        c.splice_camera_bone = camera_bone_id;
        c.model_root_id = Some(model_root_id);
        c.model_root_local_y = model_root_local_y;
        c.neck_bone_id = neck_bone_id;
        c.neck_rest_translation = neck_rest_t;
        if let Some((_, _, _, bone)) = left {
            c.left_hand_bone_id = Some(bone);
        }
        if let Some((_, _, _, bone)) = right {
            c.right_hand_bone_id = Some(bone);
        }
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

    emit_attach(emit, head_parent_id, head_splice_id);
    emit_attach(emit, driven_t_id, head_target_id);
    emit_attach(emit, head_target_id, head_bone_id);

    // Zero head_bone's local translation — splice_head now carries the rest offset
    // from neck. Preserve the authored head rest rotation/scale so the visible
    // head mesh and camera anchor share the same convention across desktop and XR.
    // Emitted *after* the reparent attach so the UpdateTransform lands on
    // head_bone in its new parent without fighting the attach intent's matrix recompute.
    emit.push_intent_now(
        head_bone_id,
        IntentValue::UpdateTransform {
            component_ids: vec![head_bone_id],
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
    // Resolution per bone:
    //   - if `*_upper_arm_bone` / `*_lower_arm_bone` set → name lookup under
    //     model_root (skip the chain if the name is wrong — fail loudly).
    //   - else fall back to `parent_of` walk-up from the hand bone (works for
    //     clean VRM-style rigs with no twist bones).
    // -----------------------------------------------------------------------
    for (hand_opt, upper_name, lower_name, pole_dir, side_label, grip_rotation_offset) in [
        (
            left,
            left_upper_arm_bone.as_deref(),
            left_lower_arm_bone.as_deref(),
            left_arm_pole_direction,
            "left",
            hand_grip_rotation_left,
        ),
        (
            right,
            right_upper_arm_bone.as_deref(),
            right_lower_arm_bone.as_deref(),
            right_arm_pole_direction,
            "right",
            hand_grip_rotation_right,
        ),
    ] {
        let Some((_, raw_driver, hand_driver, hand_bone)) = hand_opt else {
            continue;
        };

        let upper_arm = match upper_name {
            Some(name) => {
                let sel = format!("#{}", name);
                let res = world.find_component(model_root_id, &sel);
                if res.is_none() {
                    println!(
                        "[AVC] explicit {}_upper_arm_bone \"{}\" not found under model_root — {} arm IK disabled",
                        side_label, name, side_label
                    );
                }
                res
            }
            None => world
                .parent_of(hand_bone)
                .and_then(|lower| world.parent_of(lower)),
        };
        let Some(upper_arm) = upper_arm else { continue };

        let lower_arm = match lower_name {
            Some(name) => {
                let sel = format!("#{}", name);
                let res = world.find_component(model_root_id, &sel);
                if res.is_none() {
                    println!(
                        "[AVC] explicit {}_lower_arm_bone \"{}\" not found under model_root — {} arm IK disabled",
                        side_label, name, side_label
                    );
                }
                res
            }
            None => world.parent_of(hand_bone),
        };
        let Some(lower_arm) = lower_arm else { continue };

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
                set explicit {}_upper_arm_bone(\"...\") and {}_lower_arm_bone(\"...\") \
                in your AvatarControl block.",
                side_label, side_label, side_label
            );
        }

        let chain = IKChainComponent::new(
            IKSolver::TwoBoneIK {
                root_joint_id: upper_arm,
                mid_joint_id: lower_arm,
                pole_direction: pole_dir,
                copy_end_rotation: true,
            },
            hand_driver,
            hand_bone,
        );
        let chain_id = world.add_component(chain);
        let chain_serialize_id = world.add_component(SerializeComponent::off());
        let _ = world.set_parent(chain_serialize_id, Some(chain_id));
        if let Some(offset_q) = grip_rotation_offset {
            println!(
                "[AVC] {} hand IK target rotation offset = {:?}",
                side_label, offset_q
            );
        } else {
            let _ = raw_driver;
        }
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
                    let _ = world.set_parent(desktop_camera_mount_serialize_id, Some(desktop_camera_mount));
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
            "[AVC] WARNING: camera children found but camera_bone not resolved — no re-parenting"
        );
    }
}

/// Find a hand bone by name and determine its raw driver node.
///
/// Returns `(bone_original_parent, raw_driver, hand_driver, bone_id)` or `None` if the bone
/// wasn't found (model may not have this joint — silently skip).
fn resolve_hand_splice(
    world: &mut World,
    model_root: ComponentId,
    bone_name: Option<&str>,
    controller: Option<ComponentId>,
    rotation_offset: Option<[f32; 4]>,
) -> Option<(ComponentId, ComponentId, ComponentId, ComponentId)> {
    let bone_name = bone_name?;
    let sel = format!("#{}", bone_name);
    let bone = world.find_component(model_root, &sel)?;
    let bone_parent = world.parent_of(bone)?;

    let driver = if let Some(ctrl) = controller {
        world
            .children_of(ctrl)
            .iter()
            .copied()
            .find(|&ch| {
                world
                    .get_component_by_id_as::<TransformComponent>(ch)
                    .is_some()
            })
            .unwrap_or_else(|| world.add_component(TransformComponent::new()))
    } else {
        world.add_component(TransformComponent::new())
    };

    let hand_driver = if let Some(offset_q) = rotation_offset {
        let offset = world.add_component(TransformComponent::new().with_rotation_quat(offset_q));
        let offset_serialize_id = world.add_component(SerializeComponent::off());
        let _ = world.set_parent(offset_serialize_id, Some(offset));
        let _ = world.set_parent(offset, Some(driver));
        offset
    } else {
        driver
    };

    Some((bone_parent, driver, hand_driver, bone))
}

fn emit_attach(emit: &mut dyn SignalEmitter, parent: ComponentId, child: ComponentId) {
    emit.push_intent_now(
        parent,
        IntentValue::Attach {
            parents: vec![parent],
            child,
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
