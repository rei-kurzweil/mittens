//! Blasts: the push that comes with a damage impact (`Damage.at(...)
//! .blast(radius, speed, spin)`), out to the blast radius. Each Velocity with
//! a Blastable child, whose driven transform a blast reaches, gets a push away
//! from the blast and a turn, through the same velocity intents scripts use
//! (`translate_world`, `rotate_world`). Runs each frame after the
//! destruction system has drained the impacts, like Integrity, and takes the
//! spin it gave back out over about a second. Debris
//! from the same impact is launched by the destruction system
//! (`debris_motion`).
use crate::engine::ecs::component::{BlastableComponent, DamageComponent, VelocityComponent};
use crate::engine::ecs::system::{TransformSystem, VelocitySystem};
use crate::engine::ecs::{ComponentId, IntentSignal, IntentValue, SignalEmitter, World};
use crate::utils::math::{
    quat_rotate_vec3, vec3_add, vec3_cross, vec3_len, vec3_normalize, vec3_scale, vec3_sub,
};
use std::time::Instant;

/// How much a push leans upward: added to the unit direction away from the
/// blast before normalising, so things on the ground lift a little.
const LIFT: f32 = 0.5;

/// The world-space change in linear and angular velocity a blast gives a
/// body at `center` facing `forward`, or None beyond its reach.
pub(crate) fn push(
    impact: &DamageComponent,
    body: &BlastableComponent,
    center: [f32; 3],
    forward: [f32; 3],
) -> Option<([f32; 3], [f32; 3])> {
    let blast = impact.blast?;
    if !body.enabled || !(body.mass > 0.) {
        return None;
    }
    let offset = vec3_sub(center, impact.position);
    let distance = vec3_len(offset);
    let strength = blast.falloff(distance);
    if strength <= 0. {
        return None;
    }
    let away = if distance > 1e-4 {
        vec3_scale(offset, 1. / distance)
    } else {
        [0., 1., 0.]
    };
    let direction = vec3_normalize([away[0], away[1] + LIFT, away[2]]);
    let linear = vec3_scale(direction, blast.speed * strength / body.mass);
    let turn = blast.spin * strength * body.spin / body.mass;
    let angular = if body.tumble {
        // Roll over away from the blast: about the horizontal axis across it.
        let axis = vec3_normalize(vec3_cross([0., 1., 0.], away));
        vec3_scale(axis, turn)
    } else {
        // Stay upright: yaw so the front swings away from the blast.
        let side = vec3_cross(forward, away)[1];
        let sign = if side >= 0. { 1. } else { -1. };
        [0., sign * turn, 0.]
    };
    Some((linear, angular))
}

/// How fast the spin a blast gave a body is taken back out (per second,
/// exponential): nothing else damps a parked vehicle's turning, while ground
/// friction already slows its sliding.
const SETTLE_PER_SEC: f32 = 1.5;

#[derive(Debug, Default)]
pub(crate) struct BlastSystem {
    /// Spin blasts gave each body that has not settled yet (world rad/s).
    turns: Vec<(ComponentId, [f32; 3])>,
    last: Option<Instant>,
}

impl BlastSystem {
    /// Called once per frame with that frame's impacts (often none).
    pub(crate) fn apply(
        &mut self,
        world: &World,
        impacts: &[DamageComponent],
        emit: &mut dyn SignalEmitter,
    ) {
        let now = Instant::now();
        let dt = self
            .last
            .map_or(0., |last| now.duration_since(last).as_secs_f32().min(0.1));
        self.last = Some(now);
        if dt > 0. && !self.turns.is_empty() {
            let fraction = 1. - (-SETTLE_PER_SEC * dt).exp();
            self.turns.retain_mut(|(velocity, remaining)| {
                if world
                    .get_component_by_id_as::<VelocityComponent>(*velocity)
                    .is_none()
                {
                    return false;
                }
                let settled = vec3_len(*remaining) < 0.02;
                let delta = if settled {
                    vec3_scale(*remaining, -1.)
                } else {
                    vec3_scale(*remaining, -fraction)
                };
                *remaining = vec3_add(*remaining, delta);
                rotate(emit, *velocity, delta);
                !settled
            });
        }
        if !impacts.iter().any(|impact| impact.blast.is_some()) {
            return;
        }
        let blastables: Vec<(ComponentId, BlastableComponent)> = world
            .all_components()
            .filter_map(|id| {
                let blastable = *world.get_component_by_id_as::<BlastableComponent>(id)?;
                let velocity = world.parent_of(id)?;
                world.get_component_by_id_as::<VelocityComponent>(velocity)?;
                Some((velocity, blastable))
            })
            .collect();
        for (velocity, blastable) in blastables {
            let Some(driven) = VelocitySystem::driven_transform(world, velocity).ok() else {
                continue;
            };
            let Some(trs) = TransformSystem::world_trs(world, driven).ok() else {
                continue;
            };
            let forward = quat_rotate_vec3(trs.rotation_quat_xyzw, [0., 0., -1.]);
            for impact in impacts {
                let Some((linear, angular)) = push(impact, &blastable, trs.translation, forward)
                else {
                    continue;
                };
                emit.push_intent(
                    velocity,
                    IntentSignal::now(IntentValue::VelocityTranslate {
                        component_id: velocity,
                        delta_mps: linear,
                        world_space: true,
                    }),
                );
                rotate(emit, velocity, angular);
                match self.turns.iter_mut().find(|(id, _)| *id == velocity) {
                    Some((_, remaining)) => *remaining = vec3_add(*remaining, angular),
                    None => self.turns.push((velocity, angular)),
                }
            }
        }
    }
}

fn rotate(emit: &mut dyn SignalEmitter, velocity: ComponentId, delta_radps: [f32; 3]) {
    emit.push_intent(
        velocity,
        IntentSignal::now(IntentValue::VelocityRotate {
            component_id: velocity,
            delta_radps,
            world_space: true,
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::Blast;

    fn rocket(position: [f32; 3]) -> DamageComponent {
        DamageComponent {
            position,
            radius: 1.5,
            amount: 1.0,
            blast: Some(Blast::new(6.0, 9.0, 8.0).unwrap()),
        }
    }

    #[test]
    fn a_blast_pushes_away_and_up_weaker_with_distance_and_mass() {
        let body = BlastableComponent { mass: 2.0, ..Default::default() };
        let forward = [0., 0., -1.];
        let (near, _) = push(&rocket([-1., 0., 0.]), &body, [0.; 3], forward).unwrap();
        let (far, _) = push(&rocket([-4., 0., 0.]), &body, [0.; 3], forward).unwrap();
        assert!(near[0] > 0. && near[1] > 0., "{near:?}");
        assert!(vec3_len(near) > vec3_len(far), "{near:?} {far:?}");
        assert!(push(&rocket([-7., 0., 0.]), &body, [0.; 3], forward).is_none());
        let light = BlastableComponent { mass: 1.0, ..Default::default() };
        let (lighter, _) = push(&rocket([-1., 0., 0.]), &light, [0.; 3], forward).unwrap();
        assert!((vec3_len(lighter) - 2. * vec3_len(near)).abs() < 1e-4);
    }

    #[test]
    fn upright_bodies_only_yaw_and_a_pistol_hit_pushes_nothing() {
        let body = BlastableComponent::default();
        let (_, angular) = push(&rocket([-1., 0., 0.]), &body, [0.; 3], [0., 0., -1.]).unwrap();
        assert!(angular[0] == 0. && angular[2] == 0. && angular[1] != 0.);
        let tumbling = BlastableComponent { tumble: true, ..Default::default() };
        let (_, roll) = push(&rocket([-1., 0., 0.]), &tumbling, [0.; 3], [0., 0., -1.]).unwrap();
        assert!(roll[1].abs() < 1e-6 && roll[2] != 0.);
        let pistol = DamageComponent { blast: None, ..rocket([-1., 0., 0.]) };
        assert!(push(&pistol, &body, [0.; 3], [0., 0., -1.]).is_none());
    }
}
