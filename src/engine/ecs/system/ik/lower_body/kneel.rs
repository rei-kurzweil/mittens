//! Kneeling leg goals for the body solver's horizontal mode: knees on the
//! support under the hip joints, shins flat behind them, feet pointing back.
//! A crawling gait keeps each ankle planted until the body moves away from
//! it, then steps it, one leg at a time.
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
/// A planted ankle steps once its rest spot is this far away, in model units.
const STEP_TRIGGER: f32 = 0.12;
const SWING_SEC: f32 = 0.28;
/// Swing lift at mid-step, in model units.
const LIFT: f32 = 0.06;
/// A step lands this fraction of its travel ahead of the rest spot.
const OVERSHOOT: f32 = 0.35;

/// Crawl stepping state for one avatar. Reset when kneeling starts or stops.
#[derive(Debug, Default)]
pub(super) struct KneelGait {
    planted: [Option<[f32; 3]>; 2],
    swing: Option<Swing>,
    /// Side to prefer for the next step, so legs alternate.
    next: usize,
    /// Completed steps per leg, for diagnostics.
    pub(super) steps: [u64; 2],
}

#[derive(Debug, Clone, Copy)]
struct Swing {
    side: usize,
    from: [f32; 3],
    to: [f32; 3],
    progress: f32,
}

fn horizontal_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

impl KneelGait {
    /// Ankle goals for this frame from each leg's rest spot under the hip.
    fn advance(
        &mut self,
        rest: [[f32; 3]; 2],
        scale: f32,
        dt: f32,
    ) -> ([[f32; 3]; 2], [FootPhase; 2]) {
        for i in 0..2 {
            // Planted ankles keep their spot but follow floor height changes.
            let planted = self.planted[i].get_or_insert(rest[i]);
            planted[1] = rest[i][1];
        }
        if self.swing.is_none() {
            let far: [bool; 2] = std::array::from_fn(|i| {
                horizontal_distance(self.planted[i].unwrap(), rest[i]) > STEP_TRIGGER * scale
            });
            let side = if far[self.next] {
                Some(self.next)
            } else if far[1 - self.next] {
                Some(1 - self.next)
            } else {
                None
            };
            if let Some(side) = side {
                let from = self.planted[side].unwrap();
                let travel = vec3_sub(rest[side], from);
                let to = [
                    rest[side][0] + travel[0] * OVERSHOOT,
                    rest[side][1],
                    rest[side][2] + travel[2] * OVERSHOOT,
                ];
                self.swing = Some(Swing {
                    side,
                    from,
                    to,
                    progress: 0.0,
                });
                self.next = 1 - side;
            }
        }
        let mut goals = [self.planted[0].unwrap(), self.planted[1].unwrap()];
        let mut phases = [FootPhase::Planted; 2];
        if let Some(swing) = &mut self.swing {
            swing.progress = (swing.progress + dt / SWING_SEC).min(1.0);
            swing.to[1] = rest[swing.side][1];
            let t = swing.progress;
            let eased = t * t * (3.0 - 2.0 * t);
            let lift = LIFT * scale * (std::f32::consts::PI * t).sin();
            goals[swing.side] = std::array::from_fn(|k| {
                swing.from[k]
                    + (swing.to[k] - swing.from[k]) * eased
                    + if k == 1 { lift } else { 0.0 }
            });
            phases[swing.side] = FootPhase::Swing;
            if t >= 1.0 {
                self.planted[swing.side] = Some(swing.to);
                self.steps[swing.side] += 1;
                goals[swing.side] = swing.to;
                self.swing = None;
            }
        }
        (goals, phases)
    }
}

pub(super) fn solve(
    rig: &mut LowerBodyRig,
    gait: &mut KneelGait,
    bases: [TransformMatrix; 2],
    ground: &SupportQuery,
    previous_floor: Option<f32>,
    body_rotation: [f32; 4],
    knee_limit: f32,
    dt_sec: f32,
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
    // The floor carries over from standing. A fresh query may replace it
    // only within a step of that floor: the query's reference height
    // follows the pitched body, and must not drag the floor down with it.
    let Some(previous) = previous_floor else {
        return Err(LowerBodyStatus::NoSupport);
    };
    let scale = uniform(bases[0]).map_err(UnsupportedRig)?.scale[0];
    let mut supports = [None, None];
    let mut floors = [previous; 2];
    let rest: [[f32; 3]; 2] = std::array::from_fn(|i| {
        let shin = vec3_len(rig.legs[i].rest[2].translation) * scale;
        supports[i] = ground
            .at(hips[i])
            .filter(|s| (s.height - previous).abs() <= MAX_STEP * scale);
        floors[i] = supports[i].map(|s| s.height).unwrap_or(previous);
        [
            hips[i][0] - forward[0] * shin,
            floors[i] + ANKLE_RADIUS * scale,
            hips[i][2] - forward[2] * shin,
        ]
    });
    let dt = if dt_sec.is_finite() {
        dt_sec.max(0.0)
    } else {
        0.0
    };
    let (ankles, phases) = gait.advance(rest, scale, dt);
    let mut solved = [None, None];
    for i in 0..2 {
        let leg = &rig.legs[i];
        let rotation = quat_mul(turn_over, quat_mul(body_rotation, leg.foot_in_root));
        let goal = FootGoal {
            sole: ankles[i],
            rotation,
        };
        let solution =
            solve_leg(leg, bases[i], [0.0; 3], goal, pole, knee_limit).map_err(UnsupportedRig)?;
        solved[i] = Some((goal, solution));
    }
    // Both legs validated: emit them together.
    Ok(std::array::from_fn(|i| {
        let (goal, solution) = solved[i].take().unwrap();
        let leg = &mut rig.legs[i];
        for (joint, local) in leg.joints.into_iter().zip(solution.local) {
            emit_pose(emit, joint, local);
        }
        leg.active = true;
        FootState {
            support: supports[i].map(|s| s.zone),
            support_height: Some(floors[i]),
            base_sole: rest[i],
            solved_sole: solution.sole,
            penetration: 0.0,
            knee_flexion: solution.flexion,
            residual: 0.0,
            target_sole: goal.sole,
            target_rotation: goal.rotation,
            target_error: vec3_len(vec3_sub(goal.sole, solution.sole)),
            phase: phases[i],
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rest_at(x: f32) -> [[f32; 3]; 2] {
        [[x, 0.05, 0.1], [x, 0.05, -0.1]]
    }

    #[test]
    fn still_body_never_steps() {
        let mut gait = KneelGait::default();
        for _ in 0..120 {
            let (goals, phases) = gait.advance(rest_at(0.0), 1.0, 1.0 / 60.0);
            assert_eq!(goals, rest_at(0.0));
            assert_eq!(phases, [FootPhase::Planted; 2]);
        }
        assert_eq!(gait.steps, [0, 0]);
    }

    #[test]
    fn moving_body_steps_alternately_and_lands_ahead() {
        let mut gait = KneelGait::default();
        let mut x = 0.0;
        let mut swinging = Vec::new();
        for _ in 0..240 {
            x += 0.4 / 60.0; // 0.4 m/s crawl
            let (goals, phases) = gait.advance(rest_at(x), 1.0, 1.0 / 60.0);
            for i in 0..2 {
                assert!(
                    goals[i][1] >= 0.05 - 1e-6,
                    "ankle stays at or above the floor"
                );
                if phases[i] == FootPhase::Swing && swinging.last() != Some(&i) {
                    swinging.push(i);
                }
            }
            assert!(
                phases.iter().filter(|p| **p == FootPhase::Swing).count() <= 1,
                "one leg at a time"
            );
        }
        assert!(gait.steps[0] >= 3 && gait.steps[1] >= 3, "{:?}", gait.steps);
        assert!(
            swinging.windows(2).all(|w| w[0] != w[1]),
            "legs alternate: {swinging:?}"
        );
        // A completed step lands ahead of where the rest spot was when it started.
        let planted = gait.planted[0].unwrap()[0];
        assert!(
            planted > x - STEP_TRIGGER - 0.05,
            "planted near the body: {planted} vs {x}"
        );
    }

    #[test]
    fn turning_in_place_steps() {
        let mut gait = KneelGait::default();
        for frame in 0..120 {
            let angle = frame as f32 * 0.02;
            let (c, s) = (angle.cos(), angle.sin());
            let rest = [
                [0.1 * s, 0.05, 0.1 * c],
                [-0.3 * c - 0.1 * s, 0.05, 0.3 * s - 0.1 * c],
            ];
            gait.advance(rest, 1.0, 1.0 / 60.0);
        }
        assert!(gait.steps.iter().sum::<u64>() > 0);
    }
}
