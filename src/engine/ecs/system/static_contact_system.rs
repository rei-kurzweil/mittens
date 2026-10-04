use std::collections::{HashMap, HashSet};

use crate::engine::ecs::component::{
    CollidableComponent, CollidableMode, QueryRootMode, TransformComponent, ZoneComponent,
    resolve_component_ref,
};
use crate::engine::ecs::system::{TransformSystem, collision_geometry, zone_query};
use crate::engine::ecs::{ComponentId, World};
use crate::utils::math::{mat4_inverse, mat4_mul_vec4};

#[derive(Debug, Default)]
pub struct StaticContactSystem {
    previous_centers: HashMap<ComponentId, [f32; 3]>,
    pub candidate_pairs: u64,
    pub narrow_phase_tests: u64,
    pub correction_iterations: u64,
    pub non_convergences: u64,
}

impl StaticContactSystem {
    /// Resolve the first floor contact for each slide zone. Returns changed
    /// transform IDs so the caller can propagate them before dependent phases.
    pub fn tick(&mut self, world: &mut World) -> Vec<ComponentId> {
        self.candidate_pairs = 0;
        self.narrow_phase_tests = 0;
        self.correction_iterations = 0;
        let collidables: Vec<_> = world
            .all_components()
            .filter_map(|id| {
                let c = world.get_component_by_id_as::<CollidableComponent>(id)?;
                let zone = world.parent_of(id)?;
                (c.enabled
                    && world
                        .get_component_by_id_as::<ZoneComponent>(zone)
                        .is_some_and(|z| z.enabled))
                .then_some((id, zone, c.mode))
            })
            .collect();
        let live_movers: HashSet<_> = collidables
            .iter()
            .filter(|(_, _, mode)| *mode == CollidableMode::Slide)
            .map(|(_, zone, _)| *zone)
            .collect();
        self.previous_centers
            .retain(|zone, _| live_movers.contains(zone));
        let statics: Vec<_> = collidables
            .iter()
            .filter(|(_, _, mode)| *mode == CollidableMode::Static)
            .map(|(_, zone, _)| *zone)
            .collect();
        let mut changed = Vec::new();
        for (collidable_id, moving, mode) in collidables {
            if mode != CollidableMode::Slide {
                continue;
            }
            let Some((center, shape)) = zone_world_shape(world, moving) else {
                continue;
            };
            let previous = self
                .previous_centers
                .insert(moving, center)
                .unwrap_or(center);
            let Some(target) = movement_target(world, collidable_id, moving) else {
                continue;
            };
            for &surface in &statics {
                if surface == moving {
                    continue;
                }
                let Some((static_center, static_shape)) = zone_world_shape(world, surface) else {
                    continue;
                };
                let (moving_min, moving_max) = shape_aabb(center, shape);
                let (previous_min, previous_max) = shape_aabb(previous, shape);
                let (static_min, static_max) = shape_aabb(static_center, static_shape);
                self.candidate_pairs += 1;
                if !(0..3).all(|axis| {
                    moving_min[axis].min(previous_min[axis]) <= static_max[axis]
                        && static_min[axis] <= moving_max[axis].max(previous_max[axis])
                }) {
                    continue;
                }
                self.narrow_phase_tests += 1;
                let sweep = zone_query::sweep_capsule_floor(world, moving, surface, previous)
                    .ok()
                    .flatten();
                let displacement = if let Some(hit) = sweep {
                    let half_height = match shape {
                        crate::engine::ecs::component::CollisionShape::CapsuleY {
                            radius,
                            half_segment,
                        } => radius + half_segment,
                        _ => 0.0,
                    };
                    [0.0, (hit.point[1] + half_height - center[1]).max(0.0), 0.0]
                } else {
                    match zone_query::contact_zones(world, moving, surface) {
                        Ok(contact) => contact
                            .separation
                            .map(|s| s.displacement)
                            .unwrap_or([0.0; 3]),
                        Err(_) => {
                            self.non_convergences += 1;
                            continue;
                        }
                    }
                };
                if displacement.iter().all(|v| v.abs() < 1.0e-6) {
                    continue;
                }
                let Some(target_world) = TransformSystem::world_position(world, target) else {
                    continue;
                };
                let desired = [
                    target_world[0] + displacement[0],
                    target_world[1] + displacement[1],
                    target_world[2] + displacement[2],
                ];
                let local = world_to_local(world, target, desired);
                if let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(target) {
                    t.transform.translation = local;
                    t.transform.recompute_model();
                    changed.push(target);
                    self.correction_iterations += 1;
                    self.previous_centers.insert(
                        moving,
                        [
                            center[0] + displacement[0],
                            center[1] + displacement[1],
                            center[2] + displacement[2],
                        ],
                    );
                }
                // A later pass can handle corners after world transforms settle.
                break;
            }
        }
        changed
    }
}

fn movement_target(world: &World, id: ComponentId, zone: ComponentId) -> Option<ComponentId> {
    let c = world.get_component_by_id_as::<CollidableComponent>(id)?;
    let target = if let Some(source) = &c.movement_target_source {
        resolve_component_ref(world, source, Some(id), QueryRootMode::SelfSubtree)?
    } else if let Some(id) = c.movement_target_id {
        id
    } else if c.movement_target_required {
        return None;
    } else {
        let z = world.get_component_by_id_as::<ZoneComponent>(zone)?;
        zone_query::resolve_zone_frame(world, zone, z).ok()?
    };
    world
        .get_component_by_id_as::<TransformComponent>(target)
        .map(|_| target)
}

fn zone_world_shape(
    world: &World,
    id: ComponentId,
) -> Option<([f32; 3], crate::engine::ecs::component::CollisionShape)> {
    let zone = world.get_component_by_id_as::<ZoneComponent>(id)?;
    let frame = zone_query::resolve_zone_frame(world, id, zone).ok()?;
    collision_geometry::axis_aligned_world_shape(
        zone.shape,
        TransformSystem::world_model(world, frame)?,
    )
}

fn shape_aabb(
    center: [f32; 3],
    shape: crate::engine::ecs::component::CollisionShape,
) -> ([f32; 3], [f32; 3]) {
    use crate::engine::ecs::component::CollisionShape;
    let extent = match shape {
        CollisionShape::Cube { half_extents } => half_extents,
        CollisionShape::Sphere { radius } => [radius; 3],
        CollisionShape::CapsuleY {
            radius,
            half_segment,
        } => [radius, radius + half_segment, radius],
    };
    (
        std::array::from_fn(|i| center[i] - extent[i]),
        std::array::from_fn(|i| center[i] + extent[i]),
    )
}

fn world_to_local(world: &World, transform: ComponentId, desired: [f32; 3]) -> [f32; 3] {
    let mut current = transform;
    while let Some(parent) = world.parent_of(current) {
        if world
            .get_component_by_id_as::<TransformComponent>(parent)
            .is_some()
        {
            if let Some(inverse) =
                TransformSystem::world_model(world, parent).and_then(mat4_inverse)
            {
                let point = mat4_mul_vec4(inverse, [desired[0], desired[1], desired[2], 1.0]);
                return [point[0], point[1], point[2]];
            }
            break;
        }
        current = parent;
    }
    desired
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::CollisionShape;

    fn add_zone(
        world: &mut World,
        center: [f32; 3],
        shape: CollisionShape,
        mode: CollidableMode,
    ) -> (ComponentId, ComponentId) {
        let frame = world.add_component(
            TransformComponent::new().with_position(center[0], center[1], center[2]),
        );
        let transform = world
            .get_component_by_id_as_mut::<TransformComponent>(frame)
            .unwrap();
        transform.transform.matrix_world = transform.transform.model;
        let zone = world.add_component(ZoneComponent::new(shape));
        let collidable = world.add_component(match mode {
            CollidableMode::Static => CollidableComponent::static_(),
            CollidableMode::Slide => CollidableComponent::slide(),
        });
        world.add_child(frame, zone).unwrap();
        world.add_child(zone, collidable).unwrap();
        (frame, zone)
    }

    #[test]
    fn capsule_separates_from_floor_without_blocking_tangent_motion() {
        let mut world = World::default();
        let (_floor_frame, _floor) = add_zone(
            &mut world,
            [0.0, -0.05, 0.0],
            CollisionShape::cube_half_extents([5.0, 0.05, 5.0]),
            CollidableMode::Static,
        );
        let (mover, _zone) = add_zone(
            &mut world,
            [1.0, 0.7, 0.0],
            CollisionShape::capsule_y(0.25, 0.6),
            CollidableMode::Slide,
        );
        let changed = StaticContactSystem::default().tick(&mut world);
        assert_eq!(changed, vec![mover]);
        let local = world
            .get_component_by_id_as::<TransformComponent>(mover)
            .unwrap()
            .transform
            .translation;
        assert!((local[1] - 0.85).abs() < 1.0e-4);
        assert_eq!(local[0], 1.0);
    }

    #[test]
    fn capsule_crossing_thin_floor_is_caught() {
        let mut world = World::default();
        let (_floor_frame, _floor) = add_zone(
            &mut world,
            [0.0, -0.01, 0.0],
            CollisionShape::cube_half_extents([5.0, 0.01, 5.0]),
            CollidableMode::Static,
        );
        let (mover, _zone) = add_zone(
            &mut world,
            [0.0, 1.0, 0.0],
            CollisionShape::capsule_y(0.25, 0.6),
            CollidableMode::Slide,
        );
        let mut system = StaticContactSystem::default();
        assert!(system.tick(&mut world).is_empty());
        let transform = world
            .get_component_by_id_as_mut::<TransformComponent>(mover)
            .unwrap();
        transform.transform.translation[1] = -1.0;
        transform.transform.recompute_model();
        transform.transform.matrix_world = transform.transform.model;
        assert_eq!(system.tick(&mut world), vec![mover]);
        let local = world
            .get_component_by_id_as::<TransformComponent>(mover)
            .unwrap()
            .transform
            .translation;
        assert!((local[1] - 0.85).abs() < 1.0e-4);
    }

    #[test]
    fn offset_capsule_corrects_explicit_root() {
        let mut world = World::default();
        add_zone(
            &mut world,
            [0.0, -0.05, 0.0],
            CollisionShape::cube_half_extents([5.0, 0.05, 5.0]),
            CollidableMode::Static,
        );
        let root = world.add_component(TransformComponent::new());
        let (proxy, zone) = add_zone(
            &mut world,
            [0.0, 0.7, 0.0],
            CollisionShape::capsule_y(0.25, 0.6),
            CollidableMode::Slide,
        );
        world.add_child(root, proxy).unwrap();
        let collidable = world.children_of(zone)[0];
        *world
            .get_component_by_id_as_mut::<CollidableComponent>(collidable)
            .unwrap() = CollidableComponent::slide().with_runtime_movement_target(Some(root));
        assert_eq!(StaticContactSystem::default().tick(&mut world), vec![root]);
        assert!(
            (world
                .get_component_by_id_as::<TransformComponent>(root)
                .unwrap()
                .transform
                .translation[1]
                - 0.15)
                .abs()
                < 1.0e-4
        );
        assert_eq!(
            world
                .get_component_by_id_as::<TransformComponent>(proxy)
                .unwrap()
                .transform
                .translation[1],
            0.7
        );
    }
}
