use crate::engine::ecs::component::{
    AvatarControlComponent, BoneRestPoseComponent, Camera3DComponent, CameraXRComponent,
    CollisionComponent, CollisionResponseComponent, CollisionShapeComponent, ControllerHand,
    ControllerXRComponent, GLTFComponent, IKChainComponent, IKSolver, InputXRComponent,
    QuatYawFollowComponent, SerializeComponent, TransformComponent, TransformDropComponent,
    TransformForkTRSComponent, TransformMapRotationComponent, TransformMapScaleComponent,
    TransformMapTranslationComponent,
};
use crate::engine::ecs::system::bounds_system::BoundsSystem;
use crate::engine::ecs::system::collision_shape_inference::infer_upright_capsule;
use crate::engine::ecs::system::input_xr_gamepad_system::xr_locomotion_target_transform;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::RenderAssets;
use crate::engine::user_input::InputState;
use crate::utils::math::{
    mat_to_quat, quat_conjugate, quat_mul, quat_rotate_vec3, quat_rotation_y,
};
use std::collections::HashSet;
use winit::keyboard::{Key, NamedKey};

#[derive(Debug, Default)]
pub struct AvatarControlSystem {
    avatars: HashSet<ComponentId>,
}

impl AvatarControlSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tick(
        &mut self,
        world: &mut World,
        input: &InputState,
        render_assets: &RenderAssets,
        emit: &mut dyn SignalEmitter,
        dt_sec: f32,
    ) {
        let ids: Vec<_> = self.avatars.iter().copied().collect();

        let mut calibration_consumed = false;
        let calibrate_pressed = input.key_pressed(&Key::Named(NamedKey::Enter));
        for id in ids {
            let allow_calibration = calibrate_pressed && !calibration_consumed;
            if tick_one(id, world, render_assets, emit, dt_sec, allow_calibration) {
                calibration_consumed = true;
            }
        }
    }

    pub fn register(&mut self, component: ComponentId) {
        self.avatars.insert(component);
    }

    pub fn remove(&mut self, component: ComponentId) {
        self.avatars.remove(&component);
    }
}

fn tick_one(
    id: ComponentId,
    world: &mut World,
    render_assets: &RenderAssets,
    emit: &mut dyn SignalEmitter,
    _dt_sec: f32,
    allow_calibration: bool,
) -> bool {
    // --- Init phase ---
    let needs_init = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
            return false;
        };
        c.head_mount.is_none()
    };

    if needs_init {
        // Runtime splicing reparents and rewrites avatar bones, so it is itself a
        // pose-changing operation. An XR-authored avatar must remain in its authored
        // pose until the headset has supplied a valid pose. Non-XR AVC trees continue
        // to initialize immediately.
        if !ancestor_input_xr_is_ready(world, id) {
            return false;
        }
        try_init_splices(id, world, emit);
    }

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
                        component_ids: vec![head_bone_id],
                        translation: [0.0, 0.0, 0.0],
                        rotation_quat_xyzw: head_t.transform.rotation,
                        scale: head_t.transform.scale,
                    },
                );
            }
        }
    }

    if allow_calibration && capture_hand_grip_calibration(id, world, emit) {
        return true;
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

    let inferred =
        BoundsSystem::calculate_subtree_local_bounds(world, render_assets, model_root_id)
            .and_then(|bounds| infer_upright_capsule(&bounds, avc.capsule_radius))
            .or_else(|| {
                spawned_gltf_exists(world, model_root_id)
                    .then(|| fallback_avatar_height(world, avc_id, model_root_id, &avc))
                    .flatten()
                    .and_then(|height| {
                        let bounds = crate::engine::graphics::bounds::Aabb {
                            min: [0.0, 0.0, 0.0],
                            max: [0.0, height.max(0.0), 0.0],
                        };
                        infer_upright_capsule(&bounds, avc.capsule_radius)
                    })
            });
    let Some(inferred) = inferred else { return };

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
            component_ids: vec![collision],
        },
    );
    emit.push_intent_now(
        response,
        IntentValue::RegisterCollisionResponse {
            component_ids: vec![response],
        },
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
    let bone_name = avc.camera_bone.as_deref().unwrap_or(&avc.head_bone);
    let bone = world.find_component(model_root_id, &format!("#{bone_name}"))?;
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
    let head_selector = format!("#{}", head_bone_name);
    let Some(head_bone_id) = world.find_component(model_root_id, &head_selector) else {
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
        let cam_bone_world_y = world
            .get_component_by_id_as::<TransformComponent>(cam_bone_id)
            .map(|t| t.transform.matrix_world[3][1])
            .unwrap_or(0.0);
        let model_root_world_y = world
            .get_component_by_id_as::<TransformComponent>(model_root_id)
            .map(|t| t.transform.matrix_world[3][1])
            .unwrap_or(0.0);
        let bone_local_y = cam_bone_world_y - model_root_world_y;
        Some([0.0, -bone_local_y, 0.0])
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
        if let Some((_, raw_driver, hand_driver, _)) = left {
            c.left_hand_raw_target_id = Some(raw_driver);
            c.left_hand_visual_target_id = Some(hand_driver);
        }
        if let Some((_, raw_driver, hand_driver, _)) = right {
            c.right_hand_raw_target_id = Some(raw_driver);
            c.right_hand_visual_target_id = Some(hand_driver);
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
            "[AVC] WARNING: camera children found but camera_bone not resolved — no re-parenting"
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

fn capture_hand_grip_calibration(
    avc_id: ComponentId,
    world: &mut World,
    emit: &mut dyn SignalEmitter,
) -> bool {
    let (enabled, left_raw, right_raw, left_visual, right_visual, left_hand, right_hand) = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(avc_id) else {
            return false;
        };
        (
            c.calibrate_hand_transforms,
            c.left_hand_raw_target_id,
            c.right_hand_raw_target_id,
            c.left_hand_visual_target_id,
            c.right_hand_visual_target_id,
            c.left_hand_bone_id,
            c.right_hand_bone_id,
        )
    };

    if !enabled {
        return false;
    }

    let (
        Some(left_raw),
        Some(right_raw),
        Some(left_visual),
        Some(right_visual),
        Some(left_hand),
        Some(right_hand),
    ) = (
        left_raw,
        right_raw,
        left_visual,
        right_visual,
        left_hand,
        right_hand,
    )
    else {
        println!(
            "[AVC][calibrate] AVC {:?} is enabled for hand calibration but arm targets are not initialized yet.",
            avc_id
        );
        return true;
    };

    let left_offset = quat_mul(
        quat_conjugate(tc_world_rot(world, left_raw)),
        tc_world_rot(world, left_hand),
    );
    let right_offset = quat_mul(
        quat_conjugate(tc_world_rot(world, right_raw)),
        tc_world_rot(world, right_hand),
    );

    if let Some(c) = world.get_component_by_id_as_mut::<AvatarControlComponent>(avc_id) {
        c.hand_grip_rotation_left = Some(left_offset);
        c.hand_grip_rotation_right = Some(right_offset);
    }

    if left_visual != left_raw {
        update_local_rotation(world, emit, left_visual, left_offset);
    }
    if right_visual != right_raw {
        update_local_rotation(world, emit, right_visual, right_offset);
    }

    println!(
        "[AVC][calibrate] captured hand grip offsets for AVC {:?}:",
        avc_id
    );
    println!(
        "  hand_grip_rotation_left([{:.7}, {:.7}, {:.7}, {:.7}])",
        left_offset[0], left_offset[1], left_offset[2], left_offset[3]
    );
    println!(
        "  hand_grip_rotation_right([{:.7}, {:.7}, {:.7}, {:.7}])",
        right_offset[0], right_offset[1], right_offset[2], right_offset[3]
    );

    true
}

fn update_local_rotation(
    world: &World,
    emit: &mut dyn SignalEmitter,
    component_id: ComponentId,
    rotation: [f32; 4],
) {
    let (translation, scale) = world
        .get_component_by_id_as::<TransformComponent>(component_id)
        .map(|t| (t.transform.translation, t.transform.scale))
        .unwrap_or(([0.0; 3], [1.0, 1.0, 1.0]));
    emit.push_intent_now(
        component_id,
        IntentValue::UpdateTransform {
            component_ids: vec![component_id],
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

fn tc_world_rot(world: &World, id: ComponentId) -> [f32; 4] {
    world
        .get_component_by_id_as::<TransformComponent>(id)
        .map(|t| mat_to_quat(t.transform.matrix_world))
        .unwrap_or([0.0, 0.0, 0.0, 1.0])
}

#[cfg(test)]
mod capsule_tests {
    use super::*;
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::component::{CollisionShape, RenderableComponent};
    use crate::engine::ecs::system::TransformStreamSystem;

    fn attach(world: &mut World, parent: ComponentId, child: ComponentId) {
        world.set_parent(child, Some(parent)).unwrap();
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
        assert_eq!([upright[3][0], upright[3][1], upright[3][2]], [3.0, 4.0, 5.0]);
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
