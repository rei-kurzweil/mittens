//! Rays against the world's solid collidables: the nearest entry into an
//! enabled, non-sensor collidable zone (its triangle mesh for imported and
//! cut parts). A linear scan, for occasional queries (scripts' line of sight,
//! the third-person camera), not per-object per-frame use.
use crate::engine::ecs::component::{CollidableComponent, CollidableMode, CollisionShape, ZoneComponent};
use crate::engine::ecs::system::{TransformSystem, triangle_collision, zone_query};
use crate::engine::ecs::{ComponentId, World};
use crate::utils::math::{mat4_inverse, mat4_mul_vec4};

pub(crate) fn raycast(world: &World, origin: [f32; 3], direction: [f32; 3], max: f32) -> Option<f32> {
    raycast_filtered(world, origin, direction, max, |_| true)
}

/// As [`raycast`], skipping hits on zones `keep` rejects (it sees the zone id,
/// and is asked only about zones the ray actually enters).
pub(crate) fn raycast_filtered(
    world: &World,
    origin: [f32; 3],
    direction: [f32; 3],
    max: f32,
    keep: impl Fn(ComponentId) -> bool,
) -> Option<f32> {
    let length = direction.iter().map(|v| v * v).sum::<f32>().sqrt();
    if !length.is_finite() || length < 1e-6 || !(max > 0.0) {
        return None;
    }
    let dir = direction.map(|v| v / length);
    let mut best: Option<f32> = None;
    for id in world.collidable_ids() {
        let Some(collidable) = world.get_component_by_id_as::<CollidableComponent>(id) else {
            continue;
        };
        if !collidable.enabled || collidable.mode == CollidableMode::Sensor {
            continue;
        }
        let Some(zone_id) = world.parent_of(id) else {
            continue;
        };
        let Some(zone) = world
            .get_component_by_id_as::<ZoneComponent>(zone_id)
            .filter(|zone| zone.enabled)
        else {
            continue;
        };
        let Some(frame) = zone_query::resolve_zone_frame(world, zone_id, zone).ok() else {
            continue;
        };
        let Some(inverse) = TransformSystem::world_model(world, frame).and_then(mat4_inverse) else {
            continue;
        };
        let o = mat4_mul_vec4(inverse, [origin[0], origin[1], origin[2], 1.0]);
        let d = mat4_mul_vec4(inverse, [dir[0], dir[1], dir[2], 0.0]);
        let (o, d) = ([o[0], o[1], o[2]], [d[0], d[1], d[2]]);
        // World distance and local ray parameter agree: the local direction
        // is the world unit direction mapped linearly.
        // Imported and cut parts carry their triangle mesh: a door panel's
        // window stays open, where its bounding box would not.
        let entry = if let Some(mesh) = triangle_collision::on(world, zone_id) {
            mesh.ray(o, d).filter(|t| *t > 1e-4)
        } else {
            match zone.shape {
                CollisionShape::Sphere { radius } => ray_sphere(o, d, radius),
                CollisionShape::Cube { half_extents } => ray_box(o, d, half_extents),
                CollisionShape::CapsuleY {
                    radius,
                    half_segment,
                } => ray_box(o, d, [radius, radius + half_segment, radius]),
            }
        };
        if let Some(t) = entry.filter(|t| *t <= max)
            && best.is_none_or(|b| t < b)
            && keep(zone_id)
        {
            best = Some(t);
        }
    }
    best
}

/// Entry parameter into a box centred at the origin; None if missed or if the
/// ray starts inside.
fn ray_box(o: [f32; 3], d: [f32; 3], half: [f32; 3]) -> Option<f32> {
    let mut near = f32::NEG_INFINITY;
    let mut far = f32::INFINITY;
    for i in 0..3 {
        if d[i].abs() < 1e-9 {
            if o[i].abs() > half[i] {
                return None;
            }
            continue;
        }
        let a = (-half[i] - o[i]) / d[i];
        let b = (half[i] - o[i]) / d[i];
        near = near.max(a.min(b));
        far = far.min(a.max(b));
    }
    (near <= far && near > 0.0).then_some(near)
}

/// Entry parameter into a sphere centred at the origin; None if missed or if
/// the ray starts inside.
fn ray_sphere(o: [f32; 3], d: [f32; 3], radius: f32) -> Option<f32> {
    let a: f32 = d.iter().map(|v| v * v).sum();
    let b: f32 = 2.0 * (0..3).map(|i| o[i] * d[i]).sum::<f32>();
    let c: f32 = o.iter().map(|v| v * v).sum::<f32>() - radius * radius;
    if c <= 0.0 {
        return None;
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 || a <= 0.0 {
        return None;
    }
    let t = (-b - disc.sqrt()) / (2.0 * a);
    (t > 0.0).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::TransformComponent;

    fn solid(world: &mut World, position: [f32; 3], shape: CollisionShape) {
        let frame = world.add_component(
            TransformComponent::new().with_position(position[0], position[1], position[2]),
        );
        let t = world.get_component_by_id_as_mut::<TransformComponent>(frame).unwrap();
        t.transform.matrix_world = t.transform.model;
        let zone = world.add_component(ZoneComponent::new(shape));
        let collidable = world.add_component(CollidableComponent::static_());
        world.add_child(frame, zone).unwrap();
        world.add_child(zone, collidable).unwrap();
    }

    #[test]
    fn nearest_solid_along_the_ray_and_never_the_one_it_starts_in() {
        let mut world = World::default();
        solid(&mut world, [0.0, 0.0, -5.0], CollisionShape::cube_half_extents([0.5; 3]));
        solid(&mut world, [0.0, 0.0, -8.0], CollisionShape::Sphere { radius: 1.0 });
        solid(&mut world, [0.0, 0.0, 0.0], CollisionShape::Sphere { radius: 0.5 });
        let hit = raycast(&world, [0.0; 3], [0.0, 0.0, -2.0], 20.0).unwrap();
        assert!((hit - 4.5).abs() < 1e-5, "{hit}");
        assert!(raycast(&world, [0.0; 3], [0.0, 0.0, -1.0], 4.0).is_none());
        assert!(raycast(&world, [3.0, 0.0, 0.0], [0.0, 0.0, -1.0], 20.0).is_none());
        let past_box = raycast(&world, [0.0, 0.0, -6.0], [0.0, 0.0, -1.0], 20.0).unwrap();
        assert!((past_box - 1.0).abs() < 1e-5, "{past_box}");
    }
}
