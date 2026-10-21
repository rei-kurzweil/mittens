//! Retained static-zone broad phase. Updated synchronously before contact queries.
use super::{TransformSystem, collision_geometry, spatial_bvh, zone_query};
use crate::engine::ecs::component::{
    CollidableComponent, CollidableMode, CollisionShape, ZoneComponent,
};
use crate::engine::ecs::{ComponentId, World};
use bvh::Point3;
use bvh::aabb::{AABB, Bounded};
use bvh::bounding_hierarchy::BHShape;
use bvh::bvh::BVH;
use std::collections::{BTreeMap, HashMap, HashSet};

type Frame = [[f32; 4]; 4];
type AxisShape = ([f32; 3], CollisionShape);
type YawBox = ([f32; 3], [f32; 3], [[f32; 3]; 3]);

#[derive(Debug, Clone)]
pub(crate) struct Surface {
    pub collidable: ComponentId,
    pub zone: ComponentId,
    pub frame_id: ComponentId,
    pub friction: f32,
    shape: CollisionShape,
    frame: Frame,
    axis_shape: Option<AxisShape>,
    yaw_box: Option<YawBox>,
    bounds: AABB,
    node: usize,
}
impl Bounded for Surface {
    fn aabb(&self) -> AABB {
        self.bounds
    }
}
impl BHShape for Surface {
    fn set_bh_node_index(&mut self, index: usize) {
        self.node = index;
    }
    fn bh_node_index(&self) -> usize {
        self.node
    }
}
impl Surface {
    /// Horizontal top face containing this X/Z point. Broad-phase bounds alone
    /// are insufficient, particularly for yawed boxes and stacked geometry.
    pub fn flat_support(
        &self,
        sole: [f32; 3],
        capsule_center: [f32; 3],
        bottom: f32,
        margin: f32,
    ) -> Option<f32> {
        let (center, extents, axes) = self.yaw_box?;
        let height = center[1] + extents[1];
        if center[1] >= capsule_center[1] || (height - bottom).abs() > margin {
            return None;
        }
        let relative = [sole[0] - center[0], 0.0, sole[2] - center[2]];
        for axis in [0, 2] {
            let coordinate: f32 = (0..3).map(|i| relative[i] * axes[axis][i]).sum();
            if coordinate.abs() > extents[axis] + 1e-5 {
                return None;
            }
        }
        Some(height)
    }

    pub fn translation(&self, center: [f32; 3], shape: CollisionShape) -> Option<[f32; 3]> {
        if let Some((position, surface_shape)) = self.axis_shape {
            return collision_geometry::minimum_translation(
                center,
                shape,
                position,
                surface_shape,
                0.0,
            );
        }
        if !matches!(
            shape,
            CollisionShape::CapsuleY { .. } | CollisionShape::Sphere { .. }
        ) {
            return None;
        }
        let (position, extents, axes) = self.yaw_box?;
        let relative: [f32; 3] = std::array::from_fn(|i| center[i] - position[i]);
        let local = std::array::from_fn(|i| (0..3).map(|j| relative[j] * axes[i][j]).sum());
        let delta = collision_geometry::minimum_translation(
            local,
            shape,
            [0.0; 3],
            CollisionShape::cube_half_extents(extents),
            0.0,
        )?;
        Some(std::array::from_fn(|i| {
            (0..3).map(|j| delta[j] * axes[j][i]).sum()
        }))
    }
    pub fn floor_sweep(
        &self,
        moving: ComponentId,
        end: [f32; 3],
        shape: CollisionShape,
        previous: [f32; 3],
    ) -> Option<zone_query::ZoneSweepHit> {
        zone_query::sweep_capsule_floor_geometry(
            moving, self.zone, end, shape, self.shape, self.frame, previous,
        )
        .ok()
        .flatten()
    }
}

#[derive(Debug, Default)]
pub(crate) struct CollidableBvh {
    surfaces: Vec<Surface>,
    indices: HashMap<ComponentId, usize>,
    bvh: Option<BVH>,
    /// Register dependencies even for disabled/unresolved collidables, so edits can activate them.
    dependencies: HashMap<ComponentId, Vec<ComponentId>>,
    dependents: HashMap<ComponentId, HashSet<ComponentId>>,
    pub movers: BTreeMap<ComponentId, ComponentId>,
    revision: Option<u64>,
    pub rebuilds: u64,
    pub refits: u64,
    pub refreshed: u64,
}
impl CollidableBvh {
    pub fn len(&self) -> usize {
        self.surfaces.len()
    }
    pub fn surface(&self, index: usize) -> &Surface {
        &self.surfaces[index]
    }
    pub fn sync(&mut self, world: &World) {
        let dirty = world.take_contact_changes();
        let mut affected = HashSet::new();
        if self.revision != Some(world.query_revision()) {
            affected.extend(world.collidable_ids());
            affected.extend(self.dependencies.keys().copied());
            self.revision = Some(world.query_revision());
        } else {
            for id in dirty {
                if let Some(dependents) = self.dependents.get(&id) {
                    affected.extend(dependents);
                }
                if world
                    .get_component_by_id_as::<CollidableComponent>(id)
                    .is_some()
                {
                    affected.insert(id);
                }
            }
        }
        let mut rebuild = false;
        let mut changed = HashSet::new();
        // Stable insertion/order independent of hash iteration.
        let mut affected: Vec<_> = affected.into_iter().collect();
        affected.sort();
        for id in affected {
            self.refreshed += 1;
            let previous = self.dependencies.remove(&id).unwrap_or_default();
            for dependency in previous {
                if let Some(ids) = self.dependents.get_mut(&dependency) {
                    ids.remove(&id);
                    if ids.is_empty() {
                        self.dependents.remove(&dependency);
                    }
                }
            }
            self.movers.remove(&id);
            let mut dependencies = vec![id];
            let resolved = world
                .get_component_by_id_as::<CollidableComponent>(id)
                .and_then(|c| {
                    let zone_id = world.parent_of(id)?;
                    dependencies.push(zone_id);
                    let z = world.get_component_by_id_as::<ZoneComponent>(zone_id)?;
                    let frame_id = zone_query::resolve_zone_frame(world, zone_id, z).ok();
                    if let Some(frame) = frame_id {
                        dependencies.push(frame);
                    }
                    if !c.enabled || !z.enabled {
                        return None;
                    }
                    if c.mode == CollidableMode::Slide {
                        self.movers.insert(id, zone_id);
                        return None;
                    }
                    Some((
                        zone_id,
                        frame_id?,
                        z.shape,
                        TransformSystem::world_model(world, frame_id?)?,
                        c.friction,
                    ))
                });
            if world
                .get_component_by_id_as::<CollidableComponent>(id)
                .is_some()
            {
                for &dependency in &dependencies {
                    self.dependents.entry(dependency).or_default().insert(id);
                }
                self.dependencies.insert(id, dependencies);
            }
            let Some((zone, frame_id, shape, frame, friction)) = resolved else {
                if let Some(index) = self.indices.remove(&id) {
                    self.surfaces.swap_remove(index);
                    if let Some(swapped) = self.surfaces.get(index) {
                        self.indices.insert(swapped.collidable, index);
                    }
                    rebuild = true;
                }
                continue;
            };
            let existing = self.indices.get(&id).copied();
            if existing.is_some_and(|i| {
                let s = &self.surfaces[i];
                s.zone == zone
                    && s.frame_id == frame_id
                    && s.shape == shape
                    && s.frame == frame
                    && s.friction == friction
            }) {
                continue;
            }
            let (min, max) = collision_geometry::transformed_aabb(shape, frame);
            if !min.into_iter().chain(max).all(f32::is_finite) {
                if let Some(index) = self.indices.remove(&id) {
                    self.surfaces.swap_remove(index);
                    if let Some(swapped) = self.surfaces.get(index) {
                        self.indices.insert(swapped.collidable, index);
                    }
                    rebuild = true;
                }
                continue;
            }
            let surface = Surface {
                collidable: id,
                zone,
                frame_id,
                shape,
                frame,
                friction,
                axis_shape: collision_geometry::axis_aligned_world_shape(shape, frame),
                yaw_box: collision_geometry::yaw_box(shape, frame),
                bounds: bounds(min, max),
                node: existing.map(|i| self.surfaces[i].node).unwrap_or(0),
            };
            if let Some(index) = existing {
                self.surfaces[index] = surface;
                changed.insert(index);
            } else {
                self.indices.insert(id, self.surfaces.len());
                self.surfaces.push(surface);
                rebuild = true;
            }
        }
        if rebuild {
            self.bvh = if self.surfaces.is_empty() {
                None
            } else {
                Some(BVH::build(&mut self.surfaces))
            };
            self.rebuilds += 1;
        } else if !changed.is_empty() {
            if let Some(bvh) = &mut self.bvh {
                spatial_bvh::refit(bvh, &self.surfaces, &changed);
            }
            self.refits += 1;
        }
    }
    /// Neighborhood query only: exact support/contact remains consumer-owned.
    pub fn query(
        &self,
        min: [f32; 3],
        max: [f32; 3],
        stack: &mut Vec<usize>,
        out: &mut Vec<usize>,
    ) -> u64 {
        out.clear();
        let Some(bvh) = &self.bvh else {
            return 0;
        };
        let visits = spatial_bvh::query(bvh, &self.surfaces, &bounds(min, max), stack, out);
        out.sort_unstable_by_key(|&i| self.surfaces[i].collidable);
        visits
    }
}
fn bounds(min: [f32; 3], max: [f32; 3]) -> AABB {
    AABB::with_bounds(
        Point3::new(min[0], min[1], min[2]),
        Point3::new(max[0], max[1], max[2]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{ComponentRef, TransformComponent};
    use crate::engine::ecs::system::{CameraSystem, LightSystem, TransformOperatorSystem};
    use crate::engine::graphics::VisualWorld;

    fn surface(world: &mut World, x: f32) -> (ComponentId, ComponentId, ComponentId) {
        let frame = world.add_component(TransformComponent::new().with_position(x, 0.0, 0.0));
        let zone = world.add_component(ZoneComponent::cube([0.5; 3]));
        let collider = world.add_component(CollidableComponent::static_());
        world.add_child(frame, zone).unwrap();
        world.add_child(zone, collider).unwrap();
        propagate(world, frame);
        (frame, zone, collider)
    }
    fn propagate(world: &mut World, frame: ComponentId) {
        world
            .get_component_by_id_as_mut::<TransformComponent>(frame)
            .unwrap()
            .transform
            .recompute_model();
        TransformSystem::new().transform_changed(
            world,
            &mut VisualWorld::default(),
            frame,
            &mut TransformOperatorSystem::new(),
            &mut CameraSystem::new(),
            &mut LightSystem::new(),
        );
    }
    fn neighborhood(index: &CollidableBvh, x: f32) -> Vec<ComponentId> {
        let mut result = Vec::new();
        index.query(
            [x - 0.1, -0.1, -0.1],
            [x + 0.1, 0.1, 0.1],
            &mut Vec::new(),
            &mut result,
        );
        result.into_iter().map(|i| index.surface(i).zone).collect()
    }
    #[test]
    fn lifecycle_updates_registry_bounds_and_reuses_unchanged_tree() {
        let mut world = World::default();
        let (frame, zone, collider) = surface(&mut world, 0.0);
        let mut index = CollidableBvh::default();
        index.sync(&world);
        assert_eq!(neighborhood(&index, 0.0), vec![zone]);
        let rebuilds = index.rebuilds;
        let refreshed = index.refreshed;
        index.sync(&world);
        assert_eq!(
            index.refreshed, refreshed,
            "unchanged index performs no geometry work"
        );
        world
            .get_component_by_id_as_mut::<TransformComponent>(frame)
            .unwrap()
            .transform
            .translation[0] = 10.0;
        propagate(&mut world, frame);
        index.sync(&world);
        assert!(neighborhood(&index, 0.0).is_empty());
        assert_eq!(neighborhood(&index, 10.0), vec![zone]);
        assert_eq!(index.rebuilds, rebuilds);
        assert_eq!(index.refits, 1);
        world
            .get_component_by_id_as_mut::<ZoneComponent>(zone)
            .unwrap()
            .shape = CollisionShape::cube_half_extents([3.0; 3]);
        index.sync(&world);
        assert_eq!(neighborhood(&index, 12.0), vec![zone]);
        world
            .get_component_by_id_as_mut::<CollidableComponent>(collider)
            .unwrap()
            .enabled = false;
        index.sync(&world);
        assert!(neighborhood(&index, 10.0).is_empty());
        world
            .get_component_by_id_as_mut::<CollidableComponent>(collider)
            .unwrap()
            .enabled = true;
        index.sync(&world);
        assert_eq!(neighborhood(&index, 10.0), vec![zone]);
        world
            .get_component_by_id_as_mut::<ZoneComponent>(zone)
            .unwrap()
            .enabled = false;
        index.sync(&world);
        assert_eq!(index.len(), 0);
        world
            .get_component_by_id_as_mut::<ZoneComponent>(zone)
            .unwrap()
            .enabled = true;
        index.sync(&world);
        world
            .get_component_by_id_as_mut::<CollidableComponent>(collider)
            .unwrap()
            .mode = CollidableMode::Slide;
        index.sync(&world);
        assert_eq!(index.len(), 0);
        assert_eq!(index.movers.get(&collider), Some(&zone));
        world.remove_component_subtree(frame).unwrap();
        index.sync(&world);
        assert!(index.movers.is_empty());
        assert!(world.collidable_ids().next().is_none());
        let (_, new_zone, _) = surface(&mut world, 0.0);
        index.sync(&world);
        assert_eq!(neighborhood(&index, 0.0), vec![new_zone]);
    }
    #[test]
    fn ancestor_reparenting_and_referenced_frames_refresh_at_current_pose() {
        let mut world = World::default();
        let (frame, zone, _) = surface(&mut world, 0.0);
        let parent = world.add_component(TransformComponent::new().with_position(20.0, 0.0, 0.0));
        let reference =
            world.add_component(TransformComponent::new().with_position(40.0, 0.0, 0.0));
        propagate(&mut world, reference);
        let mut index = CollidableBvh::default();
        index.sync(&world);
        world.add_child(parent, frame).unwrap();
        propagate(&mut world, parent);
        index.sync(&world);
        assert!(neighborhood(&index, 0.0).is_empty());
        assert_eq!(neighborhood(&index, 20.0), vec![zone]);
        let guid = world.get_component_record(reference).unwrap().guid;
        world
            .get_component_by_id_as_mut::<ZoneComponent>(zone)
            .unwrap()
            .frame_source = Some(ComponentRef::Guid(guid));
        index.sync(&world);
        assert!(neighborhood(&index, 20.0).is_empty());
        assert_eq!(neighborhood(&index, 40.0), vec![zone]);
        world
            .get_component_by_id_as_mut::<TransformComponent>(reference)
            .unwrap()
            .transform
            .translation[0] = 60.0;
        propagate(&mut world, reference);
        index.sync(&world);
        assert!(neighborhood(&index, 40.0).is_empty());
        assert_eq!(neighborhood(&index, 60.0), vec![zone]);
        world.remove_component_subtree(reference).unwrap();
        index.sync(&world);
        assert!(neighborhood(&index, 60.0).is_empty());
        world
            .get_component_by_id_as_mut::<ZoneComponent>(zone)
            .unwrap()
            .frame_source = None;
        index.sync(&world);
        assert_eq!(neighborhood(&index, 20.0), vec![zone]);
    }
    #[test]
    fn neighborhood_matches_brute_force_with_large_and_rotated_bounds() {
        let mut world = World::default();
        for i in 0..256 {
            surface(&mut world, i as f32 * 3.0);
        }
        let (large, zone, _) = surface(&mut world, -100.0);
        world
            .get_component_by_id_as_mut::<ZoneComponent>(zone)
            .unwrap()
            .shape = CollisionShape::cube_half_extents([200.0, 0.2, 2.0]);
        world
            .get_component_by_id_as_mut::<TransformComponent>(large)
            .unwrap()
            .transform
            .rotation = crate::utils::math::quat_from_axis_angle([0.0, 1.0, 0.0], 0.2);
        propagate(&mut world, large);
        let mut index = CollidableBvh::default();
        index.sync(&world);
        for x in -20..200 {
            let min = [x as f32 - 0.2, -1.0, -1.0];
            let max = [x as f32 + 0.2, 1.0, 1.0];
            let mut found = Vec::new();
            index.query(min, max, &mut Vec::new(), &mut found);
            let actual: HashSet<_> = found.into_iter().map(|i| index.surface(i).zone).collect();
            let expected: HashSet<_> = world
                .collidable_ids()
                .filter_map(|id| {
                    let zone = world.parent_of(id)?;
                    let z = world.get_component_by_id_as::<ZoneComponent>(zone)?;
                    let frame = zone_query::resolve_zone_frame(&world, zone, z).ok()?;
                    let (lo, hi) = collision_geometry::transformed_aabb(
                        z.shape,
                        TransformSystem::world_model(&world, frame)?,
                    );
                    (0..3)
                        .all(|i| lo[i] <= max[i] && min[i] <= hi[i])
                        .then_some(zone)
                })
                .collect();
            assert_eq!(actual, expected, "query at {x}");
        }
    }
}
