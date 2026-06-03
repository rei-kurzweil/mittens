use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{RenderableComponent, TransformComponent};
use crate::engine::graphics::bounds::{mat4_identity, mat4_mul, mesh_local_aabb, Aabb};
use crate::engine::graphics::RenderAssets;

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
                    .and_then(|cpu_mesh| Aabb::from_points(&cpu_mesh.vertices.iter().map(|v| v.pos).collect::<Vec<[f32; 3]>>()))
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
}
