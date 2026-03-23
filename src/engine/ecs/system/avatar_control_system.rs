use crate::engine::ecs::component::{
    AvatarControlComponent, ControllerHand, ControllerXRComponent, TransformComponent,
};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};

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

fn tick_one(id: ComponentId, world: &mut World, emit: &mut dyn SignalEmitter, dt_sec: f32) {
    // --- Init phase ---
    let needs_init = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
            return;
        };
        c.splice_head.is_none()
    };

    if needs_init {
        try_init_splices(id, world, emit);
        return; // regular tick runs next frame after Attach intents are flushed
    }

    // --- Regular tick ---
    let (threshold, rate, body_yaw, forward_plus_z, splice_head_id, model_root_rest_local) = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
            return;
        };
        let Some(splice_head_id) = c.splice_head else { return };
        (c.body_yaw_threshold, c.body_yaw_rate, c.body_yaw, c.forward_plus_z, splice_head_id, c.model_root_rest_local)
    };

    // driven_t is the parent of AvatarControlComponent.
    let Some(driven_t_id) = world.parent_of(id) else { return };
    let driven_matrix_world = {
        let Some(t) = world.get_component_by_id_as::<TransformComponent>(driven_t_id) else {
            return;
        };
        t.transform.matrix_world
    };
    let driven_world_rot = mat_to_quat(driven_matrix_world);

    // neck_parent is the parent of splice_head (stable after init).
    let Some(neck_parent_id) = world.parent_of(splice_head_id) else { return };
    let neck_parent_world_rot = {
        let Some(t) = world.get_component_by_id_as::<TransformComponent>(neck_parent_id) else {
            return;
        };
        mat_to_quat(t.transform.matrix_world)
    };

    // First TransformComponent child of AvatarControlComponent is model_root.
    let Some(model_root_id) = world
        .children_of(id)
        .iter()
        .copied()
        .find(|&ch| world.get_component_by_id_as::<TransformComponent>(ch).is_some())
    else {
        return;
    };
    let model_root_scale = {
        let Some(t) = world.get_component_by_id_as::<TransformComponent>(model_root_id) else {
            return;
        };
        t.transform.scale
    };

    // Counteract driven_t pitch on model_root's translation: express the rest-pose offset
    // in world space so it is always a straight-down vector regardless of head pitch.
    // local_translation = quat_inverse(driven_world_rot) * rest_world_offset
    let model_root_translation = rotate_vec_by_quat(quat_inverse(driven_world_rot), model_root_rest_local);

    // --- Body rotation: counteract driven_t rotation, apply body_yaw ---
    let body_rot = quat_mul(quat_inverse(driven_world_rot), quat_rotation_y(body_yaw));
    emit.push_intent_now(
        model_root_id,
        IntentValue::UpdateTransform {
            component_ids: vec![model_root_id],
            translation: model_root_translation,
            rotation_quat_xyzw: body_rot,
            scale: model_root_scale,
        },
    );

    // --- Head rotation ---
    // For VR (-Z forward): multiply by quat_rotation_y(π) to bake the VRM/OpenXR handedness flip.
    // For desktop (+Z forward): both input and VRM face +Z — no correction needed.
    let handedness_correction = if forward_plus_z { 0.0 } else { std::f32::consts::PI };
    let head_world_rot = quat_mul(driven_world_rot, quat_rotation_y(handedness_correction));
    let splice_local_rot = quat_mul(quat_inverse(neck_parent_world_rot), head_world_rot);
    emit.push_intent_now(
        splice_head_id,
        IntentValue::UpdateTransform {
            component_ids: vec![splice_head_id],
            translation: [0.0, 0.0, 0.0],
            rotation_quat_xyzw: splice_local_rot,
            scale: [1.0, 1.0, 1.0],
        },
    );

    // --- Body yaw follow ---
    let head_yaw = extract_world_yaw(driven_matrix_world, forward_plus_z);
    let delta = signed_yaw_diff(head_yaw, body_yaw);
    if delta.abs() > threshold {
        let target = head_yaw - delta.signum() * threshold;
        let step = rate * dt_sec;
        let new_body_yaw =
            lerp_angle(body_yaw, target, step.min(delta.abs()) / delta.abs().max(1e-9));

        if (new_body_yaw - body_yaw).abs() >= 1e-6 {
            if let Some(c) = world.get_component_by_id_as_mut::<AvatarControlComponent>(id) {
                c.body_yaw = new_body_yaw;
            }
            let updated_body_rot =
                quat_mul(quat_inverse(driven_world_rot), quat_rotation_y(new_body_yaw));
            emit.push_intent_now(
                model_root_id,
                IntentValue::UpdateTransform {
                    component_ids: vec![model_root_id],
                    translation: model_root_translation,
                    rotation_quat_xyzw: updated_body_rot,
                    scale: model_root_scale,
                },
            );
        }
    }
}

/// First-time setup: splice the head bone and any configured hand bones.
///
/// Controllers are discovered by topology: any `ControllerXRComponent` that is a
/// **direct child** of this `AvatarControlComponent` is treated as a hand driver.
/// Its `hand` field (`Left` / `Right`) determines which hand bone it drives.
/// The bone is displaced under the controller's first `TransformComponent` child.
///
/// If no controller is present for a configured hand bone, a plain
/// `TransformComponent` splice is inserted instead.
fn try_init_splices(id: ComponentId, world: &mut World, emit: &mut dyn SignalEmitter) {
    let (head_bone_name, left_hand_bone, right_hand_bone) = {
        let Some(c) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
            return;
        };
        (
            c.head_bone.clone(),
            c.left_hand_bone.clone(),
            c.right_hand_bone.clone(),
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

    // Discover hand controllers by topology: direct ControllerXRComponent children,
    // matched by ControllerHand field.
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

    // Head bone is required — retry next tick if GLTF hasn't spawned yet.
    let head_selector = format!("[name='{}']", head_bone_name);
    let Some(head_bone_id) = world.find_component(model_root_id, &head_selector) else {
        return;
    };
    let Some(head_parent_id) = world.parent_of(head_bone_id) else { return };
    let head_splice_id = world.add_component(TransformComponent::new());

    // Resolve hand splices.
    // Returns (bone_original_parent, driver_node, bone_id):
    //   driver_node = the node bone is displaced under (controller's driven_t or plain TC).
    let left  = resolve_hand_splice(world, model_root_id, left_hand_bone.as_deref(),  left_ctrl);
    let right = resolve_hand_splice(world, model_root_id, right_hand_bone.as_deref(), right_ctrl);

    // Cache model_root's rest-pose local translation so tick_one can compensate driven_t pitch.
    let model_root_rest_local = world
        .get_component_by_id_as::<TransformComponent>(model_root_id)
        .map(|t| t.transform.translation)
        .unwrap_or([0.0, 0.0, 0.0]);

    // Store runtime IDs before emitting intents.
    if let Some(c) = world.get_component_by_id_as_mut::<AvatarControlComponent>(id) {
        c.model_root_rest_local = model_root_rest_local;
        c.splice_head    = Some(head_splice_id);
        c.displaced_head = Some(head_bone_id);
        if let Some((_, driver, bone)) = left  { c.splice_left_hand  = Some(driver); c.displaced_left_hand  = Some(bone); }
        if let Some((_, driver, bone)) = right { c.splice_right_hand = Some(driver); c.displaced_right_hand = Some(bone); }
    }

    // Head splice: splice_head under neck_parent, head bone under splice_head.
    emit_attach(emit, head_parent_id, head_splice_id);
    emit_attach(emit, head_splice_id, head_bone_id);

    // Hand splices: for each hand, bone_parent → splice_root → driver → bone.
    // If a controller was resolved, splice_root is the controller (parent of driver);
    // otherwise splice_root == driver (the plain TC).
    for hand in [left, right].into_iter().flatten() {
        let (bone_parent, driver, bone) = hand;
        let splice_root = world.parent_of(driver).filter(|&p| p != bone_parent).unwrap_or(driver);
        emit_attach(emit, bone_parent, splice_root);
        emit_attach(emit, driver, bone);
    }
}

/// Find a hand bone by name and determine its driver node.
///
/// Returns `(bone_original_parent, driver_node, bone_id)` or `None` if the bone
/// wasn't found (model may not have this joint — silently skip).
///
/// `driver_node` is:
/// - The controller's first `TransformComponent` child (driven_t), if `controller` is `Some`.
/// - A freshly created plain `TransformComponent`, if `controller` is `None`.
fn resolve_hand_splice(
    world: &mut World,
    model_root: ComponentId,
    bone_name: Option<&str>,
    controller: Option<ComponentId>,
) -> Option<(ComponentId, ComponentId, ComponentId)> {
    let bone_name = bone_name?;
    let sel = format!("[name='{}']", bone_name);
    let bone = world.find_component(model_root, &sel)?;
    let bone_parent = world.parent_of(bone)?;

    let driver = if let Some(ctrl) = controller {
        // Use the controller's first TC child (driven_t) as the driver.
        // The example must have pre-attached driven_t to the controller.
        world
            .children_of(ctrl)
            .iter()
            .copied()
            .find(|&ch| world.get_component_by_id_as::<TransformComponent>(ch).is_some())
            .unwrap_or_else(|| {
                // Fallback: create a plain TC if the controller somehow has no TC child yet.
                world.add_component(TransformComponent::new())
            })
    } else {
        world.add_component(TransformComponent::new())
    };

    Some((bone_parent, driver, bone))
}

fn emit_attach(emit: &mut dyn SignalEmitter, parent: ComponentId, child: ComponentId) {
    emit.push_intent_now(parent, IntentValue::Attach { parents: vec![parent], child });
}

// ---------------------------------------------------------------------------
// Math helpers
// ---------------------------------------------------------------------------

fn extract_world_yaw(m: [[f32; 4]; 4], plus_z_forward: bool) -> f32 {
    if plus_z_forward { m[2][0].atan2(m[2][2]) }
    else              { (-m[2][0]).atan2(-m[2][2]) }
}

fn mat_to_quat(m: [[f32; 4]; 4]) -> [f32; 4] {
    fn col_len(m: [[f32; 4]; 4], c: usize) -> f32 {
        (m[c][0] * m[c][0] + m[c][1] * m[c][1] + m[c][2] * m[c][2]).sqrt().max(1e-9)
    }
    let s0 = col_len(m, 0).recip();
    let s1 = col_len(m, 1).recip();
    let s2 = col_len(m, 2).recip();

    let r00 = m[0][0]*s0; let r10 = m[0][1]*s0; let r20 = m[0][2]*s0;
    let r01 = m[1][0]*s1; let r11 = m[1][1]*s1; let r21 = m[1][2]*s1;
    let r02 = m[2][0]*s2; let r12 = m[2][1]*s2; let r22 = m[2][2]*s2;

    let trace = r00 + r11 + r22;
    if trace > 0.0 {
        let s = 0.5 / (trace + 1.0).sqrt();
        normalise_quat([(r21-r12)*s, (r02-r20)*s, (r10-r01)*s, 0.25/s])
    } else if r00 > r11 && r00 > r22 {
        let s = 2.0 * (1.0 + r00 - r11 - r22).sqrt();
        normalise_quat([0.25*s, (r01+r10)/s, (r02+r20)/s, (r21-r12)/s])
    } else if r11 > r22 {
        let s = 2.0 * (1.0 + r11 - r00 - r22).sqrt();
        normalise_quat([(r01+r10)/s, 0.25*s, (r12+r21)/s, (r02-r20)/s])
    } else {
        let s = 2.0 * (1.0 + r22 - r00 - r11).sqrt();
        normalise_quat([(r02+r20)/s, (r12+r21)/s, 0.25*s, (r10-r01)/s])
    }
}

fn normalise_quat(q: [f32; 4]) -> [f32; 4] {
    let len2 = q[0]*q[0] + q[1]*q[1] + q[2]*q[2] + q[3]*q[3];
    if len2 < 1e-12 { return [0.0, 0.0, 0.0, 1.0]; }
    let inv = len2.sqrt().recip();
    [q[0]*inv, q[1]*inv, q[2]*inv, q[3]*inv]
}

fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let (ax, ay, az, aw) = (a[0], a[1], a[2], a[3]);
    let (bx, by, bz, bw) = (b[0], b[1], b[2], b[3]);
    [
        aw*bx + ax*bw + ay*bz - az*by,
        aw*by - ax*bz + ay*bw + az*bx,
        aw*bz + ax*by - ay*bx + az*bw,
        aw*bw - ax*bx - ay*by - az*bz,
    ]
}

fn quat_inverse(q: [f32; 4]) -> [f32; 4] { [-q[0], -q[1], -q[2], q[3]] }

/// Rotate a 3-vector by a unit quaternion: v' = q * (0,v) * q^-1.
fn rotate_vec_by_quat(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    // Using the sandwich product shortcut: t = 2 * cross(q.xyz, v); v' = v + q.w*t + cross(q.xyz, t)
    let (qx, qy, qz, qw) = (q[0], q[1], q[2], q[3]);
    let (vx, vy, vz) = (v[0], v[1], v[2]);
    let tx = 2.0 * (qy * vz - qz * vy);
    let ty = 2.0 * (qz * vx - qx * vz);
    let tz = 2.0 * (qx * vy - qy * vx);
    [
        vx + qw * tx + qy * tz - qz * ty,
        vy + qw * ty + qz * tx - qx * tz,
        vz + qw * tz + qx * ty - qy * tx,
    ]
}

fn quat_rotation_y(yaw: f32) -> [f32; 4] {
    let half = yaw * 0.5;
    [0.0, half.sin(), 0.0, half.cos()]
}

fn signed_yaw_diff(a: f32, b: f32) -> f32 { wrap_angle(a - b) }

fn wrap_angle(a: f32) -> f32 {
    let mut v = a % (2.0 * std::f32::consts::PI);
    if v >  std::f32::consts::PI { v -= 2.0 * std::f32::consts::PI; }
    if v < -std::f32::consts::PI { v += 2.0 * std::f32::consts::PI; }
    v
}

fn lerp_angle(from: f32, to: f32, t: f32) -> f32 {
    from + signed_yaw_diff(to, from) * t.clamp(0.0, 1.0)
}
