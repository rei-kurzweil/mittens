use std::collections::{HashMap, HashSet};

use crate::engine::ecs::component::{
    CollidableComponent, CollidableMode, QueryRootMode, TransformComponent, VelocityComponent,
    ZoneComponent, resolve_component_ref,
};
use crate::engine::ecs::system::{TransformSystem, VelocitySystem, collision_geometry, zone_query};
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
    pub(crate) fn forget_target(&mut self, world: &World, target: ComponentId) {
        self.previous_centers.retain(|zone, _| {
            !world
                .children_of(*zone)
                .iter()
                .any(|id| movement_target(world, *id, *zone) == Some(target))
        });
    }

    /// Resolve static contacts for each slide zone with at most six passes. Returns changed
    /// transform IDs so the caller can propagate them before dependent phases.
    pub fn tick(&mut self, world: &mut World) -> Vec<ComponentId> {
        self.tick_excluding(world, &super::AttachmentSystem::default())
    }

    pub fn tick_excluding(
        &mut self,
        world: &mut World,
        mounts: &super::AttachmentSystem,
    ) -> Vec<ComponentId> {
        self.tick_contacts(world, mounts, false)
    }

    /// Friction consumes the normal velocity impulse from one fixed physics step.
    /// Pose-only contact passes do not consume additional friction budgets.
    pub fn tick_substep(
        &mut self,
        world: &mut World,
        mounts: &super::AttachmentSystem,
    ) -> Vec<ComponentId> {
        self.tick_contacts(world, mounts, true)
    }

    fn tick_contacts(
        &mut self,
        world: &mut World,
        mounts: &super::AttachmentSystem,
        apply_friction: bool,
    ) -> Vec<ComponentId> {
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
                .then_some((id, zone, c.mode, c.friction))
            })
            .collect();
        let live_movers: HashSet<_> = collidables
            .iter()
            .filter(|(_, _, mode, _)| *mode == CollidableMode::Slide)
            .map(|(_, zone, _, _)| *zone)
            .collect();
        self.previous_centers
            .retain(|zone, _| live_movers.contains(zone));
        let statics: Vec<_> = collidables
            .iter()
            .filter(|(_, _, mode, _)| *mode == CollidableMode::Static)
            .map(|(_, zone, _, friction)| (*zone, *friction))
            .collect();
        let mut changed = Vec::new();
        for (collidable_id, moving, mode, _) in collidables {
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
            if mounts.is_movement_root_mounted(target) {
                self.previous_centers.remove(&moving);
                continue;
            }
            let mut desired_center = center;
            for iteration in 0..6 {
                let mut corrected = false;
                for &(surface, friction) in &statics {
                    if surface == moving {
                        continue;
                    }
                    let Some((static_center, static_shape)) = surface_world_shape(world, surface)
                    else {
                        continue;
                    };
                    let (moving_min, moving_max) = shape_aabb(desired_center, shape);
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
                    let sweep = if iteration == 0 {
                        zone_query::sweep_capsule_floor(world, moving, surface, previous)
                            .ok()
                            .flatten()
                    } else {
                        None
                    };
                    let displacement = if let Some(hit) = sweep {
                        let half_height = match shape {
                            crate::engine::ecs::component::CollisionShape::CapsuleY {
                                radius,
                                half_segment,
                            } => radius + half_segment,
                            _ => 0.0,
                        };
                        [
                            0.0,
                            (hit.point[1] + half_height - desired_center[1]).max(0.0),
                            0.0,
                        ]
                    } else {
                        surface_translation(world, surface, desired_center, shape)
                            .unwrap_or([0.0; 3])
                    };
                    if displacement.iter().all(|v| v.abs() < 1.0e-6) {
                        continue;
                    }
                    resolve_contact_velocity(
                        world,
                        target,
                        displacement,
                        if apply_friction { friction } else { 0.0 },
                    );
                    for axis in 0..3 {
                        desired_center[axis] += displacement[axis];
                    }
                    corrected = true;
                    self.correction_iterations += 1;
                }
                if !corrected {
                    break;
                }
            }
            if statics.iter().any(|&(surface, _)| {
                surface != moving
                    && surface_world_shape(world, surface).is_some_and(|_| {
                        surface_translation(world, surface, desired_center, shape).is_some_and(
                            |displacement| displacement.iter().any(|v| v.abs() >= 1.0e-6),
                        )
                    })
            }) {
                self.non_convergences += 1;
            }
            let displacement: [f32; 3] = std::array::from_fn(|i| desired_center[i] - center[i]);
            if displacement.iter().all(|v| v.abs() < 1.0e-6) {
                continue;
            }
            let Some(target_world) = TransformSystem::world_position(world, target) else {
                continue;
            };
            let desired = std::array::from_fn(|i| target_world[i] + displacement[i]);
            let local = world_to_local(world, target, desired);
            if let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(target) {
                t.transform.translation = local;
                t.transform.recompute_model();
                changed.push(target);
                self.previous_centers.insert(moving, desired_center);
            }
        }
        changed
    }
}

/// Only the Velocity directly driving the corrected transform owns this
/// contact. An ancestor locomotion layer must not lose speed by proximity.
#[cfg(test)]
fn remove_inward_velocity(world: &mut World, target: ComponentId, normal: [f32; 3]) {
    resolve_contact_velocity(world, target, normal, 0.0);
}

fn resolve_contact_velocity(
    world: &mut World,
    target: ComponentId,
    normal: [f32; 3],
    friction: f32,
) {
    let Some(owner) = world.parent_of(target) else {
        return;
    };
    if VelocitySystem::driven_transform(world, owner).ok() != Some(target) {
        return;
    }
    let Some(velocity) = world.get_component_by_id_as::<VelocityComponent>(owner) else {
        return;
    };
    if !velocity.enabled {
        return;
    }
    let linear = velocity.linear_local_mps;
    let Ok(rotation) = VelocitySystem::parent_rotation(world, owner) else {
        return;
    };
    let length = crate::utils::math::vec3_len(normal);
    if !length.is_finite() || length < 1.0e-6 {
        return;
    }
    let normal = normal.map(|value| value / length);
    if normal[1] > 0.5 {
        world
            .get_component_by_id_as_mut::<VelocityComponent>(owner)
            .unwrap()
            .grounded = true;
    }
    let mut speed = crate::utils::math::quat_rotate_vec3(rotation, linear);
    let inward = speed
        .iter()
        .zip(normal)
        .map(|(v, n)| v * n)
        .sum::<f32>()
        .min(0.0);
    for axis in 0..3 {
        speed[axis] -= inward * normal[axis];
    }
    // Tangential Coulomb impulse: |delta v_t| <= mu * |delta v_n|.
    // This is surface contact friction, not global or airborne damping.
    if friction.is_finite() && friction > 0.0 && inward < 0.0 {
        let outward = speed.iter().zip(normal).map(|(v, n)| v * n).sum::<f32>();
        let tangent: [f32; 3] = std::array::from_fn(|i| speed[i] - outward * normal[i]);
        let tangent_speed = crate::utils::math::vec3_len(tangent);
        if tangent_speed > 0.0 {
            let reduction = (friction * -inward / tangent_speed).min(1.0);
            for i in 0..3 {
                speed[i] -= tangent[i] * reduction;
            }
        }
    }
    let local =
        crate::utils::math::quat_rotate_vec3(crate::utils::math::quat_conjugate(rotation), speed);
    let _ = world
        .get_component_by_id_as_mut::<VelocityComponent>(owner)
        .unwrap()
        .set_linear_local(local);
}

pub(crate) fn movement_target(
    world: &World,
    id: ComponentId,
    zone: ComponentId,
) -> Option<ComponentId> {
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

pub(crate) fn world_to_local(world: &World, transform: ComponentId, desired: [f32; 3]) -> [f32; 3] {
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

    fn propagate(world: &mut World, target: ComponentId) {
        use crate::engine::ecs::system::{
            CameraSystem, CollisionSystem, LightSystem, TransformStreamSystem,
        };
        TransformSystem::new().transform_changed(
            world,
            &mut crate::engine::graphics::VisualWorld::default(),
            target,
            &mut TransformStreamSystem::new(),
            &mut CameraSystem::new(),
            &mut LightSystem::new(),
            &mut CollisionSystem::new(),
        );
    }

    #[test]
    fn fixed_substeps_land_on_thin_floor_and_preserve_tangent_speed() {
        fn run(render_dt: f32, frames: usize, gravity: bool) -> ([f32; 3], [f32; 3]) {
            let mut world = World::default();
            add_zone(
                &mut world,
                [0.0, -0.01, 0.0],
                CollisionShape::cube_half_extents([5.0, 0.01, 5.0]),
                CollidableMode::Static,
            );
            let mut state = VelocityComponent::new();
            state
                .set_linear_local([1.0, if gravity { 0.0 } else { -120.0 }, 0.0])
                .unwrap();
            let owner = world.add_component(state);
            if gravity {
                let provider =
                    world.add_component(crate::engine::ecs::component::GravityComponent::new());
                world.add_child(provider, owner).unwrap();
            }
            let (target, _) = add_zone(
                &mut world,
                [0.0, 1.0, 0.0],
                CollisionShape::capsule_y(0.25, 0.6),
                CollidableMode::Slide,
            );
            world.add_child(owner, target).unwrap();
            propagate(&mut world, target);
            let mut contact = StaticContactSystem::default();
            contact.tick(&mut world);
            let mut velocity = VelocitySystem::default();
            let mut emit = crate::engine::ecs::RxWorld::default();
            for _ in 0..frames {
                for _ in 0..velocity.take_steps(render_dt) {
                    velocity.step(&mut world, &mut emit);
                    propagate(&mut world, target);
                    for changed in contact.tick(&mut world) {
                        propagate(&mut world, changed);
                    }
                }
            }
            (
                TransformSystem::world_position(&world, target).unwrap(),
                world
                    .get_component_by_id_as::<VelocityComponent>(owner)
                    .unwrap()
                    .linear_local_mps,
            )
        }
        let (position, speed) = run(1.0 / 60.0, 30, false);
        assert!((position[1] - 0.85).abs() < 1.0e-4);
        assert!((position[0] - 0.5).abs() < 1.0e-4);
        assert_eq!(speed, [1.0, 0.0, 0.0]);
        let (other_position, other_speed) = run(1.0 / 120.0, 60, false);
        for axis in 0..3 {
            assert!((position[axis] - other_position[axis]).abs() < 1.0e-5);
        }
        assert_eq!(speed, other_speed);
        let (fallen, resting) = run(1.0 / 60.0, 60, true);
        assert!((fallen[1] - 0.85).abs() < 1.0e-4);
        assert_eq!(resting, [1.0, 0.0, 0.0]);
        let (other, other_resting) = run(1.0 / 120.0, 120, true);
        for axis in 0..3 {
            assert!((fallen[axis] - other[axis]).abs() < 1.0e-5);
        }
        assert_eq!(resting, other_resting);
    }

    #[test]
    fn scaled_demo_cube_lands_and_falls_again_after_support_removal() {
        use crate::engine::ecs::component::GravityComponent;
        let mut world = World::default();
        let (floor_frame, floor_zone) = add_zone(
            &mut world,
            [0.0, -0.2, 0.0],
            CollisionShape::cube_half_extents([0.5; 3]),
            CollidableMode::Static,
        );
        world
            .get_component_by_id_as_mut::<TransformComponent>(floor_frame)
            .unwrap()
            .transform
            .scale = [10.0, 0.4, 10.0];
        let floor = world
            .get_component_by_id_as_mut::<TransformComponent>(floor_frame)
            .unwrap();
        floor.transform.recompute_model();
        propagate(&mut world, floor_frame);
        let gravity = world.add_component(GravityComponent::new().with_coefficient(0.5));
        let owner = world.add_component(VelocityComponent::new());
        let (target, _) = add_zone(
            &mut world,
            [0.0, 2.0, 0.0],
            CollisionShape::cube_half_extents([0.5; 3]),
            CollidableMode::Slide,
        );
        let cube = world
            .get_component_by_id_as_mut::<TransformComponent>(target)
            .unwrap();
        cube.transform.scale = [0.5; 3];
        cube.transform.recompute_model();
        world.add_child(gravity, owner).unwrap();
        world.add_child(owner, target).unwrap();
        propagate(&mut world, target);
        let mut contact = StaticContactSystem::default();
        contact.tick(&mut world);
        let mut velocity = VelocitySystem::default();
        let mut emit = crate::engine::ecs::RxWorld::default();
        for _ in 0..240 {
            velocity.step(&mut world, &mut emit);
            propagate(&mut world, target);
            for changed in contact.tick(&mut world) {
                propagate(&mut world, changed);
            }
        }
        assert!(
            (TransformSystem::world_position(&world, target).unwrap()[1] - 0.25).abs() < 1.0e-5
        );
        assert_eq!(
            world
                .get_component_by_id_as::<VelocityComponent>(owner)
                .unwrap()
                .linear_local_mps,
            [0.0; 3]
        );
        assert!(
            world
                .get_component_by_id_as::<VelocityComponent>(owner)
                .unwrap()
                .grounded
        );
        world
            .get_component_by_id_as_mut::<ZoneComponent>(floor_zone)
            .unwrap()
            .enabled = false;
        velocity.step(&mut world, &mut emit);
        propagate(&mut world, target);
        assert!(contact.tick(&mut world).is_empty());
        assert!(
            !world
                .get_component_by_id_as::<VelocityComponent>(owner)
                .unwrap()
                .grounded
        );
        assert!(TransformSystem::world_position(&world, target).unwrap()[1] < 0.25);
        assert!(
            world
                .get_component_by_id_as::<VelocityComponent>(owner)
                .unwrap()
                .linear_local_mps[1]
                < 0.0
        );
    }

    #[test]
    fn contact_never_clears_an_unrelated_ancestor_velocity() {
        let mut world = World::default();
        let mut state = VelocityComponent::new();
        state.set_linear_local([0.0, -3.0, 0.0]).unwrap();
        let owner = world.add_component(state);
        let driven = world.add_component(TransformComponent::new());
        let proxy = world.add_component(TransformComponent::new());
        world.add_child(owner, driven).unwrap();
        world.add_child(driven, proxy).unwrap();
        remove_inward_velocity(&mut world, proxy, [0.0, 1.0, 0.0]);
        assert_eq!(
            world
                .get_component_by_id_as::<VelocityComponent>(owner)
                .unwrap()
                .linear_local_mps,
            [0.0, -3.0, 0.0]
        );
        remove_inward_velocity(&mut world, driven, [0.0, 1.0, 0.0]);
        assert_eq!(
            world
                .get_component_by_id_as::<VelocityComponent>(owner)
                .unwrap()
                .linear_local_mps,
            [0.0; 3]
        );
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
    fn capsule_resolves_floor_and_box_side_in_one_step() {
        let mut world = World::default();
        add_zone(
            &mut world,
            [0.0, -0.05, 0.0],
            CollisionShape::cube_half_extents([5.0, 0.05, 5.0]),
            CollidableMode::Static,
        );
        add_zone(
            &mut world,
            [0.0, 0.4, 0.0],
            CollisionShape::cube_half_extents([0.425, 0.4, 0.425]),
            CollidableMode::Static,
        );
        let (mover, _) = add_zone(
            &mut world,
            [0.6, 0.8, 0.2],
            CollisionShape::capsule_y(0.28, 0.57),
            CollidableMode::Slide,
        );
        let mut system = StaticContactSystem::default();
        assert_eq!(system.tick(&mut world), vec![mover]);
        let position = world
            .get_component_by_id_as::<TransformComponent>(mover)
            .unwrap()
            .transform
            .translation;
        assert!((position[0] - 0.705).abs() < 1.0e-4);
        assert!((position[1] - 0.85).abs() < 1.0e-4);
        assert_eq!(position[2], 0.2);
        assert_eq!(system.non_convergences, 0);
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
    #[test]
    fn yawed_stage_has_real_box_contacts_and_catches_fast_capsule_falls() {
        let mut world = World::default();
        let (floor, _) = add_zone(
            &mut world,
            [0.0; 3],
            CollisionShape::cube_half_extents([2.0, 0.1, 0.3]),
            CollidableMode::Static,
        );
        let t = world
            .get_component_by_id_as_mut::<TransformComponent>(floor)
            .unwrap();
        t.transform.rotation = TransformComponent::new()
            .with_rotation_euler(0.0, std::f32::consts::FRAC_PI_4, 0.0)
            .transform
            .rotation;
        t.transform.recompute_model();
        t.transform.matrix_world = t.transform.model;
        let (target, _) = add_zone(
            &mut world,
            [1.0, 0.9, 1.0],
            CollisionShape::capsule_y(0.25, 0.75),
            CollidableMode::Slide,
        );
        let mut system = StaticContactSystem::default();
        assert!(
            system.tick(&mut world).is_empty(),
            "a corner of the world AABB is outside the yawed box"
        );
        let t = world
            .get_component_by_id_as_mut::<TransformComponent>(target)
            .unwrap();
        t.transform.translation = [1.0, 0.9, -1.0];
        t.transform.recompute_model();
        t.transform.matrix_world = t.transform.model;
        assert_eq!(system.tick(&mut world), vec![target]);
        assert!(
            (world
                .get_component_by_id_as::<TransformComponent>(target)
                .unwrap()
                .transform
                .translation[1]
                - 1.1)
                .abs()
                < 1.0e-5
        );
        let t = world
            .get_component_by_id_as_mut::<TransformComponent>(target)
            .unwrap();
        t.transform.translation[1] = -3.0;
        t.transform.recompute_model();
        t.transform.matrix_world = t.transform.model;
        assert_eq!(system.tick(&mut world), vec![target]);
        assert!(
            (world
                .get_component_by_id_as::<TransformComponent>(target)
                .unwrap()
                .transform
                .translation[1]
                - 1.1)
                .abs()
                < 1.0e-5
        );
    }
    #[test]
    fn authored_surface_friction_stops_edge_drift_at_the_same_fixed_step_across_render_rates() {
        fn run(mu: f32, render_dt: f32, frames: usize, with_floor: bool) -> ([f32; 3], [f32; 3]) {
            let mut world = World::default();
            if with_floor {
                let (_, floor_zone) = add_zone(
                    &mut world,
                    [0.0, -0.01, 0.0],
                    CollisionShape::cube_half_extents([20.0, 0.01, 20.0]),
                    CollidableMode::Static,
                );
                let surface = world.children_of(floor_zone)[0];
                let collidable = world
                    .get_component_by_id_as_mut::<CollidableComponent>(surface)
                    .unwrap();
                *collidable = collidable.clone().with_friction(mu).unwrap();
            }
            let owner = world.add_component(VelocityComponent::new());
            let gravity =
                world.add_component(crate::engine::ecs::component::GravityComponent::new());
            world.add_child(gravity, owner).unwrap();
            let (target, _) = add_zone(
                &mut world,
                [0.0, 1.0, 0.0],
                CollisionShape::capsule_y(0.25, 0.75),
                CollidableMode::Slide,
            );
            world.add_child(owner, target).unwrap();
            // Model the edge-contact projection implicated in the bug: an
            // oblique normal converts vertical falling speed into lateral speed.
            world
                .get_component_by_id_as_mut::<VelocityComponent>(owner)
                .unwrap()
                .set_linear_local([0.0, -4.0, 0.0])
                .unwrap();
            resolve_contact_velocity(&mut world, target, [1.0, 1.0, 0.0], 0.0);
            assert!(
                (world
                    .get_component_by_id_as::<VelocityComponent>(owner)
                    .unwrap()
                    .linear_local_mps[0]
                    - 2.0)
                    .abs()
                    < 1.0e-5
            );
            let mounts = super::super::AttachmentSystem::default();
            let mut contact = StaticContactSystem::default();
            let mut velocity = VelocitySystem::default();
            let mut emit = crate::engine::ecs::RxWorld::default();
            propagate(&mut world, target);
            for _ in 0..frames {
                for changed in contact.tick_excluding(&mut world, &mounts) {
                    propagate(&mut world, changed);
                }
                for _ in 0..velocity.take_steps(render_dt) {
                    velocity.step(&mut world, &mut emit);
                    propagate(&mut world, target);
                    for changed in contact.tick_substep(&mut world, &mounts) {
                        propagate(&mut world, changed);
                    }
                }
                for changed in contact.tick_excluding(&mut world, &mounts) {
                    propagate(&mut world, changed);
                }
            }
            (
                TransformSystem::world_position(&world, target).unwrap(),
                world
                    .get_component_by_id_as::<VelocityComponent>(owner)
                    .unwrap()
                    .linear_local_mps,
            )
        }
        let (position, speed) = run(0.8, 1.0 / 60.0, 60, true);
        assert!(
            speed.iter().all(|v| v.abs() < 1.0e-5),
            "friction must stop grounded drift: {speed:?}"
        );
        assert!(
            position[0] < 0.5,
            "drift should settle near the landing: {position:?}"
        );
        for (dt, frames) in [(1.0 / 120.0, 120), (1.0 / 240.0, 240)] {
            let (other, other_speed) = run(0.8, dt, frames, true);
            for axis in 0..3 {
                assert!((position[axis] - other[axis]).abs() < 1.0e-5);
            }
            assert_eq!(speed, other_speed);
        }
        let (_, speed) = run(0.0, 1.0 / 60.0, 60, true);
        assert!(
            (speed[0] - 2.0).abs() < 1.0e-5,
            "zero friction must retain intentional tangent speed"
        );
        let (_, speed) = run(0.8, 1.0 / 60.0, 60, false);
        assert!(
            (speed[0] - 2.0).abs() < 1.0e-5,
            "there is no hidden airborne drag"
        );
    }

    #[test]
    fn friction_uses_surface_normal_impulse_without_reversing_tangent_speed() {
        let mut world = World::default();
        let owner = world.add_component(VelocityComponent::new());
        let (target, _) = add_zone(
            &mut world,
            [0.0; 3],
            CollisionShape::capsule_y(0.25, 0.75),
            CollidableMode::Slide,
        );
        world.add_child(owner, target).unwrap();
        world
            .get_component_by_id_as_mut::<VelocityComponent>(owner)
            .unwrap()
            .set_linear_local([3.0, -2.0, 0.0])
            .unwrap();
        resolve_contact_velocity(&mut world, target, [0.0, 1.0, 0.0], 0.5);
        assert_eq!(
            world
                .get_component_by_id_as::<VelocityComponent>(owner)
                .unwrap()
                .linear_local_mps,
            [2.0, 0.0, 0.0]
        );
        world
            .get_component_by_id_as_mut::<VelocityComponent>(owner)
            .unwrap()
            .set_linear_local([0.1, -2.0, 0.0])
            .unwrap();
        resolve_contact_velocity(&mut world, target, [0.0, 1.0, 0.0], 0.5);
        assert_eq!(
            world
                .get_component_by_id_as::<VelocityComponent>(owner)
                .unwrap()
                .linear_local_mps,
            [0.0; 3]
        );
    }
}

fn surface_world_shape(
    world: &World,
    id: ComponentId,
) -> Option<([f32; 3], crate::engine::ecs::component::CollisionShape)> {
    if let Some(shape) = zone_world_shape(world, id) {
        return Some(shape);
    }
    let zone = world.get_component_by_id_as::<ZoneComponent>(id)?;
    let frame =
        TransformSystem::world_model(world, zone_query::resolve_zone_frame(world, id, zone).ok()?)?;
    collision_geometry::yaw_box(zone.shape, frame)?;
    let (min, max) = collision_geometry::transformed_aabb(zone.shape, frame);
    Some((
        std::array::from_fn(|i| (min[i] + max[i]) * 0.5),
        crate::engine::ecs::component::CollisionShape::cube_half_extents(std::array::from_fn(
            |i| (max[i] - min[i]) * 0.5,
        )),
    ))
}

fn surface_translation(
    world: &World,
    surface: ComponentId,
    center: [f32; 3],
    shape: crate::engine::ecs::component::CollisionShape,
) -> Option<[f32; 3]> {
    if let Some((position, surface_shape)) = zone_world_shape(world, surface) {
        return collision_geometry::minimum_translation(
            center,
            shape,
            position,
            surface_shape,
            0.0,
        );
    }
    // Upright capsules and spheres are invariant under yaw. Other moving
    // shapes need a general oriented contact solver, rather than AABB response.
    if !matches!(
        shape,
        crate::engine::ecs::component::CollisionShape::CapsuleY { .. }
            | crate::engine::ecs::component::CollisionShape::Sphere { .. }
    ) {
        return None;
    }
    let zone = world.get_component_by_id_as::<ZoneComponent>(surface)?;
    let frame = TransformSystem::world_model(
        world,
        zone_query::resolve_zone_frame(world, surface, zone).ok()?,
    )?;
    let (position, extents, axes) = collision_geometry::yaw_box(zone.shape, frame)?;
    let relative: [f32; 3] = std::array::from_fn(|i| center[i] - position[i]);
    let local = std::array::from_fn(|i| (0..3).map(|j| relative[j] * axes[i][j]).sum());
    let delta = collision_geometry::minimum_translation(
        local,
        shape,
        [0.0; 3],
        crate::engine::ecs::component::CollisionShape::cube_half_extents(extents),
        0.0,
    )?;
    Some(std::array::from_fn(|i| {
        (0..3).map(|j| delta[j] * axes[j][i]).sum()
    }))
}
