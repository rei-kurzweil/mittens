//! Stateful foot placement. Produces goals; never writes joints or locomotion.
use super::{
    FootGoal, FootPhase, WalkingDiagnostics,
    crouch::RestGoal,
    support::{Support, SupportQuery},
};
use crate::engine::ecs::component::WalkingIkComponent;
use crate::utils::math::{quat_nlerp, vec3_add, vec3_dot, vec3_len, vec3_scale, vec3_sub};

#[derive(Debug, Clone, Copy)]
struct Contact {
    foot: FootGoal,
    support: Support,
}

#[derive(Debug)]
struct Swing {
    side: usize,
    from: FootGoal,
    to: Contact,
    elapsed: f32,
    duration: f32,
    height: f32,
    forward_bias: f32,
}

impl Swing {
    fn goal(&self) -> FootGoal {
        let u = (self.elapsed / self.duration).clamp(0.0, 1.0);
        // Advance horizontal travel earlier in a moving step, while retaining
        // zero endpoint velocity and the full clearance arc over time.
        let travel = u + self.forward_bias * u * (1.0 - u);
        let t = travel * travel * travel * (10.0 + travel * (-15.0 + 6.0 * travel));
        let mut sole = vec3_add(
            self.from.sole,
            vec3_scale(vec3_sub(self.to.foot.sole, self.from.sole), t),
        );
        sole[1] += self.height * 64.0 * (u * (1.0 - u)).powi(3);
        FootGoal {
            sole,
            rotation: quat_nlerp(self.from.rotation, self.to.foot.rotation, t),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct WalkingGoal {
    pub foot: FootGoal,
    pub support: Support,
    pub phase: FootPhase,
}

#[derive(Debug, Default)]
pub(super) struct WalkingState {
    contacts: Option<[Contact; 2]>,
    swing: Option<Swing>,
    previous_position: Option<[f32; 3]>,
    scale: f32,
    velocity: [f32; 3],
    next_side: Option<usize>,
    transfer_seconds: f32,
    steps: [u64; 2],
}

pub(super) fn horizontal(v: [f32; 3]) -> [f32; 3] {
    [v[0], 0.0, v[2]]
}

impl WalkingState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn diagnostics(&self) -> WalkingDiagnostics {
        WalkingDiagnostics {
            steps_started: self.steps,
            swing_side: self.swing.as_ref().map(|s| s.side),
            swing_progress: self
                .swing
                .as_ref()
                .map(|s| (s.elapsed / s.duration).clamp(0.0, 1.0)),
        }
    }

    pub fn advance(
        &mut self,
        rest: [RestGoal; 2],
        position: [f32; 3],
        scale: f32,
        config: &WalkingIkComponent,
        dt: f32,
        ground: &SupportQuery<'_>,
    ) -> Option<[WalkingGoal; 2]> {
        if !dt.is_finite() || dt <= 0.0 || dt > 0.25 {
            self.reset();
            return None;
        }
        let base_stride = config.stride * scale;
        let stride = base_stride * config.step_scale;
        let displacement = self
            .previous_position
            .map(|p| horizontal(vec3_sub(position, p)));
        if self.scale > 0.0 && (self.scale - scale).abs() > scale * 0.0001
            || displacement.is_some_and(|d| vec3_len(d) > stride.max(scale))
        {
            self.reset();
        }
        self.scale = scale;
        if let Some(previous) = self.previous_position {
            let measured = vec3_scale(horizontal(vec3_sub(position, previous)), 1.0 / dt);
            let alpha = 1.0 - (-dt / 0.08).exp();
            self.velocity = vec3_add(
                self.velocity,
                vec3_scale(vec3_sub(measured, self.velocity), alpha),
            );
        }
        self.previous_position = Some(position);

        // Contact removal, floor edits, jumping and excessive body rise release
        // the gait. Source support is irrelevant once that foot is in swing.
        let lost_contact = self.contacts.is_some_and(|contacts| {
            (0..2).any(|i| {
                self.swing.as_ref().is_none_or(|s| s.side != i)
                    && !ground.retains(contacts[i].foot.sole, contacts[i].support)
            })
        });
        let lost_landing = self
            .swing
            .as_ref()
            .is_some_and(|s| !ground.retains(s.to.foot.sole, s.to.support));
        let body_too_far = self.contacts.is_some_and(|contacts| {
            (0..2).any(|i| {
                vec3_len(horizontal(vec3_sub(
                    rest[i].foot.sole,
                    contacts[i].foot.sole,
                ))) > stride * 3.0
            })
        });
        let raised_body = rest.iter().any(|r| {
            r.support
                .is_some_and(|s| r.foot.sole[1] - s.height > 0.08 * scale)
        });
        if lost_contact || lost_landing || body_too_far || raised_body {
            self.reset();
            return None;
        }

        if self.contacts.is_none() {
            let [Some(left), Some(right)] = rest.map(|r| {
                r.support.map(|support| {
                    let mut foot = r.foot;
                    foot.sole[1] = support.height;
                    Contact { foot, support }
                })
            }) else {
                return None;
            };
            self.contacts = Some([left, right]);
            return self.goals();
        }

        // Idle time is never banked toward a future step. A newly admitted
        // idle step starts now; an existing gait owns this frame's full budget.
        let mut remaining = if self.swing.is_some() || self.transfer_seconds > 0.0 {
            dt
        } else {
            0.0
        };
        let speed = vec3_len(self.velocity);
        let direction = if speed > 0.01 * scale {
            vec3_scale(self.velocity, 1.0 / speed)
        } else {
            [0.0; 3]
        };
        loop {
            let contacts = self.contacts.unwrap();
            // Only lag along actual travel demands recovery. A foot ahead of the
            // body or a turn in place must not be treated as trailing behind it.
            let lag = (0..2)
                .map(|i| {
                    let foot = self
                        .swing
                        .as_ref()
                        .filter(|s| s.side == i)
                        .map_or(contacts[i].foot, Swing::goal);
                    vec3_dot(
                        horizontal(vec3_sub(rest[i].foot.sole, foot.sole)),
                        direction,
                    )
                })
                .fold(0.0_f32, f32::max);
            // Each leg needs to recover two steps' worth of body travel. Increase
            // cadence when that consumes the stride budget, and again when feet
            // already trail the body. The authored duration is the relaxed pace.
            let demand = (2.0 * speed * config.step_duration / stride).max(1.0) + lag / stride;
            let duration = (config.step_duration / demand)
                .clamp(config.step_duration.min(0.08), config.step_duration);

            // Spend only the time needed by each phase, carrying the remainder
            // through landing, support transfer and the next swing. Rounding each
            // phase up to a frame makes low-rate locomotion outrun the stride cap.
            if let Some(swing) = &mut self.swing {
                let rate = (swing.duration / duration).max(1.0);
                let until_landing = (swing.duration - swing.elapsed) / rate;
                if remaining < until_landing {
                    swing.elapsed += remaining * rate;
                    return self.goals();
                }
                remaining -= until_landing;
                self.contacts.as_mut().unwrap()[swing.side] = swing.to;
                self.next_side = Some(1 - swing.side);
                self.swing = None;
                self.transfer_seconds = 0.035 * duration / config.step_duration;
                continue;
            }
            let transfer = remaining.min(self.transfer_seconds);
            self.transfer_seconds -= transfer;
            remaining -= transfer;
            if self.transfer_seconds > 0.0 {
                return self.goals();
            }

            let errors = std::array::from_fn::<_, 2, _>(|i| {
                vec3_len(horizontal(vec3_sub(
                    rest[i].foot.sole,
                    contacts[i].foot.sole,
                )))
            });
            let turned = std::array::from_fn::<_, 2, _>(|i| {
                let dot = rest[i]
                    .foot
                    .rotation
                    .iter()
                    .zip(contacts[i].foot.rotation)
                    .map(|(a, b)| a * b)
                    .sum::<f32>()
                    .abs();
                dot < (25.0_f32.to_radians() * 0.5).cos()
            });
            // Anticipate continued motion so the first foot lifts before the body
            // has dragged both contacts behind it. Keep the displacement deadband
            // for idle tracking noise and corrective steps after stopping.
            let threshold = base_stride * 0.32;
            let anticipated_lag = lag + (speed * duration * 0.5).min(threshold);
            if errors.iter().all(|e| *e <= threshold)
                && anticipated_lag <= threshold
                && !turned.iter().any(|t| *t)
            {
                return self.goals();
            }
            let side = self
                .next_side
                .unwrap_or(if errors[1] > errors[0] { 1 } else { 0 });
            let lead = vec3_scale(
                vec3_add(
                    vec3_scale(self.velocity, duration * 1.5),
                    vec3_scale(direction, lag * 0.5),
                ),
                config.step_scale,
            );
            let lead_length = vec3_len(lead);
            let mut lead = vec3_scale(
                lead,
                if lead_length > 0.0 {
                    (stride * 0.8 / lead_length).min(1.0)
                } else {
                    0.0
                },
            );
            // Bound lateral prediction to avoid aiming a foot across the rest stance.
            let lateral = horizontal(vec3_sub(rest[1].foot.sole, rest[0].foot.sole));
            let width = vec3_len(lateral);
            if width > 1e-5 {
                let axis = vec3_scale(lateral, 1.0 / width);
                let amount = vec3_dot(lead, axis);
                lead = vec3_add(
                    lead,
                    vec3_scale(axis, amount.clamp(-width * 0.35, width * 0.35) - amount),
                );
            }
            let mut destination = vec3_add(rest[side].foot.sole, lead);
            let travel = horizontal(vec3_sub(destination, contacts[side].foot.sole));
            let length = vec3_len(travel);
            if length > stride {
                let bounded = vec3_add(
                    contacts[side].foot.sole,
                    vec3_scale(travel, stride / length),
                );
                destination[0] = bounded[0];
                destination[2] = bounded[2];
            }
            let Some(support) = ground.at(destination) else {
                return self.goals();
            };
            destination[1] = support.height;
            self.swing = Some(Swing {
                side,
                from: contacts[side].foot,
                to: Contact {
                    foot: FootGoal {
                        sole: destination,
                        rotation: rest[side].foot.rotation,
                    },
                    support,
                },
                elapsed: 0.0,
                duration,
                height: config.step_height * config.step_scale * scale,
                forward_bias: (speed * duration / stride).clamp(0.0, 1.0),
            });
            self.steps[side] += 1;
            if remaining <= 0.0 {
                return self.goals();
            }
        }
    }

    fn goals(&self) -> Option<[WalkingGoal; 2]> {
        let contacts = self.contacts?;
        Some(std::array::from_fn(|i| {
            if let Some(swing) = self.swing.as_ref().filter(|s| s.side == i) {
                WalkingGoal {
                    foot: swing.goal(),
                    support: swing.to.support,
                    phase: FootPhase::Swing,
                }
            } else {
                WalkingGoal {
                    foot: contacts[i].foot,
                    support: contacts[i].support,
                    phase: FootPhase::Planted,
                }
            }
        }))
    }
}
