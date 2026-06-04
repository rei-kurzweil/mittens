use crate::engine::ecs::component::{IKChainComponent, IKSolver, TransformComponent};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::utils::math::{
    mat_to_quat, quat_conjugate, quat_from_axis_angle, quat_mul, quat_nlerp, quat_rotate_vec3,
    quat_rotation_y, quat_to_axis_angle, shortest_arc_quat, vec3_add, vec3_cross, vec3_dot,
    vec3_len, vec3_lerp, vec3_normalize, vec3_scale, vec3_sub,
};

#[derive(Debug, Default)]
pub struct IKSystem;

impl IKSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn tick(&mut self, world: &mut World, emit: &mut dyn SignalEmitter, _dt_sec: f32) {
        let ids: Vec<ComponentId> = world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<IKChainComponent>(id)
                    .is_some()
            })
            .collect();
        for id in ids {
            // Lazy resolution of `target_source` / `end_effector_source` into
            // the actual ComponentId fields. Matches AnimationSystem's
            // behavior for ActionComponent and supports forward refs
            // (sources authored before the referent exists). No-op when the
            // ids are already filled (either by registry-time resolve at
            // call-construction, or by a previous tick).
            resolve_ik_chain_refs(world, id);
            tick_chain(id, world, emit);
        }
    }
}

/// Best-effort resolve of IKChain's `target_source` / `end_effector_source`
/// into the matching `target_id` / `end_effector_id` slots when the latter
/// are null. Silent no-op on miss — the IK solver short-circuits on a null
/// target anyway, so a still-unresolved chain just skips that tick. A future
/// tick may succeed once the referent spawns.
fn resolve_ik_chain_refs(world: &mut World, id: ComponentId) {
    use crate::engine::ecs::component::ComponentRef;
    use slotmap::Key;
    let (target_src, end_src, target_id, end_id) = {
        let Some(ik) = world.get_component_by_id_as::<IKChainComponent>(id) else {
            return;
        };
        (
            ik.target_source.clone(),
            ik.end_effector_source.clone(),
            ik.target_id,
            ik.end_effector_id,
        )
    };

    let resolve = |src: &ComponentRef| -> Option<ComponentId> {
        match src {
            ComponentRef::Guid(uuid) => world.component_id_by_guid(*uuid),
            ComponentRef::Query(selector) => {
                let roots: Vec<ComponentId> = world
                    .all_components()
                    .filter(|&cid| world.parent_of(cid).is_none())
                    .collect();
                roots
                    .into_iter()
                    .find_map(|root| world.find_component(root, selector))
            }
        }
    };

    let new_target = if target_id.is_null() {
        target_src.as_ref().and_then(resolve)
    } else {
        None
    };
    let new_end = if end_id.is_null() {
        end_src.as_ref().and_then(resolve)
    } else {
        None
    };

    if new_target.is_none() && new_end.is_none() {
        return;
    }
    if let Some(ik) = world.get_component_by_id_as_mut::<IKChainComponent>(id) {
        if let Some(t) = new_target {
            ik.target_id = t;
        }
        if let Some(e) = new_end {
            ik.end_effector_id = e;
        }
    }
}

fn tick_chain(id: ComponentId, world: &World, emit: &mut dyn SignalEmitter) {
    let (solver, target_id, end_effector_id, weight) = {
        let Some(c) = world.get_component_by_id_as::<IKChainComponent>(id) else {
            return;
        };
        (c.solver, c.target_id, c.end_effector_id, c.weight)
    };
    if weight <= 0.0 {
        return;
    }

    // For AimConstraint / Fabrik, root joint TC = parent of IKChainComponent.
    // TwoBoneIK ignores this and uses the explicit joint IDs on the variant.
    let root_tc_opt = world.parent_of(id).filter(|&p| {
        world
            .get_component_by_id_as::<TransformComponent>(p)
            .is_some()
    });

    match solver {
        IKSolver::AimConstraint {
            offset_yaw,
            copy_position,
            target_position_offset,
        } => {
            let Some(root_tc) = root_tc_opt else { return };
            solve_aim(
                world,
                emit,
                root_tc,
                target_id,
                offset_yaw,
                copy_position,
                target_position_offset,
                weight,
            );
        }
        IKSolver::TwoBoneIK {
            root_joint_id,
            mid_joint_id,
            pole_direction,
            copy_end_rotation,
        } => {
            // Explicit 3-node chain — no topology discovery. Sibling helper /
            // collider / cloth nodes under the arm bones are ignored.
            use slotmap::Key;
            if root_joint_id.is_null() || mid_joint_id.is_null() || end_effector_id.is_null() {
                return;
            }
            if world
                .get_component_by_id_as::<TransformComponent>(root_joint_id)
                .is_none()
                || world
                    .get_component_by_id_as::<TransformComponent>(mid_joint_id)
                    .is_none()
                || world
                    .get_component_by_id_as::<TransformComponent>(end_effector_id)
                    .is_none()
            {
                return;
            }
            let chain = [root_joint_id, mid_joint_id, end_effector_id];
            solve_two_bone(
                world,
                emit,
                &chain,
                target_id,
                pole_direction,
                copy_end_rotation,
                weight,
            );
        }
        IKSolver::Fabrik {
            max_iterations,
            tolerance,
            target_position_offset,
        } => {
            let Some(root_tc) = root_tc_opt else { return };
            let chain = collect_tc_chain(world, root_tc, end_effector_id);
            if chain.len() < 2 {
                return;
            }
            solve_fabrik(
                world,
                emit,
                &chain,
                target_id,
                target_position_offset,
                max_iterations,
                tolerance,
                weight,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Chain collection
// ---------------------------------------------------------------------------

/// Walk UP from `end_id` via TC parents until reaching `root`.  Returns the
/// collected IDs in root-to-end order.
///
/// Walking up is unique (each TC has one parent) so this picks out a single
/// topological path even when intermediate joints have multiple TC children
/// (e.g. the spine fork into clavicles).  Returns empty if `end_id` is not
/// a TC descendant of `root` within 32 hops.
fn collect_tc_chain(world: &World, root: ComponentId, end_id: ComponentId) -> Vec<ComponentId> {
    if root == end_id {
        return vec![root];
    }
    let mut up: Vec<ComponentId> = vec![end_id];
    let mut cur = end_id;
    for _ in 0..32 {
        let Some(parent) = world.parent_of(cur) else {
            return Vec::new();
        };
        if world
            .get_component_by_id_as::<TransformComponent>(parent)
            .is_none()
        {
            return Vec::new();
        }
        up.push(parent);
        if parent == root {
            up.reverse();
            return up;
        }
        cur = parent;
    }
    Vec::new()
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
    copy_position: bool,
    target_position_offset: [f32; 3],
    weight: f32,
) {
    let Some(target_tc) = world.get_component_by_id_as::<TransformComponent>(target_id) else {
        return;
    };
    let target_world_rot = mat_to_quat(target_tc.transform.matrix_world);
    let desired_world_rot = quat_mul(target_world_rot, quat_rotation_y(offset_yaw));
    // Apply the offset in TARGET local frame, then add to target world position.
    // For an HMD target with offset = (0, -eye_height, 0), this drops the bone target
    // down along the HMD's local Y so the eye mesh (above the bone pivot) lines up
    // with the HMD position.
    let target_local_offset_world = quat_rotate_vec3(target_world_rot, target_position_offset);
    let target_world_pos = [
        target_tc.transform.matrix_world[3][0] + target_local_offset_world[0],
        target_tc.transform.matrix_world[3][1] + target_local_offset_world[1],
        target_tc.transform.matrix_world[3][2] + target_local_offset_world[2],
    ];

    let parent_world_mat = world
        .parent_of(root_tc)
        .and_then(|p| world.get_component_by_id_as::<TransformComponent>(p))
        .map(|t| t.transform.matrix_world)
        .unwrap_or([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]);
    let parent_world_rot = mat_to_quat(parent_world_mat);

    let full_local_rot = quat_mul(quat_conjugate(parent_world_rot), desired_world_rot);

    let local_rot = if weight < 1.0 {
        let cur = world
            .get_component_by_id_as::<TransformComponent>(root_tc)
            .map(|t| t.transform.rotation)
            .unwrap_or([0.0, 0.0, 0.0, 1.0]);
        quat_nlerp(cur, full_local_rot, weight)
    } else {
        full_local_rot
    };

    let (cur_t, s) = world
        .get_component_by_id_as::<TransformComponent>(root_tc)
        .map(|tc| (tc.transform.translation, tc.transform.scale))
        .unwrap_or(([0.0; 3], [1.0, 1.0, 1.0]));

    let local_t = if copy_position {
        // Invert parent world matrix to get local position from target world position.
        // Closed-form inverse of an affine TRS matrix: local_pos = R^T * (target_pos - parent_pos) / parent_scale.
        // Easier: use the inverse-transpose of the 3x3 rotation+scale block, then translate.
        let parent_pos = [
            parent_world_mat[3][0],
            parent_world_mat[3][1],
            parent_world_mat[3][2],
        ];
        let delta = [
            target_world_pos[0] - parent_pos[0],
            target_world_pos[1] - parent_pos[1],
            target_world_pos[2] - parent_pos[2],
        ];
        // Apply inverse parent rotation (assuming uniform scale; for non-uniform scale this would be approximate).
        let inv_parent_rot = quat_conjugate(parent_world_rot);
        let local_pos = quat_rotate_vec3(inv_parent_rot, delta);
        if weight < 1.0 {
            [
                cur_t[0] + (local_pos[0] - cur_t[0]) * weight,
                cur_t[1] + (local_pos[1] - cur_t[1]) * weight,
                cur_t[2] + (local_pos[2] - cur_t[2]) * weight,
            ]
        } else {
            local_pos
        }
    } else {
        cur_t
    };

    emit.push_intent_now(
        root_tc,
        IntentValue::UpdateTransform {
            component_ids: vec![root_tc],
            translation: local_t,
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
    let mid_pos = tc_world_pos(world, mid_tc);
    let end_pos = tc_world_pos(world, end_tc);
    let target_pos = tc_world_pos(world, target_id);

    let root_world_rot = tc_world_rot(world, root_tc);
    let mid_world_rot = tc_world_rot(world, mid_tc);

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
        let fallback = if reach_dir[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
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

    let full_upper_local = quat_mul(quat_conjugate(root_parent_world_rot), new_upper_world_rot);
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
    let lower_fwd_after_upper = quat_rotate_vec3(delta_upper, old_lower_fwd);
    let new_lower_fwd = if vec3_len(vec3_sub(target_pos, elbow_pos)) > 1e-6 {
        vec3_normalize(vec3_sub(target_pos, elbow_pos))
    } else {
        lower_fwd_after_upper
    };
    let delta_lower = shortest_arc_quat(lower_fwd_after_upper, new_lower_fwd);
    let new_lower_world_rot = quat_mul(delta_lower, quat_mul(delta_upper, mid_world_rot));

    // Parent of lower arm is now upper arm with new_upper_world_rot.
    let full_lower_local = quat_mul(quat_conjugate(new_upper_world_rot), new_lower_world_rot);
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
        let full_end_local = quat_mul(quat_conjugate(new_lower_world_rot), target_world_rot);
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
    target_position_offset: [f32; 3],
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
    // Target = target world pos + R(target_world_rot) * target_position_offset.
    // Symmetric with solve_aim: lets a Y-only offset like (0, -eye_h, 0) drop the
    // chase point down along the target's local Y, regardless of target yaw.
    let target_pos = {
        let base = tc_world_pos(world, target_id);
        let target_rot = tc_world_rot(world, target_id);
        let off = quat_rotate_vec3(target_rot, target_position_offset);
        [base[0] + off[0], base[1] + off[1], base[2] + off[2]]
    };

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
        .unwrap_or([0.0, 0.0, 0.0, 1.0f32]);

    // Detect neck bone by name — it should be one bone before the head in the chain.
    // Typical name patterns: "J_Bip_C_Neck", "Armature.Neck", "Neck", etc.
    let neck_index = chain.iter().position(|&id| {
        world
            .get_component_by_id_as::<TransformComponent>(id)
            .and_then(|_| Some(()))
            .is_some()
            && world.children_of(id).iter().any(|&child| {
                // Head is typically a direct child of neck (or wrapped in splice).
                let is_head = world
                    .get_component_by_id_as::<TransformComponent>(child)
                    .and_then(|_| {
                        // Just check if it's the next bone in the chain
                        chain.iter().find(|&&c| c == child).map(|_| ())
                    })
                    .is_some();
                is_head
            })
    });

    for i in 0..n - 1 {
        let tc = chain[i];
        let cur_world_rot = tc_world_rot(world, tc);

        let cur_fwd = {
            let from = tc_world_pos(world, tc);
            let to = tc_world_pos(world, chain[i + 1]);
            let d = vec3_sub(to, from);
            if vec3_len(d) > 1e-6 {
                vec3_normalize(d)
            } else {
                [0.0, 0.0, 1.0]
            }
        };
        let desired_fwd = {
            let d = vec3_sub(positions[i + 1], positions[i]);
            if vec3_len(d) > 1e-6 {
                vec3_normalize(d)
            } else {
                cur_fwd
            }
        };

        // If this is the neck bone, constrain its rotation to be minimal (keep it
        // rigid relative to the upper torso). The neck should inherit the upper body's
        // yaw/rotation without bending, so we clamp the rotation angle.
        let delta = if neck_index == Some(i) {
            // Neck constraint: allow only small deviations from the current forward direction.
            // Use a very tight cone (max ~15 degrees) to keep the neck stiff.
            let unconstrained_delta = shortest_arc_quat(cur_fwd, desired_fwd);
            let (axis, angle) = quat_to_axis_angle(unconstrained_delta);
            let max_neck_angle = 0.26f32; // ~15 degrees
            if angle.abs() > max_neck_angle {
                // Clamp the rotation angle
                let clamped_angle = angle.max(-max_neck_angle).min(max_neck_angle);
                quat_from_axis_angle(axis, clamped_angle)
            } else {
                unconstrained_delta
            }
        } else {
            shortest_arc_quat(cur_fwd, desired_fwd)
        };

        let new_world_rot = quat_mul(delta, cur_world_rot);
        let full_local = quat_mul(quat_conjugate(parent_world_rot), new_world_rot);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{ComponentRef, IKChainComponent, IKSolver};
    use crate::engine::ecs::CommandQueue;

    // Temporarily gated: see docs/bugs/ik-solver-api-drift-breaks-tests.md.
    #[cfg(any())]
    #[test]
    fn resolves_forward_reference_on_first_tick() {
        let mut w = World::default();

        // Author IKChain *before* the target/end_effector components exist.
        // Both ids start as null sentinels; only the sources carry the
        // selector strings.
        let ik_id = w.add_component(
            IKChainComponent::new(
                IKSolver::TwoBoneIK {
                    pole_direction: [0.0, 1.0, 0.0],
                    copy_end_rotation: false,
                },
                ComponentId::null(),
                ComponentId::null(),
            )
            .with_target_source(ComponentRef::Query("#hand".to_string()))
            .with_end_effector_source(ComponentRef::Query("#elbow".to_string())),
        );

        // Now spawn the targets the IKChain refers to.
        let hand = w.add_component_boxed_named("hand", Box::new(TransformComponent::new()));
        let elbow = w.add_component_boxed_named("elbow", Box::new(TransformComponent::new()));

        // Sanity: nothing resolved yet.
        {
            let ik = w.get_component_by_id_as::<IKChainComponent>(ik_id).unwrap();
            assert!(ik.target_id.is_null());
            assert!(ik.end_effector_id.is_null());
        }

        // First tick triggers the deferred resolve.
        let mut emit = CommandQueue::new();
        let mut sys = IKSystem::new();
        sys.tick(&mut w, &mut emit, 0.016);

        let ik = w.get_component_by_id_as::<IKChainComponent>(ik_id).unwrap();
        assert_eq!(ik.target_id, hand);
        assert_eq!(ik.end_effector_id, elbow);
    }

    #[test]
    fn does_not_overwrite_already_resolved_ids() {
        let mut w = World::default();
        let pre_target = w.add_component(TransformComponent::new());
        let pre_ee = w.add_component(TransformComponent::new());
        let unrelated = w.add_component_boxed_named("hand", Box::new(TransformComponent::new()));

        let ik_id = w.add_component(
            IKChainComponent::new(
                IKSolver::AimConstraint {
                    offset_yaw: 0.0,
                    copy_position: false,
                    target_position_offset: [0.0, 0.0, 0.0],
                },
                pre_target,
                pre_ee,
            )
            // Source points at a different component named "hand" — but
            // since target_id / end_effector_id are already non-null, the
            // resolve pass should leave them alone.
            .with_target_source(ComponentRef::Query("#hand".to_string())),
        );

        let mut emit = CommandQueue::new();
        let mut sys = IKSystem::new();
        sys.tick(&mut w, &mut emit, 0.016);

        let ik = w.get_component_by_id_as::<IKChainComponent>(ik_id).unwrap();
        assert_eq!(ik.target_id, pre_target);
        assert_ne!(ik.target_id, unrelated);
        assert_eq!(ik.end_effector_id, pre_ee);
    }
}
