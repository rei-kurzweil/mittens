use crate::engine::ecs::component::{
    AvatarControlComponent, Camera3DComponent, CameraXRComponent, ControllerHand,
    ControllerXRComponent, IKChainComponent, IKSolver, QuatTemporalFilterComponent,
    QuatYawFollowComponent, TransformComponent, TransformForkTRSComponent,
    TransformMapRotationComponent,
};
use crate::engine::ecs::system::bone_mapping_system::BoneMappingSystem;
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
    let (head_bone_name, left_hand_bone, right_hand_bone,
         body_yaw_threshold, body_yaw_rate, forward_plus_z,
         initial_body_yaw, hand_rotation_smoothing, skip_body_pipeline,
         camera_bone_name, avatar_height_override, eye_height_from_head_bone,
         head_ik_eye_height,
         hips_bone_name,
         left_upper_arm_bone, left_lower_arm_bone,
         right_upper_arm_bone, right_lower_arm_bone) = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
            return;
        };
        (
            c.head_bone.clone(),
            c.left_hand_bone.clone(),
            c.right_hand_bone.clone(),
            c.body_yaw_threshold,
            c.body_yaw_rate,
            c.forward_plus_z,
            c.initial_body_yaw,
            c.hand_rotation_smoothing,
            c.skip_body_pipeline,
            c.camera_bone.clone(),
            c.avatar_height,
            c.eye_height_from_head_bone,
            c.head_ik_eye_height,
            c.hips_bone.clone().or_else(|| Some("J_Bip_C_Hips".to_string())),
            c.left_upper_arm_bone.clone(),
            c.left_lower_arm_bone.clone(),
            c.right_upper_arm_bone.clone(),
            c.right_lower_arm_bone.clone(),
        )
    };

    // Find model_root: first TransformComponent child of AvatarControlComponent.
    let Some(model_root_id) = world
        .children_of(id)
        .iter()
        .copied()
        .find(|&ch| world.get_component_by_id_as::<TransformComponent>(ch).is_some())
    else {
        return;
    };

    // Discover hand controllers by topology: direct ControllerXRComponent children.
    let left_ctrl = world
        .children_of(id)
        .iter()
        .copied()
        .find(|&ch| {
            world
                .get_component_by_id_as::<ControllerXRComponent>(ch)
                .map(|c| c.hand == ControllerHand::Left)
                .unwrap_or(false)
        });
    let right_ctrl = world
        .children_of(id)
        .iter()
        .copied()
        .find(|&ch| {
            world
                .get_component_by_id_as::<ControllerXRComponent>(ch)
                .map(|c| c.hand == ControllerHand::Right)
                .unwrap_or(false)
        });

    // driven_t is the parent of AVC — needed as IK target for the head AimConstraint.
    let Some(driven_t_id) = world.parent_of(id) else { return };

    // Head bone is required — retry next tick if GLTF hasn't spawned yet.
    let head_selector = format!("#{}", head_bone_name);
    let Some(head_bone_id) = world.find_component(model_root_id, &head_selector) else {
        return;
    };
    let Some(head_parent_id) = world.parent_of(head_bone_id) else { return };

    // Read head_bone's REST local TRS before splicing.  The splice_head TC will be
    // stamped with this translation so it sits at the FK rest head position; the
    // head_bone is then re-anchored under splice_head with zero translation.
    // This way splice_head IS the head pivot (FK-positioned by spine bending), and
    // the spine FABRIK chain's last bone length = rest neck-to-head distance.
    let (head_rest_t, head_rest_rot, head_rest_s) = world
        .get_component_by_id_as::<TransformComponent>(head_bone_id)
        .map(|t| (t.transform.translation, t.transform.rotation, t.transform.scale))
        .unwrap_or(([0.0; 3], [0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]));
    let head_splice_id = world.add_component(
        TransformComponent::new().with_position(head_rest_t[0], head_rest_t[1], head_rest_t[2]),
    );

    // Resolve hand splices (raw driver = controller's driven_t or plain TC).
    let left  = resolve_hand_splice(world, model_root_id, left_hand_bone.as_deref(),  left_ctrl);
    let right = resolve_hand_splice(world, model_root_id, right_hand_bone.as_deref(), right_ctrl);

    // Attempt arm chain resolution for TwoBoneIK.  Only attempted when a controller
    // is present — without a real driver the IK target would be stuck at origin.
    let arm_left = if left_ctrl.is_some() {
        left_hand_bone.as_deref().and_then(|hand_name| {
            BoneMappingSystem::resolve_arm_chain(
                world,
                model_root_id,
                hand_name,
                left_lower_arm_bone.as_deref(),
                left_upper_arm_bone.as_deref(),
                Some(0.03),
            )
        })
    } else {
        None
    };
    let arm_right = if right_ctrl.is_some() {
        right_hand_bone.as_deref().and_then(|hand_name| {
            BoneMappingSystem::resolve_arm_chain(
                world,
                model_root_id,
                hand_name,
                right_lower_arm_bone.as_deref(),
                right_upper_arm_bone.as_deref(),
                Some(0.03),
            )
        })
    } else {
        None
    };
    if arm_left.is_some()  { println!("[AVC] left arm chain resolved for TwoBoneIK"); }
    if arm_right.is_some() { println!("[AVC] right arm chain resolved for TwoBoneIK"); }

    // --- Camera bone: auto-calibrate model_root.y + discover camera children ---
    //
    // Priority:
    //   1. avatar_height_override — use directly, skip bone measurement.
    //   2. camera_bone auto-calibration — measure bone local Y in rest pose.
    // Either way, emit UpdateTransform(model_root, y = -height).
    //
    // Any Camera3D or CameraXR direct children of AVC are re-parented under the
    // camera bone so they inherit its world transform each tick.
    let camera_bone_id: Option<ComponentId> = camera_bone_name.as_deref().and_then(|name| {
        let sel = format!("#{}", name);
        let found = world.find_component(model_root_id, &sel);
        if found.is_none() {
            println!("[AVC] camera_bone '{}' not found under model_root {:?}", name, model_root_id);
        }
        found
    });

    // Discover camera children + derive eye_offset_head_local FIRST — the
    // model_root xz compensation below needs the offset, and the eye_offset
    // also feeds the head IK target_position_offset (used much later).
    let camera_children: Vec<(ComponentId, [f32; 3])> = world
        .children_of(id)
        .iter()
        .copied()
        .filter_map(|ch| {
            let is_c3d = world.get_component_by_id_as::<Camera3DComponent>(ch).is_some();
            let is_cxr = world.get_component_by_id_as::<CameraXRComponent>(ch).is_some();
            if is_c3d || is_cxr {
                println!("[AVC] found bare camera child {:?} — re-parent to camera_bone (no eye offset)", ch);
                return Some((ch, [0.0, 0.0, 0.0]));
            }
            if let Some(tc) = world.get_component_by_id_as::<TransformComponent>(ch) {
                let wraps_cam = world.children_of(ch).iter().any(|&gc| {
                    world.get_component_by_id_as::<Camera3DComponent>(gc).is_some()
                        || world.get_component_by_id_as::<CameraXRComponent>(gc).is_some()
                });
                if wraps_cam {
                    let eye_offset = tc.transform.translation;
                    println!("[AVC] found T-wrapped camera child {:?} — eye_offset = {:?}", ch, eye_offset);
                    return Some((ch, eye_offset));
                }
            }
            None
        })
        .collect();
    if camera_children.is_empty() && camera_bone_id.is_some() {
        println!("[AVC] WARNING: camera_bone set but no Camera3D/CameraXR direct children of AVC found");
    }
    let eye_offset_head_local: [f32; 3] = camera_children
        .iter()
        .map(|&(_, off)| off)
        .find(|off| off != &[0.0, 0.0, 0.0])
        .unwrap_or([0.0, eye_height_from_head_bone.unwrap_or(0.0), 0.0]);

    // Eye offset mapped from head-local into driven_t-local space.
    // This offset is applied to BOTH:
    //   1) model_root baseline translation (moves whole avatar), and
    //   2) spine IK target offset (head pivot relative to HMD target).
    // Using the same baseline offset prevents upper-body crushing from a
    // target-only compensation.
    let head_ik_offset_yaw = if forward_plus_z { 0.0 } else { std::f32::consts::PI };
    let root_eye_neg = [
        -eye_offset_head_local[0],
        -eye_offset_head_local[1],
        -eye_offset_head_local[2],
    ];
    let root_eye_offset_driven_local =
        quat_rotate_vec3(quat_rotation_y(head_ik_offset_yaw), root_eye_neg);

    let model_root_translation: Option<[f32; 3]> = if let Some(h) = avatar_height_override {
        println!("[AVC] using avatar_height_override = {}", h);
        Some([
            root_eye_offset_driven_local[0],
            -h + root_eye_offset_driven_local[1],
            root_eye_offset_driven_local[2],
        ])
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
        Some([
            root_eye_offset_driven_local[0],
            -bone_local_y + root_eye_offset_driven_local[1],
            root_eye_offset_driven_local[2],
        ])
    } else {
        if camera_bone_name.is_some() {
            println!("[AVC] camera_bone not found and no avatar_height_override — model_root.y unchanged");
        }
        None
    };

    // model_root baseline calibration plus authored eye offset compensation.
    // This moves the whole avatar relative to the fixed XR camera pose.
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

    // Store runtime IDs (body_pipeline_id stored after pipeline creation below).
    if let Some(c) = world.get_component_by_id_as_mut::<AvatarControlComponent>(id) {
        c.splice_head       = Some(head_splice_id);
        c.displaced_head    = Some(head_bone_id);
        c.splice_camera_bone = camera_bone_id;
        if let Some((_, driver, bone)) = left  { c.splice_left_hand  = Some(driver); c.displaced_left_hand  = Some(bone); }
        if let Some((_, driver, bone)) = right { c.splice_right_hand = Some(driver); c.displaced_right_hand = Some(bone); }
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
        let body_pipeline_id  = world.add_component(TransformForkTRSComponent::new());
        let map_rot_id        = world.add_component(TransformMapRotationComponent::new());
        let yaw_follow_id     = world.add_component(
            QuatYawFollowComponent::new(body_yaw_threshold, body_yaw_rate)
                .with_initial_yaw(initial_body_yaw)
                .with_forward_plus_z_if(forward_plus_z),
        );

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

    // Dedicated fixed head target under driven_t.  Spine FABRIK chases this node,
    // so head position is defined by HMD pose + authored eye offset, and the body
    // solves around it.
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
    // from neck.  Preserve head_bone's rest rotation/scale so face mesh orientation
    // stays correct.  Emitted *after* the reparent attach so the UpdateTransform
    // lands on head_bone in its new parent (splice_head) without fighting the
    // attach intent's matrix recompute.
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
    // Spine FABRIK chain: hips → ... → splice_head.
    //
    // Bends the spine so the FK chain places splice_head at the head pose
    // driver's target.  target_position_offset carries the eye-offset (head
    // pivot vs camera position) so the head bone pivot, not the eye mesh,
    // lands at the HMD.
    //
    // root_tc = hips (IKChain parented under it).  end_effector = splice_head.
    // collect_tc_chain in ik_system walks UP from end → root for a unique path.
    // -----------------------------------------------------------------------
    if let Some(hips_name) = hips_bone_name.as_deref() {
        let hips_sel = format!("#{}", hips_name);
        if let Some(hips_id) = world.find_component(model_root_id, &hips_sel) {
            let spine_ik_id = world.add_component(IKChainComponent::new(
                IKSolver::Fabrik {
                    max_iterations: 8,
                    tolerance: 0.001,
                    target_position_offset: [0.0, 0.0, 0.0],
                },
                head_target_id,
                head_splice_id,
            ));
            let _ = world.set_parent(spine_ik_id, Some(hips_id));
            println!(
                "[AVC] spine FABRIK chain wired: hips={:?} end_effector=splice_head={:?} target=head_target={:?} offset={:?}",
                hips_id, head_splice_id, head_target_id, head_target_offset,
            );
        } else {
            println!("[AVC] hips bone '{}' not found — spine FABRIK disabled", hips_name);
        }
    }

    // -----------------------------------------------------------------------
    // Hand splices and arm IK.
    //
    // For each hand, two modes:
    //
    //   Arm IK mode (BoneMappingSystem resolved upper/lower arm):
    //     - Controller stays under AVC — OpenXRSystem handles world→local correctly.
    //     - Arm bone stays in FK skeleton (not displaced under controller).
    //     - IKChainComponent { TwoBoneIK } placed under upper_arm drives the chain.
    //     - target_id = raw_driver (controller), end_effector_id = hand bone.
    //     - copy_end_rotation: true — wrist rotation copied from controller.
    //
    //   Simple splice mode (arm chain not available):
    //     - Controller re-parented under bone's original parent.
    //     - Hand bone displaced under controller (or smoothing pipeline output).
    //     - Optional QuatTemporalFilter smoothing pipeline on rotation.
    // -----------------------------------------------------------------------
    for (hand, arm_chain) in [(left, arm_left), (right, arm_right)] {
        let Some((bone_parent, raw_driver, bone)) = hand else { continue };

        if let Some(arm) = arm_chain {
            // --- Arm IK mode ---
            // Pole hint: elbow pointing down is a safe neutral for arms at rest.
            // Body-local pole direction (open question; world-space for now).
            let ik_id = world.add_component(IKChainComponent::new(
                IKSolver::TwoBoneIK {
                    pole_direction: [0.0, -1.0, 0.0],
                    copy_end_rotation: true,
                },
                raw_driver, // IK target = controller driven_t world position
                arm.hand,   // end effector = hand bone (stays in FK skeleton)
            ));
            let _ = world.set_parent(ik_id, Some(arm.upper_arm));
        } else {
            // --- Simple splice mode ---
            let splice_root = world
                .parent_of(raw_driver)
                .filter(|&p| p != bone_parent)
                .unwrap_or(raw_driver);
            emit_attach(emit, bone_parent, splice_root);

            if let Some(smoothing_factor) = hand_rotation_smoothing {
                // Create smoothing pipeline under raw_driver.
                let hfork_id   = world.add_component(TransformForkTRSComponent::new());
                let hmrot_id   = world.add_component(TransformMapRotationComponent::new());
                let hfilt_id   = world.add_component(
                    QuatTemporalFilterComponent::new().with_smoothing_factor(smoothing_factor),
                );
                let smoothed_t = world.add_component(TransformComponent::new());

                let _ = world.set_parent(hmrot_id, Some(hfork_id));
                let _ = world.set_parent(hfilt_id, Some(hmrot_id));
                let _ = world.set_parent(smoothed_t, Some(hfork_id));

                emit_attach(emit, raw_driver, hfork_id);
                emit_attach(emit, smoothed_t, bone);
            } else {
                emit_attach(emit, raw_driver, bone);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Camera re-parenting: move discovered Camera3D/CameraXR children of AVC
    // under the camera bone so they inherit its world transform each tick.
    // -----------------------------------------------------------------------
    if let Some(cam_bone_id) = camera_bone_id {
        for &(cam, _eye_offset) in &camera_children {
            println!("[AVC] re-parenting camera {:?} under camera_bone {:?}", cam, cam_bone_id);
            emit_attach(emit, cam_bone_id, cam);
        }
    } else if !camera_children.is_empty() {
        println!("[AVC] WARNING: camera children found but camera_bone not resolved — no re-parenting");
    }
}

/// Find a hand bone by name and determine its raw driver node.
///
/// Returns `(bone_original_parent, raw_driver, bone_id)` or `None` if the bone
/// wasn't found (model may not have this joint — silently skip).
fn resolve_hand_splice(
    world: &mut World,
    model_root: ComponentId,
    bone_name: Option<&str>,
    controller: Option<ComponentId>,
) -> Option<(ComponentId, ComponentId, ComponentId)> {
    let bone_name = bone_name?;
    let sel = format!("#{}", bone_name);
    let bone = world.find_component(model_root, &sel)?;
    let bone_parent = world.parent_of(bone)?;

    let driver = if let Some(ctrl) = controller {
        world
            .children_of(ctrl)
            .iter()
            .copied()
            .find(|&ch| world.get_component_by_id_as::<TransformComponent>(ch).is_some())
            .unwrap_or_else(|| world.add_component(TransformComponent::new()))
    } else {
        world.add_component(TransformComponent::new())
    };

    Some((bone_parent, driver, bone))
}

fn emit_attach(emit: &mut dyn SignalEmitter, parent: ComponentId, child: ComponentId) {
    emit.push_intent_now(parent, IntentValue::Attach { parents: vec![parent], child });
}

