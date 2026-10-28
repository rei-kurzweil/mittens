//! Bounded leg geometry. Does not emit transforms or choose behavior.
use super::FootGoal;
use super::rig::{Leg, uniform};
use crate::engine::transform::{TransformMatrix, TransformTrs};
use crate::utils::math::{
    mat4_mul, quat_conjugate, quat_mul, quat_rotate_vec3, shortest_arc_quat, vec3_add, vec3_cross,
    vec3_len, vec3_normalize, vec3_scale, vec3_sub,
};

pub(super) struct LegSolution {
    pub(super) local: [TransformTrs; 3],
    pub(super) sole: [f32; 3],
    pub(super) flexion: f32,
}
pub(super) fn solve_leg(
    leg: &Leg,
    upper: TransformMatrix,
    sole_offset: [f32; 3],
    goal: FootGoal,
    pole: [f32; 3],
    max_flexion: f32,
) -> Result<LegSolution, &'static str> {
    let mid = mat4_mul(upper, leg.rest[1].to_matrix().unwrap());
    let foot = mat4_mul(mid, leg.rest[2].to_matrix().unwrap());
    let upper_trs = uniform(upper)?;
    let mid_trs = uniform(mid)?;
    let foot_trs = uniform(foot)?;
    let root = upper_trs.translation;
    let old_mid = mid_trs.translation;
    let old_end = foot_trs.translation;
    let scaled_offset = sole_offset.map(|v| v * foot_trs.scale[0]);
    let target = vec3_sub(goal.sole, quat_rotate_vec3(goal.rotation, scaled_offset));
    let a = vec3_len(vec3_sub(old_mid, root));
    let b = vec3_len(vec3_sub(old_end, old_mid));
    if !a.is_finite() || !b.is_finite() || a < 1e-6 || b < 1e-6 || !(a + b).is_finite() {
        return Err("degenerate world-space leg segment");
    }
    let raw = vec3_sub(target, root);
    // Never bend the ankle above the hip on an unsupported very deep crouch.
    // Clamp to the last reachable downward target and expose its residual.
    let target_vector = [raw[0], raw[1].min(-1e-5 * (a + b)), raw[2]];
    let raw_reach = vec3_len(target_vector);
    if !raw_reach.is_finite() {
        return Err("nonfinite ankle target");
    }
    let min_reach = (a * a + b * b + 2.0 * a * b * max_flexion.cos())
        .max(0.0)
        .sqrt()
        .max((a - b).abs() + 1e-6 * (a + b));
    let reach = raw_reach.clamp(min_reach.min(a + b), a + b);
    let horizontal_squared = raw[0] * raw[0] + raw[2] * raw[2];
    let end = if horizontal_squared <= reach * reach {
        // Keep the stance: excess depth is vertical residual, not sliding feet.
        [
            target[0],
            root[1] - (reach * reach - horizontal_squared).max(0.0).sqrt(),
            target[2],
        ]
    } else {
        vec3_add(root, vec3_scale(vec3_normalize(target_vector), reach))
    };
    let direction = vec3_normalize(vec3_sub(end, root));
    let mut normal = vec3_cross(direction, pole);
    if vec3_len(normal) < 1e-5 {
        normal = vec3_cross(direction, [1.0, 0.0, 0.0]);
    }
    if vec3_len(normal) < 1e-5 {
        normal = vec3_cross(direction, [0.0, 0.0, 1.0]);
    }
    let knee =
        super::super::two_bone::midpoint(root, direction, vec3_normalize(normal), a, b, reach);
    let upper_delta = shortest_arc_quat(
        vec3_normalize(vec3_sub(old_mid, root)),
        vec3_normalize(vec3_sub(knee, root)),
    );
    let upper_rotation = quat_mul(upper_delta, upper_trs.rotation_quat_xyzw);
    let lower_delta = shortest_arc_quat(
        quat_rotate_vec3(upper_delta, vec3_normalize(vec3_sub(old_end, old_mid))),
        vec3_normalize(vec3_sub(end, knee)),
    );
    let lower_rotation = quat_mul(
        lower_delta,
        quat_mul(upper_delta, mid_trs.rotation_quat_xyzw),
    );
    let parent_rotation = quat_mul(
        upper_trs.rotation_quat_xyzw,
        quat_conjugate(leg.rest[0].rotation_quat_xyzw),
    );
    let rotations = [
        quat_mul(quat_conjugate(parent_rotation), upper_rotation),
        quat_mul(quat_conjugate(upper_rotation), lower_rotation),
        quat_mul(quat_conjugate(lower_rotation), goal.rotation),
    ];
    let local = std::array::from_fn(|i| TransformTrs {
        rotation_quat_xyzw: rotations[i],
        ..leg.rest[i]
    });
    if local.iter().any(|p| !p.is_finite()) {
        return Err("nonfinite leg solve");
    }
    let sole = vec3_add(end, quat_rotate_vec3(goal.rotation, scaled_offset));
    let flexion = ((reach * reach - a * a - b * b) / (2.0 * a * b))
        .clamp(-1.0, 1.0)
        .acos();
    Ok(LegSolution {
        local,
        sole,
        flexion,
    })
}
