use crate::engine::ecs::component::{
    CollisionShape, QueryRootMode, TransformComponent, ZoneComponent, resolve_component_ref,
};
use crate::engine::ecs::system::{TransformSystem, collision_geometry};
use crate::engine::ecs::{ComponentId, World};
use crate::utils::math::{mat4_inverse, mat4_mul_vec4};

const ZONE_EPSILON: f32 = 1.0e-5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneRelation {
    Outside,
    Boundary,
    Inside,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoneQueryError {
    NotZone(ComponentId),
    Disabled(ComponentId),
    UnresolvedFrame(ComponentId),
    FrameHasNoTransform {
        zone: ComponentId,
        resolved: ComponentId,
    },
    SingularFrame(ComponentId),
    NonConverged {
        a: ComponentId,
        b: ComponentId,
    },
    UnsupportedContactFrame(ComponentId),
    UnsupportedFloorShape(ComponentId),
    InvalidSweepStart(ComponentId),
}

/// A synchronous shape query at the zones' current world poses. Use
/// `contact_zones` for separation data and `sweep_capsule_floor` for a crossing.
#[derive(Debug, Clone, PartialEq)]
pub struct ZoneOverlap {
    pub a: ComponentId,
    pub b: ComponentId,
    pub a_shape: CollisionShape,
    pub b_shape: CollisionShape,
    pub a_frame: [[f32; 4]; 4],
    pub b_frame: [[f32; 4]; 4],
    pub intersects: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ZoneSeparation {
    /// Minimum displacement of `overlap.a` out of `overlap.b`.
    pub displacement: [f32; 3],
    pub normal: [f32; 3],
    pub depth: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ZoneContact {
    pub overlap: ZoneOverlap,
    /// None for separated or exactly tangent zones.
    pub separation: Option<ZoneSeparation>,
}

/// Current-pose contact data for axis-aligned boxes and uniformly scaled
/// spheres/capsules. General transformed overlap remains available through
/// `overlap_zones`; unsupported contact frames return an error when overlapping.
pub fn contact_zones(
    world: &World,
    a: ComponentId,
    b: ComponentId,
) -> Result<ZoneContact, ZoneQueryError> {
    let overlap = overlap_zones(world, a, b)?;
    if !overlap.intersects {
        return Ok(ZoneContact {
            overlap,
            separation: None,
        });
    }
    let (a_center, a_shape) =
        collision_geometry::axis_aligned_world_shape(overlap.a_shape, overlap.a_frame)
            .ok_or(ZoneQueryError::UnsupportedContactFrame(a))?;
    let (b_center, b_shape) =
        collision_geometry::axis_aligned_world_shape(overlap.b_shape, overlap.b_frame)
            .ok_or(ZoneQueryError::UnsupportedContactFrame(b))?;
    let separation = collision_geometry::minimum_translation(
        a_center, a_shape, b_center, b_shape, 0.0,
    )
    .map(|displacement| {
        let depth = (displacement[0] * displacement[0]
            + displacement[1] * displacement[1]
            + displacement[2] * displacement[2])
            .sqrt();
        ZoneSeparation {
            displacement,
            normal: displacement.map(|component| component / depth),
            depth,
        }
    });
    Ok(ZoneContact {
        overlap,
        separation,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct ZoneSweepHit {
    pub moving: ComponentId,
    pub surface: ComponentId,
    pub fraction: f32,
    pub point: [f32; 3],
    pub normal: [f32; 3],
}

/// Detect a downward crossing of the top face of an upright static box (including yaw).
/// `previous_center` is the capsule's center before its pose driver moved it;
/// the zone's current frame supplies the proposed end center. This narrow
/// floor query intentionally does not cover side walls or sloped surfaces.
pub fn sweep_capsule_floor(
    world: &World,
    moving: ComponentId,
    surface: ComponentId,
    previous_center: [f32; 3],
) -> Result<Option<ZoneSweepHit>, ZoneQueryError> {
    if !previous_center
        .iter()
        .all(|coordinate| coordinate.is_finite())
    {
        return Err(ZoneQueryError::InvalidSweepStart(moving));
    }
    let (moving_shape, moving_frame) = zone_shape_frame(world, moving)?;
    let (floor_shape, floor_frame) = zone_shape_frame(world, surface)?;
    let (end, moving_shape) =
        collision_geometry::axis_aligned_world_shape(moving_shape, moving_frame)
            .ok_or(ZoneQueryError::UnsupportedContactFrame(moving))?;
    let (floor_center, half_extents, axes) = collision_geometry::yaw_box(floor_shape, floor_frame)
        .ok_or(ZoneQueryError::UnsupportedContactFrame(surface))?;
    let CollisionShape::CapsuleY {
        radius,
        half_segment,
    } = moving_shape
    else {
        return Err(ZoneQueryError::UnsupportedFloorShape(moving));
    };
    let top = floor_center[1] + half_extents[1];
    let bottom_offset = half_segment + radius;
    let start_bottom = previous_center[1] - bottom_offset;
    let end_bottom = end[1] - bottom_offset;
    if start_bottom < top - ZONE_EPSILON
        || end_bottom > top + ZONE_EPSILON
        || end_bottom >= start_bottom
    {
        return Ok(None);
    }
    let fraction = ((start_bottom - top) / (start_bottom - end_bottom)).clamp(0.0, 1.0);
    let x = previous_center[0] + (end[0] - previous_center[0]) * fraction;
    let z = previous_center[2] + (end[2] - previous_center[2]) * fraction;
    // At the instant its bottom crosses the top plane, a capsule's rounded
    // cap touches at its center X/Z. Side and edge sweeps are separate queries.
    let relative = [x - floor_center[0], 0.0, z - floor_center[2]];
    let local_x = (0..3).map(|i| relative[i] * axes[0][i]).sum::<f32>();
    let local_z = (0..3).map(|i| relative[i] * axes[2][i]).sum::<f32>();
    if local_x.abs() > half_extents[0] + ZONE_EPSILON
        || local_z.abs() > half_extents[2] + ZONE_EPSILON
    {
        return Ok(None);
    }
    Ok(Some(ZoneSweepHit {
        moving,
        surface,
        fraction,
        point: [x, top, z],
        normal: [0.0, 1.0, 0.0],
    }))
}

pub fn overlap_zones(
    world: &World,
    a: ComponentId,
    b: ComponentId,
) -> Result<ZoneOverlap, ZoneQueryError> {
    let (a_shape, a_frame) = zone_shape_frame(world, a)?;
    let (b_shape, b_frame) = zone_shape_frame(world, b)?;
    let (a_min, a_max) = collision_geometry::transformed_aabb(a_shape, a_frame);
    let (b_min, b_max) = collision_geometry::transformed_aabb(b_shape, b_frame);
    let candidate = (0..3).all(|axis| a_min[axis] <= b_max[axis] && b_min[axis] <= a_max[axis]);
    let intersects = if candidate {
        match (
            collision_geometry::axis_aligned_world_shape(a_shape, a_frame),
            collision_geometry::axis_aligned_world_shape(b_shape, b_frame),
        ) {
            (Some((a_center, a_world_shape)), Some((b_center, b_world_shape))) => {
                collision_geometry::intersects(a_center, a_world_shape, b_center, b_world_shape)
            }
            _ => collision_geometry::intersects_transformed(a_shape, a_frame, b_shape, b_frame)
                .ok_or(ZoneQueryError::NonConverged { a, b })?,
        }
    } else {
        false
    };
    Ok(ZoneOverlap {
        a,
        b,
        a_shape,
        b_shape,
        a_frame,
        b_frame,
        intersects,
    })
}

fn zone_shape_frame(
    world: &World,
    id: ComponentId,
) -> Result<(CollisionShape, [[f32; 4]; 4]), ZoneQueryError> {
    let zone = world
        .get_component_by_id_as::<ZoneComponent>(id)
        .ok_or(ZoneQueryError::NotZone(id))?;
    if !zone.enabled {
        return Err(ZoneQueryError::Disabled(id));
    }
    let frame_id = resolve_zone_frame(world, id, zone)?;
    let frame = TransformSystem::world_model(world, frame_id).ok_or(
        ZoneQueryError::FrameHasNoTransform {
            zone: id,
            resolved: frame_id,
        },
    )?;
    Ok((zone.shape, frame))
}

pub fn classify_point(
    world: &World,
    zone_id: ComponentId,
    point_world: [f32; 3],
) -> Result<ZoneRelation, ZoneQueryError> {
    let zone = world
        .get_component_by_id_as::<ZoneComponent>(zone_id)
        .ok_or(ZoneQueryError::NotZone(zone_id))?;
    if !zone.enabled {
        return Err(ZoneQueryError::Disabled(zone_id));
    }
    let frame = resolve_zone_frame(world, zone_id, zone)?;
    let world_matrix =
        TransformSystem::world_model(world, frame).ok_or(ZoneQueryError::FrameHasNoTransform {
            zone: zone_id,
            resolved: frame,
        })?;
    let inverse = mat4_inverse(world_matrix).ok_or(ZoneQueryError::SingularFrame(zone_id))?;
    let local = mat4_mul_vec4(
        inverse,
        [point_world[0], point_world[1], point_world[2], 1.0],
    );
    if !local.iter().all(|value| value.is_finite()) || local[3].abs() <= f32::EPSILON {
        return Err(ZoneQueryError::SingularFrame(zone_id));
    }
    let point = [
        local[0] / local[3],
        local[1] / local[3],
        local[2] / local[3],
    ];
    Ok(classify_local_point(zone.shape, point))
}

pub fn zone_has_role(zone: &ZoneComponent, role: &str) -> bool {
    zone.roles.iter().any(|candidate| candidate == role)
}

/// Enumerate enabled zones in stable component-tree order below `root`.
pub fn zones_in_subtree(world: &World, root: ComponentId, role: Option<&str>) -> Vec<ComponentId> {
    fn visit(
        world: &World,
        component: ComponentId,
        role: Option<&str>,
        zones: &mut Vec<ComponentId>,
    ) {
        if let Some(zone) = world.get_component_by_id_as::<ZoneComponent>(component)
            && zone.enabled
            && role.is_none_or(|required| zone_has_role(zone, required))
        {
            zones.push(component);
        }
        for &child in world.children_of(component) {
            visit(world, child, role, zones);
        }
    }

    let mut zones = Vec::new();
    if world.get_component_record(root).is_some() {
        visit(world, root, role, &mut zones);
    }
    zones
}

pub fn resolve_zone_frame(
    world: &World,
    zone_id: ComponentId,
    zone: &ZoneComponent,
) -> Result<ComponentId, ZoneQueryError> {
    let resolved = match &zone.frame_source {
        Some(source) => resolve_component_ref(
            world,
            source,
            Some(zone_id),
            QueryRootMode::ParentScope { levels_up: 1 },
        )
        .ok_or(ZoneQueryError::UnresolvedFrame(zone_id))?,
        None => world
            .parent_of(zone_id)
            .ok_or(ZoneQueryError::UnresolvedFrame(zone_id))?,
    };
    let frame = nearest_transform(world, resolved).ok_or(ZoneQueryError::FrameHasNoTransform {
        zone: zone_id,
        resolved,
    })?;
    let world_matrix =
        TransformSystem::world_model(world, frame).ok_or(ZoneQueryError::FrameHasNoTransform {
            zone: zone_id,
            resolved: frame,
        })?;
    if mat4_inverse(world_matrix).is_none() {
        return Err(ZoneQueryError::SingularFrame(zone_id));
    }
    Ok(frame)
}

fn nearest_transform(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut current = Some(start);
    while let Some(component) = current {
        if world
            .get_component_by_id_as::<TransformComponent>(component)
            .is_some()
        {
            return Some(component);
        }
        current = world.parent_of(component);
    }
    None
}

fn classify_local_point(shape: CollisionShape, point: [f32; 3]) -> ZoneRelation {
    match shape.normalized() {
        CollisionShape::Cube { half_extents } => {
            let margins = [
                half_extents[0] - point[0].abs(),
                half_extents[1] - point[1].abs(),
                half_extents[2] - point[2].abs(),
            ];
            classify_margins(&margins)
        }
        CollisionShape::Sphere { radius } => {
            let distance = (point[0] * point[0] + point[1] * point[1] + point[2] * point[2]).sqrt();
            classify_margin(radius - distance)
        }
        CollisionShape::CapsuleY {
            radius,
            half_segment,
        } => {
            let closest_y = point[1].clamp(-half_segment, half_segment);
            let dy = point[1] - closest_y;
            let distance = (point[0] * point[0] + dy * dy + point[2] * point[2]).sqrt();
            classify_margin(radius - distance)
        }
    }
}

fn classify_margins(margins: &[f32]) -> ZoneRelation {
    if margins.iter().any(|margin| *margin < -ZONE_EPSILON) {
        ZoneRelation::Outside
    } else if margins.iter().any(|margin| margin.abs() <= ZONE_EPSILON) {
        ZoneRelation::Boundary
    } else {
        ZoneRelation::Inside
    }
}

fn classify_margin(margin: f32) -> ZoneRelation {
    if margin < -ZONE_EPSILON {
        ZoneRelation::Outside
    } else if margin.abs() <= ZONE_EPSILON {
        ZoneRelation::Boundary
    } else {
        ZoneRelation::Inside
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{ComponentRef, TransformComponent};

    #[test]
    fn transformed_cube_classifies_inside_boundary_and_outside() {
        let mut world = World::default();
        let frame = world.add_component(
            TransformComponent::new()
                .with_position(2.0, 0.0, 0.0)
                .with_rotation_quat([
                    0.0,
                    0.0,
                    std::f32::consts::FRAC_1_SQRT_2,
                    std::f32::consts::FRAC_1_SQRT_2,
                ])
                .with_scale(2.0, 1.0, 1.0),
        );
        let transform = world
            .get_component_by_id_as_mut::<TransformComponent>(frame)
            .unwrap();
        transform.transform.matrix_world = transform.transform.model;
        let zone = world.add_component(ZoneComponent::cube([1.0, 0.5, 0.5]));
        world.add_child(frame, zone).unwrap();

        assert_eq!(
            classify_point(&world, zone, [2.0, 0.0, 0.0]),
            Ok(ZoneRelation::Inside)
        );
        assert_eq!(
            classify_point(&world, zone, [2.0, 2.0, 0.0]),
            Ok(ZoneRelation::Boundary)
        );
        assert_eq!(
            classify_point(&world, zone, [2.0, 2.1, 0.0]),
            Ok(ZoneRelation::Outside)
        );
    }

    #[test]
    fn at_accepts_guid_and_query_refs_in_the_containing_scope() {
        let mut world = World::default();
        let scope = world.add_component(TransformComponent::new());
        let target = world.add_component_boxed_named("target", Box::new(TransformComponent::new()));
        let by_query = world.add_component(
            ZoneComponent::sphere(1.0).at(ComponentRef::Query("[name='target']".into())),
        );
        let guid = world.get_component_record(target).unwrap().guid;
        let by_guid = world.add_component(ZoneComponent::sphere(1.0).at(ComponentRef::Guid(guid)));
        world.add_child(scope, target).unwrap();
        world.add_child(scope, by_query).unwrap();
        world.add_child(scope, by_guid).unwrap();

        assert_eq!(
            classify_point(&world, by_query, [0.0, 0.0, 0.0]),
            Ok(ZoneRelation::Inside)
        );
        assert_eq!(
            classify_point(&world, by_guid, [0.0, 0.0, 0.0]),
            Ok(ZoneRelation::Inside)
        );
    }

    #[test]
    fn disabled_and_unresolved_zones_fail_explicitly() {
        let mut world = World::default();
        let frame = world.add_component(TransformComponent::new());
        let disabled = world.add_component(ZoneComponent::sphere(1.0).enabled(false));
        let unresolved = world.add_component(
            ZoneComponent::sphere(1.0).at(ComponentRef::Query("[name='missing']".into())),
        );
        world.add_child(frame, disabled).unwrap();
        world.add_child(frame, unresolved).unwrap();

        assert_eq!(
            classify_point(&world, disabled, [0.0; 3]),
            Err(ZoneQueryError::Disabled(disabled))
        );
        assert_eq!(
            classify_point(&world, unresolved, [0.0; 3]),
            Err(ZoneQueryError::UnresolvedFrame(unresolved))
        );
    }

    #[test]
    fn sphere_and_capsule_classify_curved_boundaries() {
        let mut world = World::default();
        let frame = world.add_component(TransformComponent::new());
        let sphere = world.add_component(ZoneComponent::sphere(1.0));
        let capsule = world.add_component(ZoneComponent::capsule_y(0.5, 1.0));
        world.add_child(frame, sphere).unwrap();
        world.add_child(frame, capsule).unwrap();

        assert_eq!(
            classify_point(&world, sphere, [1.0, 0.0, 0.0]),
            Ok(ZoneRelation::Boundary)
        );
        assert_eq!(
            classify_point(&world, sphere, [1.01, 0.0, 0.0]),
            Ok(ZoneRelation::Outside)
        );
        assert_eq!(
            classify_point(&world, capsule, [0.0, 1.5, 0.0]),
            Ok(ZoneRelation::Boundary)
        );
        assert_eq!(
            classify_point(&world, capsule, [0.0, 0.0, 0.0]),
            Ok(ZoneRelation::Inside)
        );
    }

    #[test]
    fn singular_zone_frame_is_rejected() {
        let mut world = World::default();
        let frame = world.add_component(TransformComponent::new().with_scale(0.0, 1.0, 1.0));
        let transform = world
            .get_component_by_id_as_mut::<TransformComponent>(frame)
            .unwrap();
        transform.transform.matrix_world = transform.transform.model;
        let zone = world.add_component(ZoneComponent::cube([1.0; 3]));
        world.add_child(frame, zone).unwrap();

        assert_eq!(
            classify_point(&world, zone, [0.0; 3]),
            Err(ZoneQueryError::SingularFrame(zone))
        );
    }

    #[test]
    fn subtree_enumeration_is_stable_role_filtered_and_skips_disabled_zones() {
        let mut world = World::default();
        let root = world.add_component(TransformComponent::new());
        let legs = world.add_component(ZoneComponent::sphere(1.0).role("mount"));
        let mouth = world.add_component(ZoneComponent::sphere(1.0).role("socket"));
        let disabled = world.add_component(ZoneComponent::sphere(1.0).role("mount").enabled(false));
        world.add_child(root, legs).unwrap();
        world.add_child(root, mouth).unwrap();
        world.add_child(root, disabled).unwrap();

        assert_eq!(zones_in_subtree(&world, root, None), vec![legs, mouth]);
        assert_eq!(zones_in_subtree(&world, root, Some("mount")), vec![legs]);
    }

    #[test]
    fn shape_overlap_uses_current_rotated_and_scaled_frames() {
        let mut world = World::default();
        let a_frame = world.add_component(
            TransformComponent::new().with_rotation_quat([0.0, 0.0, 0.38268343, 0.9238795]),
        );
        let b_frame = world.add_component(TransformComponent::new().with_position(0.9, 0.9, 0.0));
        for id in [a_frame, b_frame] {
            let transform = world
                .get_component_by_id_as_mut::<TransformComponent>(id)
                .unwrap();
            transform.transform.matrix_world = transform.transform.model;
        }
        let a = world.add_component(ZoneComponent::cube([1.0, 0.1, 0.1]));
        let b = world.add_component(ZoneComponent::sphere(0.2));
        world.add_child(a_frame, a).unwrap();
        world.add_child(b_frame, b).unwrap();

        // Their world AABBs overlap, but the sphere misses the thin rotated box.
        assert!(!overlap_zones(&world, a, b).unwrap().intersects);
        let transform = world
            .get_component_by_id_as_mut::<TransformComponent>(b_frame)
            .unwrap();
        transform.transform.translation = [0.65, 0.65, 0.0];
        transform.transform.recompute_model();
        transform.transform.matrix_world = transform.transform.model;
        assert!(overlap_zones(&world, a, b).unwrap().intersects);

        let transform = world
            .get_component_by_id_as_mut::<TransformComponent>(b_frame)
            .unwrap();
        transform.transform.translation = [2.0, 0.0, 0.0];
        transform.transform.scale = [2.0, 1.0, 1.0];
        transform.transform.recompute_model();
        transform.transform.matrix_world = transform.transform.model;
        assert!(!overlap_zones(&world, a, b).unwrap().intersects);
    }

    #[test]
    fn shape_overlap_reports_disabled_and_singular_frames() {
        let mut world = World::default();
        let frame = world.add_component(TransformComponent::new());
        let singular_frame =
            world.add_component(TransformComponent::new().with_scale(0.0, 1.0, 1.0));
        let transform = world
            .get_component_by_id_as_mut::<TransformComponent>(singular_frame)
            .unwrap();
        transform.transform.matrix_world = transform.transform.model;
        let a = world.add_component(ZoneComponent::sphere(1.0));
        let disabled = world.add_component(ZoneComponent::sphere(1.0).enabled(false));
        let singular = world.add_component(ZoneComponent::sphere(1.0));
        world.add_child(frame, a).unwrap();
        world.add_child(frame, disabled).unwrap();
        world.add_child(singular_frame, singular).unwrap();
        assert_eq!(
            overlap_zones(&world, a, disabled),
            Err(ZoneQueryError::Disabled(disabled))
        );
        assert_eq!(
            overlap_zones(&world, a, singular),
            Err(ZoneQueryError::SingularFrame(singular))
        );
    }

    #[test]
    fn shape_overlap_includes_tangency_and_ignores_zone_roles() {
        let mut world = World::default();
        let a_frame = world.add_component(TransformComponent::new());
        let b_frame = world.add_component(TransformComponent::new().with_position(1.5, 0.0, 0.0));
        let transform = world
            .get_component_by_id_as_mut::<TransformComponent>(b_frame)
            .unwrap();
        transform.transform.matrix_world = transform.transform.model;
        let a = world.add_component(ZoneComponent::capsule_y(0.5, 1.0));
        let b = world.add_component(ZoneComponent::sphere(1.0).role("decorative"));
        world.add_child(a_frame, a).unwrap();
        world.add_child(b_frame, b).unwrap();
        assert!(overlap_zones(&world, a, b).unwrap().intersects);

        let transform = world
            .get_component_by_id_as_mut::<TransformComponent>(b_frame)
            .unwrap();
        transform.transform.translation = [1.51, 0.0, 0.0];
        transform.transform.recompute_model();
        transform.transform.matrix_world = transform.transform.model;
        assert!(!overlap_zones(&world, a, b).unwrap().intersects);
    }

    #[test]
    fn contact_separates_capsule_from_floor_and_sphere_from_wall() {
        let mut world = World::default();
        let capsule_frame =
            world.add_component(TransformComponent::new().with_position(0.0, 1.4, 0.0));
        let floor_frame =
            world.add_component(TransformComponent::new().with_position(0.0, -0.5, 0.0));
        let sphere_frame =
            world.add_component(TransformComponent::new().with_position(1.15, 0.0, 0.0));
        let wall_frame =
            world.add_component(TransformComponent::new().with_position(1.5, 0.0, 0.0));
        for id in [capsule_frame, floor_frame, sphere_frame, wall_frame] {
            let t = world
                .get_component_by_id_as_mut::<TransformComponent>(id)
                .unwrap();
            t.transform.matrix_world = t.transform.model;
        }
        let capsule = world.add_component(ZoneComponent::capsule_y(0.5, 1.0));
        let floor = world.add_component(ZoneComponent::cube([5.0, 0.5, 5.0]));
        let sphere = world.add_component(ZoneComponent::sphere(0.25));
        let wall = world.add_component(ZoneComponent::cube([0.25, 2.0, 2.0]));
        for (frame, zone) in [
            (capsule_frame, capsule),
            (floor_frame, floor),
            (sphere_frame, sphere),
            (wall_frame, wall),
        ] {
            world.add_child(frame, zone).unwrap();
        }
        let floor_contact = contact_zones(&world, capsule, floor).unwrap();
        let separation = floor_contact.separation.unwrap();
        assert!((separation.depth - 0.1).abs() < 1.0e-5);
        assert_eq!(separation.normal, [0.0, 1.0, 0.0]);

        let wall_contact = contact_zones(&world, sphere, wall).unwrap();
        let separation = wall_contact.separation.unwrap();
        assert!((separation.depth - 0.15).abs() < 1.0e-5);
        assert_eq!(separation.normal, [-1.0, 0.0, 0.0]);
    }

    #[test]
    fn floor_sweep_catches_crossing_without_end_pose_overlap() {
        let mut world = World::default();
        let moving_frame =
            world.add_component(TransformComponent::new().with_position(0.0, -2.0, 0.0));
        let floor_frame =
            world.add_component(TransformComponent::new().with_position(0.0, -0.1, 0.0));
        for id in [moving_frame, floor_frame] {
            let t = world
                .get_component_by_id_as_mut::<TransformComponent>(id)
                .unwrap();
            t.transform.matrix_world = t.transform.model;
        }
        let moving = world.add_component(ZoneComponent::capsule_y(0.25, 0.75));
        let floor = world.add_component(ZoneComponent::cube([2.0, 0.1, 2.0]));
        world.add_child(moving_frame, moving).unwrap();
        world.add_child(floor_frame, floor).unwrap();
        assert!(!overlap_zones(&world, moving, floor).unwrap().intersects);
        let hit = sweep_capsule_floor(&world, moving, floor, [0.0, 2.0, 0.0])
            .unwrap()
            .unwrap();
        assert!((hit.fraction - 0.25).abs() < 1.0e-5);
        assert_eq!(hit.point, [0.0, 0.0, 0.0]);
        assert_eq!(hit.normal, [0.0, 1.0, 0.0]);
        assert!(
            sweep_capsule_floor(&world, moving, floor, [5.0, 2.0, 0.0])
                .unwrap()
                .is_none()
        );
        assert_eq!(
            sweep_capsule_floor(&world, moving, floor, [f32::NAN, 2.0, 0.0]),
            Err(ZoneQueryError::InvalidSweepStart(moving))
        );
    }

    #[test]
    fn rotated_contact_reports_unsupported_frame_but_overlap_still_works() {
        let mut world = World::default();
        let frame = world.add_component(
            TransformComponent::new().with_rotation_quat([0.0, 0.0, 0.38268343, 0.9238795]),
        );
        let t = world
            .get_component_by_id_as_mut::<TransformComponent>(frame)
            .unwrap();
        t.transform.matrix_world = t.transform.model;
        let other_frame = world.add_component(TransformComponent::new());
        let a = world.add_component(ZoneComponent::cube([1.0; 3]));
        let b = world.add_component(ZoneComponent::sphere(0.2));
        world.add_child(frame, a).unwrap();
        world.add_child(other_frame, b).unwrap();
        assert!(overlap_zones(&world, a, b).unwrap().intersects);
        assert_eq!(
            contact_zones(&world, a, b),
            Err(ZoneQueryError::UnsupportedContactFrame(a))
        );
    }
}
