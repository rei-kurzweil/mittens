use crate::engine::ecs::component::{IKChainComponent, IKSolver, TransformComponent};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};

#[derive(Debug, Default)]
pub struct IKSystem;

impl IKSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn tick(&mut self, world: &mut World, emit: &mut dyn SignalEmitter, _dt_sec: f32) {
        let ids: Vec<ComponentId> = world
            .all_components()
            .filter(|&id| world.get_component_by_id_as::<IKChainComponent>(id).is_some())
            .collect();
        for id in ids {
            tick_chain(id, world, emit);
        }
    }
}

fn tick_chain(id: ComponentId, world: &World, emit: &mut dyn SignalEmitter) {
    let (solver, target_id, end_effector_id, weight) = {
        let Some(c) = world.get_component_by_id_as::<IKChainComponent>(id) else { return };
        (c.solver, c.target_id, c.end_effector_id, c.weight)
    };
    if weight <= 0.0 {
        return;
    }

    // Root joint TC = parent of IKChainComponent.
    let Some(root_tc) = world
        .parent_of(id)
        .filter(|&p| world.get_component_by_id_as::<TransformComponent>(p).is_some())
    else {
        return;
    };

    match solver {
        IKSolver::AimConstraint { offset_yaw } => {
            solve_aim(world, emit, root_tc, target_id, offset_yaw, weight);
        }
        IKSolver::TwoBoneIK { pole_direction, copy_end_rotation } => {
            // Build the 3-node chain directly: [root, first-TC-child-of-root, end_effector_id].
            // This avoids topology walk issues when non-TC nodes (controllers, helpers) sit
            // between the lower-arm and the hand bone after a splice.
            let mid_tc = world
                .children_of(root_tc)
                .iter()
                .copied()
                .find(|&ch| world.get_component_by_id_as::<TransformComponent>(ch).is_some());
            let Some(mid_tc) = mid_tc else { return };
            let chain = [root_tc, mid_tc, end_effector_id];
            solve_two_bone(world, emit, &chain, target_id, pole_direction, copy_end_rotation, weight);
        }
        IKSolver::Fabrik { max_iterations, tolerance } => {
            let chain = collect_tc_chain(world, root_tc, end_effector_id);
            if chain.len() < 2 {
                return;
            }
            solve_fabrik(world, emit, &chain, target_id, max_iterations, tolerance, weight);
        }
    }
}

// ---------------------------------------------------------------------------
// Chain collection
// ---------------------------------------------------------------------------

/// Walk the TC hierarchy from `root` down the first-TC-child path until `end_id` is reached.
/// Returns the collected IDs in root-to-end order.
fn collect_tc_chain(world: &World, root: ComponentId, end_id: ComponentId) -> Vec<ComponentId> {
    let mut chain = vec![root];
    if root == end_id {
        return chain;
    }
    let mut cur = root;
    for _ in 0..32 {
        let next = world
            .children_of(cur)
            .iter()
            .copied()
            .find(|&ch| world.get_component_by_id_as::<TransformComponent>(ch).is_some());
        let Some(next) = next else { break };
        chain.push(next);
        if next == end_id {
            break;
        }
        cur = next;
    }
    chain
}

// ---------------------------------------------------------------------------
// AimConstraint solver
// ---------------------------------------------------------------------------

/// Orient `root_tc` so its world rotation matches `target_id`'s world rotation
/// post-multiplied by `rot_y(offset_yaw)`.  Only modifies rotation; preserves
/// existing local translation and scale.
fn solve_aim(
    world: &World,
    emit: &mut dyn SignalEmitter,
    root_tc: ComponentId,
    target_id: ComponentId,
    offset_yaw: f32,
    weight: f32,
) {
    let Some(target_tc) = world.get_component_by_id_as::<TransformComponent>(target_id) else {
        return;
    };
    let target_world_rot = mat_to_quat(target_tc.transform.matrix_world);
    let desired_world_rot = quat_mul(target_world_rot, quat_rotation_y(offset_yaw));

    let parent_world_rot = world
        .parent_of(root_tc)
        .and_then(|p| world.get_component_by_id_as::<TransformComponent>(p))
        .map(|t| mat_to_quat(t.transform.matrix_world))
        .unwrap_or([0.0, 0.0, 0.0, 1.0]);

    let full_local_rot = quat_mul(quat_inverse(parent_world_rot), desired_world_rot);

    let local_rot = if weight < 1.0 {
        let cur = world
            .get_component_by_id_as::<TransformComponent>(root_tc)
            .map(|t| t.transform.rotation)
            .unwrap_or([0.0, 0.0, 0.0, 1.0]);
        quat_nlerp(cur, full_local_rot, weight)
    } else {
        full_local_rot
    };

    let (t, s) = world
        .get_component_by_id_as::<TransformComponent>(root_tc)
        .map(|tc| (tc.transform.translation, tc.transform.scale))
        .unwrap_or(([0.0; 3], [1.0, 1.0, 1.0]));

    emit.push_intent_now(
        root_tc,
        IntentValue::UpdateTransform {
            component_ids: vec![root_tc],
            translation: t,
            rotation_quat_xyzw: local_rot,
            scale: s,
        },
    );
}

// ---------------------------------------------------------------------------
// TwoBoneIK solver
// ---------------------------------------------------------------------------

/// Closed-form 2-bone IK.
///
/// `chain` must have length ≥ 3: [root (upper arm), mid (lower arm), end (hand)].
/// `target_id`: TC providing the desired hand world position.
/// `pole_direction`: world-space elbow hint.
/// `copy_end_rotation`: if true, also aligns the end bone to the target's world rotation.
fn solve_two_bone(
    world: &World,
    emit: &mut dyn SignalEmitter,
    chain: &[ComponentId],
    target_id: ComponentId,
    pole_direction: [f32; 3],
    copy_end_rotation: bool,
    weight: f32,
) {
    let (root_tc, mid_tc, end_tc) = (chain[0], chain[1], chain[2]);

    // FK world positions — bone lengths are measured here each tick.
    let root_pos = tc_world_pos(world, root_tc);
    let mid_pos  = tc_world_pos(world, mid_tc);
    let end_pos  = tc_world_pos(world, end_tc);
    let target_pos = tc_world_pos(world, target_id);

    let root_world_rot = tc_world_rot(world, root_tc);
    let mid_world_rot  = tc_world_rot(world, mid_tc);

    let upper_len = vec3_len(vec3_sub(mid_pos, root_pos)).max(1e-6);
    let lower_len = vec3_len(vec3_sub(end_pos, mid_pos)).max(1e-6);

    let root_parent_world_rot = world
        .parent_of(root_tc)
        .and_then(|p| world.get_component_by_id_as::<TransformComponent>(p))
        .map(|t| mat_to_quat(t.transform.matrix_world))
        .unwrap_or([0.0, 0.0, 0.0, 1.0]);

    // Triangle solve — clamp reach to avoid degenerate case beyond full extension.
    let to_target = vec3_sub(target_pos, root_pos);
    let raw_reach = vec3_len(to_target);
    let reach = raw_reach.min(upper_len + lower_len - 1e-4).max(1e-6);
    let reach_dir = if raw_reach > 1e-6 {
        vec3_scale(to_target, 1.0 / raw_reach)
    } else {
        [0.0, 1.0, 0.0]
    };

    let cos_upper = ((upper_len * upper_len + reach * reach - lower_len * lower_len)
        / (2.0 * upper_len * reach))
        .clamp(-1.0, 1.0);
    let upper_angle = cos_upper.acos();

    // Build elbow plane from pole hint.
    let cross_tp = vec3_cross(to_target, pole_direction);
    let plane_normal = if vec3_len(cross_tp) > 1e-6 {
        vec3_normalize(cross_tp)
    } else {
        let fallback = if reach_dir[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        vec3_normalize(vec3_cross(to_target, fallback))
    };
    let perp = vec3_normalize(vec3_cross(plane_normal, reach_dir));

    let elbow_dir = vec3_normalize(vec3_add(
        vec3_scale(reach_dir, upper_angle.cos()),
        vec3_scale(perp, upper_angle.sin()),
    ));
    let elbow_pos = vec3_add(root_pos, vec3_scale(elbow_dir, upper_len));

    // --- Upper arm ---
    let old_upper_fwd = if vec3_len(vec3_sub(mid_pos, root_pos)) > 1e-6 {
        vec3_normalize(vec3_sub(mid_pos, root_pos))
    } else {
        [0.0, 0.0, 1.0]
    };
    let delta_upper = shortest_arc_quat(old_upper_fwd, elbow_dir);
    let new_upper_world_rot = quat_mul(delta_upper, root_world_rot);

    let full_upper_local = quat_mul(quat_inverse(root_parent_world_rot), new_upper_world_rot);
    let upper_local = if weight < 1.0 {
        let cur = world
            .get_component_by_id_as::<TransformComponent>(root_tc)
            .map(|t| t.transform.rotation)
            .unwrap_or([0.0, 0.0, 0.0, 1.0]);
        quat_nlerp(cur, full_upper_local, weight)
    } else {
        full_upper_local
    };

    // --- Lower arm ---
    let old_lower_fwd = if vec3_len(vec3_sub(end_pos, mid_pos)) > 1e-6 {
        vec3_normalize(vec3_sub(end_pos, mid_pos))
    } else {
        [0.0, 0.0, 1.0]
    };
    // After the upper arm delta, the lower arm's FK forward rotates by delta_upper too.
    let lower_fwd_after_upper = rotate_vec_by_quat(old_lower_fwd, delta_upper);
    let new_lower_fwd = if vec3_len(vec3_sub(target_pos, elbow_pos)) > 1e-6 {
        vec3_normalize(vec3_sub(target_pos, elbow_pos))
    } else {
        lower_fwd_after_upper
    };
    let delta_lower = shortest_arc_quat(lower_fwd_after_upper, new_lower_fwd);
    let new_lower_world_rot = quat_mul(delta_lower, quat_mul(delta_upper, mid_world_rot));

    // Parent of lower arm is now upper arm with new_upper_world_rot.
    let full_lower_local = quat_mul(quat_inverse(new_upper_world_rot), new_lower_world_rot);
    let lower_local = if weight < 1.0 {
        let cur = world
            .get_component_by_id_as::<TransformComponent>(mid_tc)
            .map(|t| t.transform.rotation)
            .unwrap_or([0.0, 0.0, 0.0, 1.0]);
        quat_nlerp(cur, full_lower_local, weight)
    } else {
        full_lower_local
    };

    // Emit UpdateTransform for upper arm (preserve existing translation/scale).
    let (rt, rs) = world
        .get_component_by_id_as::<TransformComponent>(root_tc)
        .map(|t| (t.transform.translation, t.transform.scale))
        .unwrap_or(([0.0; 3], [1.0, 1.0, 1.0]));
    emit.push_intent_now(
        root_tc,
        IntentValue::UpdateTransform {
            component_ids: vec![root_tc],
            translation: rt,
            rotation_quat_xyzw: upper_local,
            scale: rs,
        },
    );

    // Emit UpdateTransform for lower arm.
    let (mt, ms) = world
        .get_component_by_id_as::<TransformComponent>(mid_tc)
        .map(|t| (t.transform.translation, t.transform.scale))
        .unwrap_or(([0.0; 3], [1.0, 1.0, 1.0]));
    emit.push_intent_now(
        mid_tc,
        IntentValue::UpdateTransform {
            component_ids: vec![mid_tc],
            translation: mt,
            rotation_quat_xyzw: lower_local,
            scale: ms,
        },
    );

    // Optionally copy target rotation to end-effector bone.
    if copy_end_rotation {
        let target_world_rot = tc_world_rot(world, target_id);
        let full_end_local = quat_mul(quat_inverse(new_lower_world_rot), target_world_rot);
        let end_local = if weight < 1.0 {
            let cur = world
                .get_component_by_id_as::<TransformComponent>(end_tc)
                .map(|t| t.transform.rotation)
                .unwrap_or([0.0, 0.0, 0.0, 1.0]);
            quat_nlerp(cur, full_end_local, weight)
        } else {
            full_end_local
        };
        let (et, es) = world
            .get_component_by_id_as::<TransformComponent>(end_tc)
            .map(|t| (t.transform.translation, t.transform.scale))
            .unwrap_or(([0.0; 3], [1.0, 1.0, 1.0]));
        emit.push_intent_now(
            end_tc,
            IntentValue::UpdateTransform {
                component_ids: vec![end_tc],
                translation: et,
                rotation_quat_xyzw: end_local,
                scale: es,
            },
        );
    }
}

// ---------------------------------------------------------------------------
// FABRIK solver
// ---------------------------------------------------------------------------

fn solve_fabrik(
    world: &World,
    emit: &mut dyn SignalEmitter,
    chain: &[ComponentId],
    target_id: ComponentId,
    max_iterations: u32,
    tolerance: f32,
    weight: f32,
) {
    let n = chain.len();

    let mut positions: Vec<[f32; 3]> = chain.iter().map(|&tc| tc_world_pos(world, tc)).collect();
    let bone_lengths: Vec<f32> = (0..n - 1)
        .map(|i| vec3_len(vec3_sub(positions[i + 1], positions[i])).max(1e-6))
        .collect();
    let root_pos = positions[0];
    let target_pos = tc_world_pos(world, target_id);

    // FABRIK iterations.
    for _ in 0..max_iterations {
        if vec3_len(vec3_sub(*positions.last().unwrap(), target_pos)) < tolerance {
            break;
        }
        // Forward pass — pull end to target.
        *positions.last_mut().unwrap() = target_pos;
        for i in (0..n - 1).rev() {
            let d = vec3_len(vec3_sub(positions[i], positions[i + 1]));
            let t = if d > 1e-9 { bone_lengths[i] / d } else { 0.0 };
            positions[i] = vec3_lerp(positions[i + 1], positions[i], t);
        }
        // Backward pass — pin root.
        positions[0] = root_pos;
        for i in 0..n - 1 {
            let d = vec3_len(vec3_sub(positions[i + 1], positions[i]));
            let t = if d > 1e-9 { bone_lengths[i] / d } else { 0.0 };
            positions[i + 1] = vec3_lerp(positions[i], positions[i + 1], t);
        }
    }

    // Convert solved bone directions to local rotations and emit.
    let mut parent_world_rot = world
        .parent_of(chain[0])
        .and_then(|p| world.get_component_by_id_as::<TransformComponent>(p))
        .map(|t| mat_to_quat(t.transform.matrix_world))
        .unwrap_or([0.0, 0.0, 0.0, 1.0]);

    for i in 0..n - 1 {
        let tc = chain[i];
        let cur_world_rot = tc_world_rot(world, tc);

        let cur_fwd = {
            let from = tc_world_pos(world, tc);
            let to = tc_world_pos(world, chain[i + 1]);
            let d = vec3_sub(to, from);
            if vec3_len(d) > 1e-6 { vec3_normalize(d) } else { [0.0, 0.0, 1.0] }
        };
        let desired_fwd = {
            let d = vec3_sub(positions[i + 1], positions[i]);
            if vec3_len(d) > 1e-6 { vec3_normalize(d) } else { cur_fwd }
        };

        let delta = shortest_arc_quat(cur_fwd, desired_fwd);
        let new_world_rot = quat_mul(delta, cur_world_rot);
        let full_local = quat_mul(quat_inverse(parent_world_rot), new_world_rot);

        let local_rot = if weight < 1.0 {
            let cur = world
                .get_component_by_id_as::<TransformComponent>(tc)
                .map(|t| t.transform.rotation)
                .unwrap_or([0.0, 0.0, 0.0, 1.0]);
            quat_nlerp(cur, full_local, weight)
        } else {
            full_local
        };

        let (t, s) = world
            .get_component_by_id_as::<TransformComponent>(tc)
            .map(|tc| (tc.transform.translation, tc.transform.scale))
            .unwrap_or(([0.0; 3], [1.0, 1.0, 1.0]));
        emit.push_intent_now(
            tc,
            IntentValue::UpdateTransform {
                component_ids: vec![tc],
                translation: t,
                rotation_quat_xyzw: local_rot,
                scale: s,
            },
        );

        parent_world_rot = new_world_rot;
    }
}

// ---------------------------------------------------------------------------
// World-matrix helpers
// ---------------------------------------------------------------------------

fn tc_world_pos(world: &World, id: ComponentId) -> [f32; 3] {
    world
        .get_component_by_id_as::<TransformComponent>(id)
        .map(|t| {
            let m = t.transform.matrix_world;
            [m[3][0], m[3][1], m[3][2]]
        })
        .unwrap_or([0.0; 3])
}

fn tc_world_rot(world: &World, id: ComponentId) -> [f32; 4] {
    world
        .get_component_by_id_as::<TransformComponent>(id)
        .map(|t| mat_to_quat(t.transform.matrix_world))
        .unwrap_or([0.0, 0.0, 0.0, 1.0])
}

// ---------------------------------------------------------------------------
// Math helpers
// ---------------------------------------------------------------------------

fn mat_to_quat(m: [[f32; 4]; 4]) -> [f32; 4] {
    fn col_len(m: [[f32; 4]; 4], c: usize) -> f32 {
        (m[c][0] * m[c][0] + m[c][1] * m[c][1] + m[c][2] * m[c][2])
            .sqrt()
            .max(1e-9)
    }
    let s0 = col_len(m, 0).recip();
    let s1 = col_len(m, 1).recip();
    let s2 = col_len(m, 2).recip();
    let r00 = m[0][0] * s0; let r10 = m[0][1] * s0; let r20 = m[0][2] * s0;
    let r01 = m[1][0] * s1; let r11 = m[1][1] * s1; let r21 = m[1][2] * s1;
    let r02 = m[2][0] * s2; let r12 = m[2][1] * s2; let r22 = m[2][2] * s2;
    let trace = r00 + r11 + r22;
    if trace > 0.0 {
        let s = 0.5 / (trace + 1.0).sqrt();
        normalise_quat([(r21 - r12) * s, (r02 - r20) * s, (r10 - r01) * s, 0.25 / s])
    } else if r00 > r11 && r00 > r22 {
        let s = 2.0 * (1.0 + r00 - r11 - r22).sqrt();
        normalise_quat([0.25 * s, (r01 + r10) / s, (r02 + r20) / s, (r21 - r12) / s])
    } else if r11 > r22 {
        let s = 2.0 * (1.0 + r11 - r00 - r22).sqrt();
        normalise_quat([(r01 + r10) / s, 0.25 * s, (r12 + r21) / s, (r02 - r20) / s])
    } else {
        let s = 2.0 * (1.0 + r22 - r00 - r11).sqrt();
        normalise_quat([(r02 + r20) / s, (r12 + r21) / s, 0.25 * s, (r10 - r01) / s])
    }
}

fn normalise_quat(q: [f32; 4]) -> [f32; 4] {
    let len2 = q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3];
    if len2 < 1e-12 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let inv = len2.sqrt().recip();
    [q[0] * inv, q[1] * inv, q[2] * inv, q[3] * inv]
}

fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let (ax, ay, az, aw) = (a[0], a[1], a[2], a[3]);
    let (bx, by, bz, bw) = (b[0], b[1], b[2], b[3]);
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

fn quat_inverse(q: [f32; 4]) -> [f32; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

fn quat_rotation_y(yaw: f32) -> [f32; 4] {
    let half = yaw * 0.5;
    [0.0, half.sin(), 0.0, half.cos()]
}

/// Normalised linear interpolation — fast approximate slerp.
fn quat_nlerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    let b = if dot < 0.0 { [-b[0], -b[1], -b[2], -b[3]] } else { b };
    normalise_quat([
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ])
}

/// Minimum-arc quaternion that rotates unit vector `from` to unit vector `to`.
fn shortest_arc_quat(from: [f32; 3], to: [f32; 3]) -> [f32; 4] {
    let d = vec3_dot(from, to);
    if d < -0.9999 {
        // Anti-parallel: 180° rotation around an arbitrary perpendicular.
        let perp = if from[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let axis = vec3_normalize(vec3_cross(from, perp));
        return [axis[0], axis[1], axis[2], 0.0];
    }
    let c = vec3_cross(from, to);
    normalise_quat([c[0], c[1], c[2], 1.0 + d])
}

/// Rotate a 3-vector by a unit quaternion: v' = q * v * q⁻¹.
fn rotate_vec_by_quat(v: [f32; 3], q: [f32; 4]) -> [f32; 3] {
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

fn vec3_sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn vec3_add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn vec3_scale(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}
fn vec3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn vec3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn vec3_len(v: [f32; 3]) -> f32 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}
fn vec3_normalize(v: [f32; 3]) -> [f32; 3] {
    let l = vec3_len(v).max(1e-9);
    vec3_scale(v, 1.0 / l)
}
fn vec3_lerp(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    vec3_add(a, vec3_scale(vec3_sub(b, a), t))
}
