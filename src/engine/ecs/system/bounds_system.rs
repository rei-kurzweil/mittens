use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{RenderableComponent, TransformComponent};
use crate::engine::graphics::RenderAssets;
use crate::engine::graphics::bounds::{Aabb, mat4_identity, mat4_mul, mesh_local_aabb};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderableBoundsMeasure {
    Measured(Aabb),
    Unmeasurable,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UniformFitTransform {
    pub translation: [f32; 3],
    pub scale: [f32; 3],
}

/// Utility system for calculating aggregate bounds of component subtrees.
#[derive(Debug, Default)]
pub struct BoundsSystem;

impl BoundsSystem {
    pub fn new() -> Self {
        Self
    }

    /// Recursively compute the aggregate AABB of a component subtree in the root's coordinate frame.
    ///
    /// This walks the child topology and unions the transformed AABBs of all
    /// `RenderableComponent`s encountered. It accounts for nested `TransformComponent`s.
    pub fn calculate_subtree_local_bounds(
        world: &World,
        render_assets: &RenderAssets,
        root: ComponentId,
    ) -> Option<Aabb> {
        let mut aggregate: Option<Aabb> = None;
        let mut stack = vec![(root, mat4_identity())];

        while let Some((node, parent_to_root)) = stack.pop() {
            let mut local_to_root = parent_to_root;

            // Compose the transform of this node into the root-relative matrix.
            if let Some(tc) = world.get_component_by_id_as::<TransformComponent>(node) {
                local_to_root = mat4_mul(parent_to_root, tc.transform.model);
            }

            // If it's a renderable, union its transformed bounds.
            if let Some(r) = world.get_component_by_id_as::<RenderableComponent>(node) {
                // Try looking up the AABB for this specific mesh in RenderAssets,
                // otherwise fallback to the hardcoded primitives.
                let aabb = render_assets
                    .cpu_mesh(r.renderable.mesh)
                    .and_then(|cpu_mesh| {
                        Aabb::from_points(
                            &cpu_mesh
                                .vertices
                                .iter()
                                .map(|v| v.pos)
                                .collect::<Vec<[f32; 3]>>(),
                        )
                    })
                    .or_else(|| mesh_local_aabb(r.renderable.base_mesh));

                if let Some(local_aabb) = aabb {
                    let transformed = local_aabb.transformed(local_to_root);
                    aggregate = Some(match aggregate {
                        Some(a) => a.union(&transformed),
                        None => transformed,
                    });
                }
            }

            // Recurse into children.
            for &child in world.children_of(node) {
                stack.push((child, local_to_root));
            }
        }

        aggregate
    }

    pub fn measure_renderable_subtree_bounds(
        world: &World,
        render_assets: &RenderAssets,
        root: ComponentId,
    ) -> RenderableBoundsMeasure {
        match Self::calculate_subtree_local_bounds(world, render_assets, root) {
            Some(bounds) => RenderableBoundsMeasure::Measured(bounds),
            None => RenderableBoundsMeasure::Unmeasurable,
        }
    }

    pub fn fit_aabb_uniform(aabb: &Aabb, target_bounds: [f32; 6]) -> Option<UniformFitTransform> {
        let target = Aabb {
            min: [target_bounds[0], target_bounds[1], target_bounds[2]],
            max: [target_bounds[3], target_bounds[4], target_bounds[5]],
        };

        let measured_dims = [aabb.width(), aabb.height(), aabb.depth()];
        let target_dims = [target.width(), target.height(), target.depth()];
        let mut uniform_scale: Option<f32> = None;

        for (&measured, &target) in measured_dims.iter().zip(target_dims.iter()) {
            if measured <= 1e-6 || target <= 1e-6 {
                continue;
            }
            let axis_scale = target / measured;
            uniform_scale = Some(match uniform_scale {
                Some(current) => current.min(axis_scale),
                None => axis_scale,
            });
        }

        let scale = uniform_scale?;
        let measured_center = aabb.center();
        let target_center = target.center();

        Some(UniformFitTransform {
            translation: [
                target_center[0] - measured_center[0] * scale,
                target_center[1] - measured_center[1] * scale,
                target_center[2] - measured_center[2] * scale,
            ],
            scale: [scale, scale, scale],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{ColorComponent, RenderableComponent};

    #[test]
    fn uniform_fit_centers_and_scales_bounds() {
        let bounds = Aabb {
            min: [-0.25, -0.5, -0.05],
            max: [0.25, 0.5, 0.05],
        };

        let transform = BoundsSystem::fit_aabb_uniform(&bounds, [-1.0, -1.0, -0.1, 1.0, 1.0, 0.1])
            .expect("fit transform");

        assert_eq!(transform.translation, [0.0, 0.0, 0.0]);
        assert_eq!(transform.scale, [2.0, 2.0, 2.0]);
    }

    #[test]
    fn renderable_measure_wraps_calculated_bounds() {
        let mut world = World::default();
        let render_assets = RenderAssets::new();

        let root = world.add_component(TransformComponent::new());
        let shape = world.add_component(TransformComponent::new().with_scale(0.25, 0.5, 0.1));
        let color = world.add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
        let renderable = world.add_component(RenderableComponent::cube());

        world.add_child(root, shape).expect("attach shape");
        world.add_child(shape, color).expect("attach color");
        world
            .add_child(color, renderable)
            .expect("attach renderable");

        let measure = BoundsSystem::measure_renderable_subtree_bounds(&world, &render_assets, root);
        let RenderableBoundsMeasure::Measured(bounds) = measure else {
            panic!("expected measured bounds");
        };

        assert!((bounds.width() - 0.25).abs() < 1e-4);
        assert!((bounds.height() - 0.5).abs() < 1e-4);
        assert!((bounds.depth() - 0.1).abs() < 1e-4);
    }
}
