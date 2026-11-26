//! Kneeling leg goals for the body solver's horizontal mode: knees on the
//! support under the hip joints, shins flat behind them, feet pointing back.
//! A crawling gait keeps each knee planted on the floor until the body moves
//! away from it, then steps it, one leg at a time.
use super::leg::solve_leg;
use super::rig::{LowerBodyRig, emit_pose, uniform};
use super::support::SupportQuery;
use super::{FootGoal, FootPhase, FootState, LowerBodyStatus};
use crate::engine::ecs::SignalEmitter;
use crate::engine::ecs::system::ik::body::KNEE_RADIUS;
use crate::engine::transform::TransformMatrix;
use crate::utils::math::{
    quat_from_axis_angle, quat_mul, vec3_cross, vec3_dot, vec3_len, vec3_normalize, vec3_sub,
};

/// Ankle clearance above the floor with the shin lying flat, in model units.
const ANKLE_RADIUS: f32 = 0.05;
/// Minimum drop from the hip to a knee goal, and tolerance for resolved contact.
const KNEE_CLEARANCE: f32 = 0.02;
/// Largest floor change accepted while kneeling, in model units.
const MAX_STEP: f32 = 0.3;
/// A planted knee steps once its rest spot is this far away, in model units.
const STEP_TRIGGER: f32 = 0.12;
const SWING_SEC: f32 = 0.28;
/// Swing lift at mid-step, in model units.
const LIFT: f32 = 0.06;
/// A step lands this fraction of its travel ahead of the rest spot.
const OVERSHOOT: f32 = 0.35;

/// Crawl stepping state for one avatar. Reset when kneeling starts or stops.
#[derive(Debug, Default)]
pub(super) struct KneelGait {
    planted: [Option<Plant>; 2],
    swing: Option<Swing>,
    /// Side to prefer for the next step, so legs alternate.
    next: usize,
    /// Completed steps per leg, for diagnostics.
    pub(super) steps: [u64; 2],
}

/// A planted knee: its floor spot and where its hip was when it landed.
#[derive(Debug, Clone, Copy)]
struct Plant {
    spot: [f32; 3],
    hip: [f32; 3],
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
    /// Knee contact goals for this frame. A knee steps once its hip has moved
    /// `STEP_TRIGGER` since that knee landed; `rest` is where it would land.
    fn advance(
        &mut self,
        rest: [[f32; 3]; 2],
        hips: [[f32; 3]; 2],
        scale: f32,
        dt: f32,
    ) -> ([[f32; 3]; 2], [FootPhase; 2]) {
        for i in 0..2 {
            // Planted knees keep their spot but follow floor height changes.
            let plant = self.planted[i].get_or_insert(Plant {
                spot: rest[i],
                hip: hips[i],
            });
            plant.spot[1] = rest[i][1];
        }
        if self.swing.is_none() {
            let far: [bool; 2] = std::array::from_fn(|i| {
                horizontal_distance(self.planted[i].unwrap().hip, hips[i]) > STEP_TRIGGER * scale
            });
            let side = if far[self.next] {
                Some(self.next)
            } else if far[1 - self.next] {
                Some(1 - self.next)
            } else {
                None
            };
            if let Some(side) = side {
                let plant = self.planted[side].unwrap();
                let travel = vec3_sub(hips[side], plant.hip);
                let to = [
                    rest[side][0] + travel[0] * OVERSHOOT,
                    rest[side][1],
                    rest[side][2] + travel[2] * OVERSHOOT,
                ];
                self.swing = Some(Swing {
                    side,
                    from: plant.spot,
                    to,
                    progress: 0.0,
                });
                self.next = 1 - side;
            }
        }
        let mut goals = [self.planted[0].unwrap().spot, self.planted[1].unwrap().spot];
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
                self.planted[swing.side] = Some(Plant {
                    spot: swing.to,
                    hip: hips[swing.side],
                });
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
    let clearance = KNEE_CLEARANCE * scale;
    let thighs = rig
        .legs
        .each_ref()
        .map(|leg| vec3_len(leg.rest[1].translation) * scale);
    let floors = hips.map(|hip| {
        ground
            .at(hip)
            .filter(|s| (s.height - previous).abs() <= MAX_STEP * scale)
            .map_or(previous, |s| s.height)
    });
    // Knee contact spots: where the thigh reaches the floor. Straight below
    // the hip when the hips sit a thigh length up; forward of it when they are
    // lower, so low hips sit back behind the knees instead of sinking them.
    let rest: [[f32; 3]; 2] = std::array::from_fn(|i| {
        let drop = hips[i][1] - (floors[i] + KNEE_RADIUS * scale);
        let drop = if drop < clearance {
            thighs[i]
        } else {
            drop.min(thighs[i])
        };
        knee_at_drop(hips[i], forward, thighs[i], drop)
    });
    let dt = if dt_sec.is_finite() {
        dt_sec.max(0.0)
    } else {
        0.0
    };
    let (spots, phases) = gait.advance(rest, hips, scale, dt);
    let mut solved = [None, None];
    for i in 0..2 {
        let leg = &rig.legs[i];
        let hip = hips[i];
        let thigh = thighs[i];
        // A moving plant can trail the hip. Keep the knee in its forward
        // hemisphere, and recompute reachable extension after swing lift.
        let offset = vec3_sub(spots[i], hip);
        let ahead = vec3_dot(offset, forward).max(1e-4 * scale);
        let lateral = vec3_dot(offset, left);
        let direction = vec3_normalize(std::array::from_fn(|axis| {
            forward[axis] * ahead + left[axis] * lateral
        }));
        let lift = if hip[1] - (floors[i] + KNEE_RADIUS * scale) >= clearance {
            (spots[i][1] - rest[i][1]).max(0.0)
        } else {
            0.0
        };
        let drop = (hip[1] - rest[i][1] - lift).clamp(clearance.min(thigh), thigh);
        let knee = knee_at_drop(hip, direction, thigh, drop);
        let shin = vec3_len(leg.rest[2].translation) * scale;
        let ankle_drop = ((KNEE_RADIUS - ANKLE_RADIUS) * scale).min(shin);
        let trailing = (shin * shin - ankle_drop * ankle_drop).max(0.0).sqrt();
        let rotation = quat_mul(turn_over, quat_mul(body_rotation, leg.foot_in_root));
        let solve_at = |knee: [f32; 3]| {
            // The shin trails the knee in both stance and swing. Build the
            // ankle goal from the reachable knee, so it moves down with it.
            let ankle = [
                knee[0] - forward[0] * trailing,
                knee[1] - ankle_drop,
                knee[2] - forward[2] * trailing,
            ];
            let goal = FootGoal {
                sole: ankle,
                rotation,
            };
            // Bend toward the knee spot: away from the hip-to-ankle chord.
            let chord = vec3_sub(ankle, hip);
            let to_knee = vec3_sub(knee, hip);
            let along = vec3_dot(to_knee, chord) / vec3_dot(chord, chord).max(1e-9);
            let off = vec3_sub(to_knee, chord.map(|v| v * along));
            let bend = if vec3_len(off) > 1e-4 * scale {
                vec3_normalize(off)
            } else {
                pole
            };
            solve_leg(leg, bases[i], [0.0; 3], goal, bend, knee_limit)
                .map(|solution| (goal, solution))
        };
        let (mut goal, mut solution) = solve_at(knee).map_err(UnsupportedRig)?;
        if solution.knee[1] >= hip[1] {
            // The knee-angle limit can still push a nearly folded solution
            // above its hip. Give up contact and tuck the leg down instead.
            (goal, solution) =
                solve_at(knee_at_drop(hip, forward, thigh, thigh)).map_err(UnsupportedRig)?;
        }
        let knee_floor = solution.knee[1] - KNEE_RADIUS * scale;
        let support = (phases[i] == FootPhase::Planted
            && (knee_floor - floors[i]).abs() <= clearance)
            .then(|| ground.at(solution.knee))
            .flatten()
            .filter(|s| (knee_floor - s.height).abs() <= clearance);
        solved[i] = Some((goal, solution, support));
    }
    // Both legs validated: emit them together.
    Ok(std::array::from_fn(|i| {
        let (goal, solution, support) = solved[i].take().unwrap();
        let leg = &mut rig.legs[i];
        for (joint, local) in leg.joints.into_iter().zip(solution.local) {
            emit_pose(emit, joint, local);
        }
        leg.active = true;
        FootState {
            support: support.map(|s| s.zone),
            support_height: Some(support.map_or(floors[i], |s| s.height)),
            base_sole: goal.sole,
            solved_sole: solution.sole,
            penetration: 0.0,
            knee_flexion: solution.flexion,
            knee: solution.knee,
            residual: 0.0,
            target_sole: goal.sole,
            target_rotation: goal.rotation,
            target_error: vec3_len(vec3_sub(goal.sole, solution.sole)),
            phase: if phases[i] == FootPhase::Planted && support.is_none() {
                FootPhase::Airborne
            } else {
                phases[i]
            },
        }
    }))
}

fn knee_at_drop(hip: [f32; 3], direction: [f32; 3], thigh: f32, drop: f32) -> [f32; 3] {
    let reach = (thigh * thigh - drop * drop).max(0.0).sqrt();
    [
        hip[0] + direction[0] * reach,
        hip[1] - drop,
        hip[2] + direction[2] * reach,
    ]
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
            let (goals, phases) = gait.advance(rest_at(0.0), rest_at(0.0), 1.0, 1.0 / 60.0);
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
            let (goals, phases) = gait.advance(rest_at(x), rest_at(x), 1.0, 1.0 / 60.0);
            for i in 0..2 {
                assert!(
                    goals[i][1] >= 0.05 - 1e-6,
                    "knee stays at or above the floor"
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
        // Planted knees trail the body by at most about two step triggers plus
        // the distance covered during the other leg's swing.
        let planted = gait.planted[0].unwrap().spot[0];
        assert!(
            planted > x - 2.0 * STEP_TRIGGER - 0.15,
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
            gait.advance(rest, rest, 1.0, 1.0 / 60.0);
        }
        assert!(gait.steps.iter().sum::<u64>() > 0);
    }
}
