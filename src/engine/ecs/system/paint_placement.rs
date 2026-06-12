use crate::engine::ecs::component::{BoundsComponent, RenderableComponent, TransformComponent};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::system::grid_system::GridSnapResult;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::bounds::{Aabb, mesh_local_aabb};
use crate::engine::graphics::primitives::CpuMeshHandle;
use crate::utils::math::{
    mat_to_quat, mat4_inverse, shortest_arc_quat, vec3_add, vec3_cross, vec3_dot, vec3_len,
    vec3_normalize, vec3_scale, vec3_sub,
};

const SURFACE_EPSILON: f32 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacementPose {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub surface_normal: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacementError {
    MissingAssetBounds,
    MissingTargetTransform,
    UnsupportedSurface,
}

pub fn resolve_placement_pose(
    world: &World,
    target_renderable: ComponentId,
    hit_point_world: [f32; 3],
    asset_root: ComponentId,
    grid_snap: Option<GridSnapResult>,
) -> Result<PlacementPose, PlacementError> {
    let asset_bounds = measure_subtree_local_bounds(world, asset_root)
        .ok_or(PlacementError::MissingAssetBounds)?;

    let (surface_point, surface_normal) = match grid_snap {
        Some(grid) => (grid.point_world, grid.normal_world),
        None => (
            hit_point_world,
            resolve_surface_normal(world, target_renderable, hit_point_world)?,
        ),
    };

    resolve_surface_aligned_pose_from_normal(
        world,
        target_renderable,
        surface_point,
        surface_normal,
        asset_bounds.min[2],
    )
}

pub fn resolve_surface_aligned_pose(
    world: &World,
    target_renderable: ComponentId,
    hit_point_world: [f32; 3],
    local_min_z: f32,
    grid_snap: Option<GridSnapResult>,
) -> Result<PlacementPose, PlacementError> {
    let (surface_point, surface_normal) = match grid_snap {
        Some(grid) => (grid.point_world, grid.normal_world),
        None => (
            hit_point_world,
            resolve_surface_normal(world, target_renderable, hit_point_world)?,
        ),
    };

    resolve_surface_aligned_pose_from_normal(
        world,
        target_renderable,
        surface_point,
        surface_normal,
        local_min_z,
    )
}

fn resolve_surface_aligned_pose_from_normal(
    world: &World,
    target_renderable: ComponentId,
    surface_point: [f32; 3],
    surface_normal: [f32; 3],
    local_min_z: f32,
) -> Result<PlacementPose, PlacementError> {
    let rotation =
        make_alignment_quat(surface_normal, world_up_reference(world, target_renderable));
    let outward_offset = SURFACE_EPSILON - local_min_z;

    Ok(PlacementPose {
        translation: vec3_add(surface_point, vec3_scale(surface_normal, outward_offset)),
        rotation,
        surface_normal,
    })
}

pub fn resolve_surface_normal(
    world: &World,
    target_renderable: ComponentId,
    hit_point_world: [f32; 3],
) -> Result<[f32; 3], PlacementError> {
    let renderable = world
        .get_component_by_id_as::<RenderableComponent>(target_renderable)
        .ok_or(PlacementError::UnsupportedSurface)?;
    let target_world = TransformSystem::world_model(world, target_renderable)
        .ok_or(PlacementError::MissingTargetTransform)?;
    let inv_world = mat4_inverse(target_world).ok_or(PlacementError::MissingTargetTransform)?;
    let hit_local = transform_point(inv_world, hit_point_world);

    let local_normal = match renderable.renderable.base_mesh {
        CpuMeshHandle::QUAD_2D | CpuMeshHandle::TRIANGLE_2D => [0.0, 0.0, 1.0],
        CpuMeshHandle::CUBE => cube_face_normal(hit_local),
        CpuMeshHandle::SPHERE => vec3_normalize(hit_local),
        _ => return Err(PlacementError::UnsupportedSurface),
    };

    let world_normal = vec3_normalize(transform_direction(target_world, local_normal));
    if world_normal == [0.0, 0.0, 0.0] {
        return Err(PlacementError::UnsupportedSurface);
    }
    Ok(world_normal)
}

pub fn measure_subtree_local_bounds(world: &World, root: ComponentId) -> Option<Aabb> {
    fn visit(
        world: &World,
        node: ComponentId,
        parent_to_root: [[f32; 4]; 4],
        aggregate: &mut Option<Aabb>,
    ) {
        let mut local_to_root = parent_to_root;
        if let Some(transform) = world.get_component_by_id_as::<TransformComponent>(node) {
            local_to_root = crate::engine::graphics::bounds::mat4_mul(
                parent_to_root,
                transform.transform.model,
            );
        }

        if let Some(renderable) = world.get_component_by_id_as::<RenderableComponent>(node) {
            let local_bounds = world
                .children_of(node)
                .iter()
                .copied()
                .find_map(|child| world.get_component_by_id_as::<BoundsComponent>(child))
                .map(|bounds| bounds.local)
                .or_else(|| mesh_local_aabb(renderable.renderable.base_mesh));

            if let Some(bounds) = local_bounds {
                let transformed = bounds.transformed(local_to_root);
                *aggregate = Some(match aggregate.take() {
                    Some(prev) => prev.union(&transformed),
                    None => transformed,
                });
            }
        }

        for &child in world.children_of(node) {
            visit(world, child, local_to_root, aggregate);
        }
    }

    let mut aggregate = None;
    visit(
        world,
        root,
        crate::engine::graphics::bounds::mat4_identity(),
        &mut aggregate,
    );
    aggregate
}

fn cube_face_normal(hit_local: [f32; 3]) -> [f32; 3] {
    let extents = [0.5f32, 0.5, 0.5];
    let ratios = [
        (hit_local[0] / extents[0]).abs(),
        (hit_local[1] / extents[1]).abs(),
        (hit_local[2] / extents[2]).abs(),
    ];
    if ratios[0] >= ratios[1] && ratios[0] >= ratios[2] {
        [hit_local[0].signum(), 0.0, 0.0]
    } else if ratios[1] >= ratios[2] {
        [0.0, hit_local[1].signum(), 0.0]
    } else {
        [0.0, 0.0, hit_local[2].signum()]
    }
}

fn world_up_reference(world: &World, target_renderable: ComponentId) -> [f32; 3] {
    let target_world = TransformSystem::world_model(world, target_renderable)
        .unwrap_or(crate::utils::math::mat4_identity());
    let up = vec3_normalize(transform_direction(target_world, [0.0, 1.0, 0.0]));
    if vec3_len(up) > 1e-5 {
        up
    } else {
        [0.0, 1.0, 0.0]
    }
}

fn make_alignment_quat(surface_normal: [f32; 3], reference_up: [f32; 3]) -> [f32; 4] {
    let z = vec3_normalize(surface_normal);
    let projected_up = vec3_sub(reference_up, vec3_scale(z, vec3_dot(reference_up, z)));
    let y = if vec3_len(projected_up) > 1e-5 {
        vec3_normalize(projected_up)
    } else {
        let fallback = if z[1].abs() < 0.99 {
            [0.0, 1.0, 0.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        vec3_normalize(vec3_sub(fallback, vec3_scale(z, vec3_dot(fallback, z))))
    };
    let x = vec3_normalize(vec3_cross(y, z));
    let y = vec3_normalize(vec3_cross(z, x));

    let basis = [
        [x[0], x[1], x[2], 0.0],
        [y[0], y[1], y[2], 0.0],
        [z[0], z[1], z[2], 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let quat = mat_to_quat(basis);
    if quat == [0.0, 0.0, 0.0, 1.0] && surface_normal != [0.0, 0.0, 1.0] {
        shortest_arc_quat([0.0, 0.0, 1.0], surface_normal)
    } else {
        quat
    }
}

fn transform_point(m: [[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0],
        m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1],
        m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2],
    ]
}

fn transform_direction(m: [[f32; 4]; 4], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2],
        m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2],
        m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{ColorComponent, RenderableComponent};

    fn build_asset(world: &mut World, scale: [f32; 3]) -> ComponentId {
        let root = world.add_component(TransformComponent::new());
        let shape =
            world.add_component(TransformComponent::new().with_scale(scale[0], scale[1], scale[2]));
        let color = world.add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
        let renderable = world.add_component(RenderableComponent::cube());
        let _ = world.add_child(root, shape);
        let _ = world.add_child(shape, color);
        let _ = world.add_child(color, renderable);
        root
    }

    #[test]
    fn cube_face_normal_prefers_dominant_axis() {
        let normal = cube_face_normal([0.49, 0.10, 0.20]);
        assert_eq!(normal, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn sphere_normal_points_outward_from_center() {
        let mut world = World::default();
        let target = world.add_component(TransformComponent::new());
        let renderable = world.add_component(RenderableComponent::sphere());
        let _ = world.add_child(target, renderable);

        let normal = resolve_surface_normal(&world, renderable, [0.0, 0.0, 0.5]).expect("normal");
        assert!((normal[2] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn placement_offset_clears_surface_by_epsilon() {
        let mut world = World::default();
        let target = world.add_component(TransformComponent::new());
        let renderable = world.add_component(RenderableComponent::plane());
        let _ = world.add_child(target, renderable);
        let asset = build_asset(&mut world, [0.2, 0.2, 0.2]);

        let pose =
            resolve_placement_pose(&world, renderable, [0.0, 0.0, 0.0], asset, None).expect("pose");
        assert!((pose.translation[2] - 0.11).abs() < 1e-4);
    }

    #[test]
    fn unsupported_mesh_refusal_is_deterministic() {
        let mut world = World::default();
        let target = world.add_component(TransformComponent::new());
        let renderable = world.add_component(RenderableComponent::tetrahedron());
        let _ = world.add_child(target, renderable);
        let asset = build_asset(&mut world, [0.2, 0.2, 0.2]);

        let err = resolve_placement_pose(&world, renderable, [0.0, 0.0, 0.0], asset, None)
            .expect_err("unsupported surface");
        assert_eq!(err, PlacementError::UnsupportedSurface);
    }
}
