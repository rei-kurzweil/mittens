//! Kneeling leg goals for the body solver's horizontal mode: knees on the
//! support under the hip joints, shins flat behind them, feet pointing back.
use super::leg::solve_leg;
use super::rig::{LowerBodyRig, emit_pose, uniform};
use super::support::SupportQuery;
use super::{FootGoal, FootPhase, FootState, LowerBodyStatus};
use crate::engine::ecs::SignalEmitter;
use crate::engine::transform::TransformMatrix;
use crate::utils::math::{
    quat_from_axis_angle, quat_mul, vec3_cross, vec3_len, vec3_normalize, vec3_sub,
};

/// Ankle clearance above the floor with the shin lying flat, in model units.
const ANKLE_RADIUS: f32 = 0.05;
/// Largest floor change accepted while kneeling, in model units.
const MAX_STEP: f32 = 0.3;

pub(super) fn solve(
    rig: &mut LowerBodyRig,
    bases: [TransformMatrix; 2],
    ground: &SupportQuery,
    previous_floor: Option<f32>,
    body_rotation: [f32; 4],
    knee_limit: f32,
    emit: &mut dyn SignalEmitter,
) -> Result<[FootState; 2], LowerBodyStatus> {
    use LowerBodyStatus::UnsupportedRig;
    let hips: [[f32; 3]; 2] = bases.map(|b| [b[3][0], b[3][1], b[3][2]]);
    let across = vec3_sub(hips[0], hips[1]);
    let left = vec3_normalize([across[0], 0.0, across[2]]);
    if !left.iter().all(|v| v.is_finite()) || vec3_len(left) < 0.5 {
        return Err(UnsupportedRig("hip joints coincide"));
    }
    let up = [0.0, 1.0, 0.0];
    let forward = vec3_cross(left, up);
    let pole = vec3_normalize(vec3_sub(forward, up));
    // Turn the upright foot over so it points back along the shin, sole up.
    let turn_over = quat_from_axis_angle(left, std::f32::consts::PI);
    let mut solved = [None, None];
    for i in 0..2 {
        let leg = &rig.legs[i];
        let scale = uniform(bases[i]).map_err(UnsupportedRig)?.scale[0];
        let shin = vec3_len(leg.rest[2].translation) * scale;
        // The floor carries over from standing. A fresh query may replace it
        // only within a step of that floor: the query's reference height
        // follows the pitched body, and must not drag the floor down with it.
        let Some(previous) = previous_floor else {
            return Err(LowerBodyStatus::NoSupport);
        };
        let support = ground
            .at(hips[i])
            .filter(|s| (s.height - previous).abs() <= MAX_STEP * scale);
        let floor = support.map(|s| s.height).unwrap_or(previous);
        let ankle = [
            hips[i][0] - forward[0] * shin,
            floor + ANKLE_RADIUS * scale,
            hips[i][2] - forward[2] * shin,
        ];
        let rotation = quat_mul(turn_over, quat_mul(body_rotation, leg.foot_in_root));
        let goal = FootGoal {
            sole: ankle,
            rotation,
        };
        let solution =
            solve_leg(leg, bases[i], [0.0; 3], goal, pole, knee_limit).map_err(UnsupportedRig)?;
        solved[i] = Some((goal, support, floor, solution));
    }
    // Both legs validated: emit them together.
    Ok(std::array::from_fn(|i| {
        let (goal, support, floor, solution) = solved[i].take().unwrap();
        let leg = &mut rig.legs[i];
        for (joint, local) in leg.joints.into_iter().zip(solution.local) {
            emit_pose(emit, joint, local);
        }
        leg.active = true;
        FootState {
            support: support.map(|s| s.zone),
            support_height: Some(floor),
            base_sole: goal.sole,
            solved_sole: solution.sole,
            penetration: 0.0,
            knee_flexion: solution.flexion,
            residual: 0.0,
            target_sole: goal.sole,
            target_rotation: goal.rotation,
            target_error: vec3_len(vec3_sub(goal.sole, solution.sole)),
            phase: FootPhase::Rest,
        }
    }))
}
