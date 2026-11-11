//! Fixed-step acceleration and sphere projectile impacts; no script polling.
use super::{TransformSystem, collidable_bvh::CollidableBvh, zone_query::ZoneSweepHit};
use crate::engine::ecs::component::{
    AccelerationComponent, CollisionShape, ProjectileImpactComponent, VelocityComponent,
    ZoneComponent,
};
use crate::engine::ecs::{ComponentId, World};

pub(super) fn accelerate(world: &World, velocity: ComponentId, linear: &mut [f32; 3], dt: f32) {
    let mut cursor = world.parent_of(velocity);
    while let Some(id) = cursor {
        if world
            .get_component_by_id_as::<VelocityComponent>(id)
            .is_some()
        {
            break;
        }
        if let Some(a) = world.get_component_by_id_as::<AccelerationComponent>(id) {
            let direction_length = a.local.iter().map(|v| v * v).sum::<f32>().sqrt();
            if direction_length > 0.0 {
                let direction = a.local.map(|v| v / direction_length);
                let along = (0..3).map(|i| linear[i] * direction[i]).sum::<f32>();
                let addition = (direction_length * dt).min((a.max_speed - along).max(0.0));
                for i in 0..3 {
                    linear[i] += direction[i] * addition;
                }
            }
        }
        cursor = world.parent_of(id);
    }
}

#[derive(Debug, Default)]
pub(super) struct ProjectileMotion {
    index: CollidableBvh,
    stack: Vec<usize>,
    candidates: Vec<usize>,
}
impl ProjectileMotion {
    pub fn sweep(
        &mut self,
        world: &World,
        target: ComponentId,
        delta: [f32; 3],
    ) -> Option<(ComponentId, ZoneSweepHit)> {
        let zone = world.children_of(target).iter().copied().find(|&id| {
            world.get_component_by_id_as::<ZoneComponent>(id).is_some()
                && world.children_of(id).iter().any(|&child| {
                    world
                        .get_component_by_id_as::<ProjectileImpactComponent>(child)
                        .is_some()
                })
        })?;
        let definition = world.get_component_by_id_as::<ZoneComponent>(zone)?;
        if !definition.enabled {
            return None;
        }
        let CollisionShape::Sphere { radius } = definition.shape else {
            return None;
        };
        let frame = super::zone_query::resolve_zone_frame(world, zone, definition).ok()?;
        let matrix = TransformSystem::world_model(world, frame)?;
        let start = TransformSystem::world_position(world, frame)?;
        let scale = (0..3)
            .map(|axis| {
                (0..3)
                    .map(|i| matrix[axis][i] * matrix[axis][i])
                    .sum::<f32>()
                    .sqrt()
            })
            .fold(0.0f32, f32::max);
        let radius = radius * scale;
        let end = std::array::from_fn(|i| start[i] + delta[i]);
        self.index.sync(world);
        self.index.query(
            std::array::from_fn(|i| start[i].min(end[i]) - radius),
            std::array::from_fn(|i| start[i].max(end[i]) + radius),
            &mut self.stack,
            &mut self.candidates,
        );
        let mut best: Option<ZoneSweepHit> = None;
        for &index in &self.candidates {
            let surface = self.index.surface(index);
            let mut cursor = Some(surface.zone);
            let mut own = false;
            while let Some(id) = cursor {
                if id == target {
                    own = true;
                    break;
                }
                cursor = world.parent_of(id);
            }
            if own {
                continue;
            }
            if let Some(hit) = surface.sphere_sweep(zone, start, end, radius) {
                if best.as_ref().is_none_or(|b| hit.fraction < b.fraction) {
                    best = Some(hit);
                }
            }
        }
        best.map(|hit| (zone, hit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{CollidableComponent, TransformComponent};
    fn wall(world: &mut World, z: f32) -> ComponentId {
        let frame = world.add_component(TransformComponent::new().with_position(0.0, 0.0, z));
        let t = world
            .get_component_by_id_as_mut::<TransformComponent>(frame)
            .unwrap();
        t.transform.matrix_world = t.transform.model;
        let zone = world.add_component(ZoneComponent::cube([1.0, 1.0, 0.01]));
        let solid = world.add_component(CollidableComponent::static_());
        world.add_child(frame, zone).unwrap();
        world.add_child(zone, solid).unwrap();
        zone
    }
    #[test]
    fn rocket_native_sphere_sweep_finds_nearest_thin_wall_initial_overlap_and_miss() {
        let mut world = World::default();
        let target = world.add_component(TransformComponent::new());
        let zone = world.add_component(ZoneComponent::sphere(0.05));
        let marker = world.add_component(ProjectileImpactComponent);
        world.add_child(target, zone).unwrap();
        world.add_child(zone, marker).unwrap();
        wall(&mut world, -4.0);
        let near = wall(&mut world, -2.0);
        let mut motion = ProjectileMotion::default();
        let (_, hit) = motion.sweep(&world, target, [0.0, 0.0, -10.0]).unwrap();
        assert_eq!(hit.surface, near);
        assert!((hit.fraction - 0.194).abs() < 1e-4, "{hit:?}");
        assert!(hit.normal[2] > 0.99);
        assert!(motion.sweep(&world, target, [10.0, 0.0, 0.0]).is_none());
        wall(&mut world, 0.0);
        assert_eq!(
            motion
                .sweep(&world, target, [0.0, 0.0, -10.0])
                .unwrap()
                .1
                .fraction,
            0.0
        );
    }
    #[test]
    fn rocket_native_acceleration_caps_projection_and_respects_nested_velocity_boundary() {
        let mut world = World::default();
        let provider = world.add_component(AccelerationComponent {
            local: [0.0, 0.0, -12.0],
            max_speed: 18.0,
        });
        let velocity = world.add_component(VelocityComponent::new());
        let nested = world.add_component(VelocityComponent::new());
        world.add_child(provider, velocity).unwrap();
        world.add_child(velocity, nested).unwrap();
        let mut speed = [3.0, 0.0, -17.99];
        accelerate(&world, velocity, &mut speed, 1.0);
        assert_eq!(speed, [3.0, 0.0, -18.0]);
        accelerate(&world, nested, &mut speed, 1.0);
        assert_eq!(speed, [3.0, 0.0, -18.0]);
        let mut zero = [0.0; 3];
        accelerate(&world, nested, &mut zero, 1.0);
        assert_eq!(zero, [0.0; 3]);
    }
}
