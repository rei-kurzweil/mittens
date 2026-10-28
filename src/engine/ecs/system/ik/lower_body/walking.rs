//! Stateful foot placement. Produces goals; never writes joints or locomotion.
use super::{
    FootGoal, FootPhase, WalkingDiagnostics,
    crouch::RestGoal,
    support::{Support, SupportQuery},
};
use crate::engine::ecs::component::AvatarControlComponent;
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
}

impl Swing {
    fn goal(&self) -> FootGoal {
        let u = (self.elapsed / self.duration).clamp(0.0, 1.0);
        // Quintic easing and a clearance bump have zero endpoint velocity.
        let t = u * u * u * (10.0 + u * (-15.0 + 6.0 * u));
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
        config: &AvatarControlComponent,
        dt: f32,
        ground: &SupportQuery<'_>,
    ) -> Option<[WalkingGoal; 2]> {
        if !dt.is_finite() || dt <= 0.0 || dt > 0.25 {
            self.reset();
            return None;
        }
        let stride = config.walking_stride * scale;
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

        // Completing an airborne step depends on time, including after a stop.
        if let Some(swing) = &mut self.swing {
            swing.elapsed += dt;
            if swing.elapsed >= swing.duration {
                self.contacts.as_mut().unwrap()[swing.side] = swing.to;
                self.next_side = Some(1 - swing.side);
                self.swing = None;
                self.transfer_seconds = 0.035;
            }
            return self.goals();
        }
        self.transfer_seconds = (self.transfer_seconds - dt).max(0.0);
        if self.transfer_seconds > 0.0 {
            return self.goals();
        }

        let contacts = self.contacts.unwrap();
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
        // A displacement deadband and the transfer interval suppress tiny
        // tracking fluctuations without advancing an idle gait clock.
        let threshold = stride * 0.32;
        if errors.iter().all(|e| *e <= threshold) && !turned.iter().any(|t| *t) {
            return self.goals();
        }
        let side = self
            .next_side
            .unwrap_or(if errors[1] > errors[0] { 1 } else { 0 });
        let duration = config.walking_step_duration;
        let lead = vec3_scale(self.velocity, duration * 1.5);
        let lead_length = vec3_len(lead);
        let mut lead = vec3_scale(
            lead,
            if lead_length > 0.0 {
                (stride * 0.5 / lead_length).min(1.0)
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
            height: config.walking_step_height * scale,
        });
        self.steps[side] += 1;
        self.goals()
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
