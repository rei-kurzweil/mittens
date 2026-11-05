use super::collidable_bvh::CollidableBvh;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::engine::ecs::component::{
    CollidableComponent, QueryRootMode, TransformComponent, VelocityComponent, ZoneComponent,
    resolve_component_ref,
};
use crate::engine::ecs::system::{TransformSystem, VelocitySystem, collision_geometry, zone_query};
use crate::engine::ecs::{ComponentId, World};
use crate::utils::math::{mat4_inverse, mat4_mul_vec4};

#[derive(Debug, Default)]
pub struct StaticContactSystem {
    previous_centers: HashMap<ComponentId, [f32; 3]>,
    body_offsets: HashMap<ComponentId, [f32; 3]>,
    broad_phase: CollidableBvh,
    traversal: Vec<usize>,
    candidates: Vec<usize>,
    additional: Vec<usize>,
    pub bvh_node_visits: u64,
    pub index_update_time: Duration,
    pub query_time: Duration,
    pub exact_test_time: Duration,
    pub total_time: Duration,
    profile_passes: u64,
    profile_times: [Duration; 4],
    profile_counts: [u64; 3],
    pub candidate_pairs: u64,
    pub narrow_phase_tests: u64,
    pub correction_iterations: u64,
    pub non_convergences: u64,
}

impl StaticContactSystem {
    /// Share the retained index with read-only pose consumers after propagation.
    /// Synchronize once at their phase boundary; never build a per-avatar index.
    pub(crate) fn support_index(&mut self, world: &World) -> &CollidableBvh {
        self.broad_phase.sync(world);
        &self.broad_phase
    }

    pub(crate) fn forget_zone(&mut self, zone: ComponentId) {
        self.previous_centers.remove(&zone);
    }

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
        let profiling = spatial_profiling();
        let started = profiling.then(Instant::now);
        self.bvh_node_visits = 0;
        self.query_time = Duration::ZERO;
        self.exact_test_time = Duration::ZERO;
        let maintenance = profiling.then(Instant::now);
        self.broad_phase.sync(world);
        self.index_update_time = maintenance.map(|t| t.elapsed()).unwrap_or_default();
        let movers: Vec<_> = self
            .broad_phase
            .movers
            .iter()
            .map(|(&id, &zone)| (id, zone))
            .collect();
        let live_movers: HashSet<_> = movers.iter().map(|&(_, zone)| zone).collect();
        self.previous_centers
            .retain(|zone, _| live_movers.contains(zone));
        let mut candidates = std::mem::take(&mut self.candidates);
        let mut additional = std::mem::take(&mut self.additional);
        let mut changed = Vec::new();
        self.body_offsets.clear();
        for (collidable_id, moving) in movers {
            let Some(target) = movement_target(world, collidable_id, moving) else {
                continue;
            };
            let Some((cached_center, shape)) = zone_world_shape(world, moving) else {
                continue;
            };
            // Several contact zones may drive one body. World matrices are
            // propagated after this pass, so account for prior zones' corrections.
            let offset = self.body_offsets.get(&target).copied().unwrap_or([0.0; 3]);
            let center = std::array::from_fn(|i| cached_center[i] + offset[i]);
            let previous = self
                .previous_centers
                .insert(moving, center)
                .unwrap_or(center);
            if mounts.is_movement_root_mounted(world, target) {
                self.previous_centers.remove(&moving);
                continue;
            }
            let mut desired_center = center;
            for iteration in 0..6 {
                let mut corrected = false;
                self.query_candidates(
                    previous,
                    desired_center,
                    shape,
                    iteration == 0,
                    &mut candidates,
                );
                let mut cursor = 0;
                while cursor < candidates.len() {
                    let index = candidates[cursor];
                    cursor += 1;
                    let surface = self.broad_phase.surface(index);
                    if surface.zone == moving || is_body_part(world, surface.zone, target) {
                        continue;
                    }
                    self.candidate_pairs += 1;
                    self.narrow_phase_tests += 1;
                    let exact = profiling.then(Instant::now);
                    let sweep = if iteration == 0 {
                        surface.floor_sweep(moving, desired_center, shape, previous)
                    } else {
                        None
                    };
                    let displacement = if let Some(hit) = sweep {
                        let motion: [f32; 3] =
                            std::array::from_fn(|i| desired_center[i] - previous[i]);
                        let remaining: [f32; 3] = motion.map(|v| v * (1.0 - hit.fraction));
                        let inward: f32 = (0..3).map(|i| remaining[i] * hit.normal[i]).sum();
                        hit.normal.map(|v| v * (-inward).max(0.0))
                    } else {
                        surface
                            .translation(desired_center, shape)
                            .unwrap_or([0.0; 3])
                    };
                    self.exact_test_time += exact.map(|t| t.elapsed()).unwrap_or_default();
                    if displacement.iter().all(|v| v.abs() < 1.0e-6) {
                        continue;
                    }
                    resolve_contact_velocity(
                        world,
                        target,
                        displacement,
                        if apply_friction {
                            surface.friction
                        } else {
                            0.0
                        },
                    );
                    for axis in 0..3 {
                        desired_center[axis] += displacement[axis];
                    }
                    corrected = true;
                    self.correction_iterations += 1;
                    // A correction can expose a neighbor outside the original query.
                    // Discover it now; already-processed surfaces are revisited next pass.
                    self.query_candidates(
                        previous,
                        desired_center,
                        shape,
                        iteration == 0,
                        &mut additional,
                    );
                    for &new in &additional {
                        if !candidates.contains(&new) {
                            candidates.push(new);
                        }
                    }
                    candidates[cursor..]
                        .sort_unstable_by_key(|&i| self.broad_phase.surface(i).collidable);
                }
                if !corrected {
                    break;
                }
            }
            self.query_candidates(previous, desired_center, shape, false, &mut candidates);
            let mut remaining = false;
            for &index in &candidates {
                let surface = self.broad_phase.surface(index);
                if surface.zone == moving || is_body_part(world, surface.zone, target) {
                    continue;
                }
                self.candidate_pairs += 1;
                self.narrow_phase_tests += 1;
                let exact = profiling.then(Instant::now);
                remaining = surface
                    .translation(desired_center, shape)
                    .is_some_and(|delta| delta.iter().any(|v| v.abs() >= 1.0e-6));
                self.exact_test_time += exact.map(|t| t.elapsed()).unwrap_or_default();
                if remaining {
                    break;
                }
            }
            if remaining {
                self.non_convergences += 1;
            }
            let displacement: [f32; 3] = std::array::from_fn(|i| desired_center[i] - center[i]);
            if displacement.iter().all(|v| v.abs() < 1.0e-6) {
                continue;
            }
            let Some(target_world) = TransformSystem::world_position(world, target) else {
                continue;
            };
            let desired = std::array::from_fn(|i| target_world[i] + offset[i] + displacement[i]);
            let local = world_to_local(world, target, desired);
            if let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(target) {
                t.transform.translation = local;
                t.transform.recompute_model();
                changed.push(target);
                self.previous_centers.insert(moving, desired_center);
                self.body_offsets
                    .insert(target, std::array::from_fn(|i| offset[i] + displacement[i]));
            }
        }
        self.candidates = candidates;
        self.additional = additional;
        self.total_time = started.map(|t| t.elapsed()).unwrap_or_default();
        if profiling {
            self.profile_passes += 1;
            for (sum, elapsed) in self.profile_times.iter_mut().zip([
                self.index_update_time,
                self.query_time,
                self.exact_test_time,
                self.total_time,
            ]) {
                *sum += elapsed;
            }
            for (sum, count) in self.profile_counts.iter_mut().zip([
                self.bvh_node_visits,
                self.candidate_pairs,
                self.narrow_phase_tests,
            ]) {
                *sum += count;
            }
            if self.profile_passes == 120 {
                let times = self.profile_times.map(|t| t.as_secs_f64() * 1000.0 / 120.0);
                let line = format!(
                    "[spatial-profile][contact] passes=120 statics={} movers={} avg_ms maintenance={:.4} query={:.4} exact={:.4} total={:.4} nodes={} candidates={} exact_tests={} rebuilds={} refits={}",
                    self.broad_phase.len(),
                    self.broad_phase.movers.len(),
                    times[0],
                    times[1],
                    times[2],
                    times[3],
                    self.profile_counts[0],
                    self.profile_counts[1],
                    self.profile_counts[2],
                    self.broad_phase.rebuilds,
                    self.broad_phase.refits
                );
                eprintln!("{line}");
                crate::utils::profile_log::append("spatial-profile", &line);
                self.profile_passes = 0;
                self.profile_times = [Duration::ZERO; 4];
                self.profile_counts = [0; 3];
            }
        }
        changed
    }

    fn query_candidates(
        &mut self,
        previous: [f32; 3],
        center: [f32; 3],
        shape: crate::engine::ecs::component::CollisionShape,
        swept: bool,
        out: &mut Vec<usize>,
    ) {
        let started = spatial_profiling().then(Instant::now);
        let (mut min, mut max) = shape_aabb(center, shape);
        if swept {
            let (start_min, start_max) = shape_aabb(previous, shape);
            for i in 0..3 {
                min[i] = min[i].min(start_min[i]);
                max[i] = max[i].max(start_max[i]);
            }
        }
        self.bvh_node_visits += self.broad_phase.query(min, max, &mut self.traversal, out);
        self.query_time += started.map(|t| t.elapsed()).unwrap_or_default();
    }
}

fn is_body_part(world: &World, zone: ComponentId, target: ComponentId) -> bool {
    let mut current = Some(zone);
    while let Some(id) = current {
        if id == target {
            return true;
        }
        current = world.parent_of(id);
    }
    false
}

fn spatial_profiling() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        matches!(
            std::env::var("CAT_PROFILE_SPATIAL")
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
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
#[path = "static_contact_tests.rs"]
mod tests;

#[cfg(test)]
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

#[cfg(test)]
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
