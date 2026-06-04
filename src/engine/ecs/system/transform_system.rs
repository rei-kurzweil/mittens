use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{
    Camera2DComponent, Camera3DComponent, CollisionComponent, RenderableComponent,
    TransformComponent, TransformParentComponent,
};
use crate::engine::ecs::system::CollisionSystem;
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::TransformStreamSystem;
use crate::engine::graphics::VisualWorld;
use crate::engine::graphics::primitives::TransformMatrix;
use crate::engine::user_input::InputState;

/// System responsible for
/// syncing `TransformComponent` changes into `VisualWorld`.
/// applying side effects to direct children of transforms
/// and calculating world matrices for descendant transform components.
///
/// Key points:
/// - A `TransformComponent` can parent other transforms to form groups.
/// - Instances in `VisualWorld` are created per `RenderableComponent` under transforms.
#[derive(Debug, Default)]
pub struct TransformSystem;

impl TransformSystem {
    pub fn new() -> Self {
        Self
    }

    fn mat4_identity() -> TransformMatrix {
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ]
    }

    fn mat4_mul(a: TransformMatrix, b: TransformMatrix) -> TransformMatrix {
        let mut out = [[0.0f32; 4]; 4];
        for c in 0..4 {
            for r in 0..4 {
                out[c][r] =
                    a[0][r] * b[c][0] + a[1][r] * b[c][1] + a[2][r] * b[c][2] + a[3][r] * b[c][3];
            }
        }
        out
    }

    fn is_descendant_of(world: &World, mut node: ComponentId, ancestor: ComponentId) -> bool {
        while let Some(parent) = world.parent_of(node) {
            if parent == ancestor {
                return true;
            }
            node = parent;
        }
        false
    }

    fn nearest_transform_self_or_ancestor(world: &World, cid: ComponentId) -> Option<ComponentId> {
        if world
            .get_component_by_id_as::<TransformComponent>(cid)
            .is_some()
        {
            return Some(cid);
        }
        let mut cur = cid;
        while let Some(parent) = world.parent_of(cur) {
            if world
                .get_component_by_id_as::<TransformComponent>(parent)
                .is_some()
            {
                return Some(parent);
            }
            cur = parent;
        }
        None
    }

    fn propagate_subtree(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        root_node: ComponentId,
        inherited_world: TransformMatrix,
        transform_stream_system: &mut TransformStreamSystem,
        camera_system: &mut crate::engine::ecs::system::CameraSystem,
        collision_system: &mut CollisionSystem,
    ) {
        let mut stack: Vec<(ComponentId, TransformMatrix)> = vec![(root_node, inherited_world)];
        while let Some((node, current_world)) = stack.pop() {
            let stream_evaluated =
                transform_stream_system.evaluate_stream_node(world, node, current_world);
            let (current_world, stream_output_roots) = match stream_evaluated {
                Some((processed_world, outputs)) => (processed_world, Some(outputs)),
                None => (current_world, None),
            };

            let children: Vec<ComponentId> = match stream_output_roots {
                Some(outputs) if !outputs.is_empty() => outputs,
                _ => world.children_of(node).to_vec(),
            };
            for child in children {
                let next_world = if let Some(t) =
                    world.get_component_by_id_as_mut::<TransformComponent>(child)
                {
                    let w = Self::mat4_mul(current_world, t.transform.model);
                    t.transform.matrix_world = w;
                    w
                } else {
                    current_world
                };

                if world
                    .get_component_by_id_as::<TransformComponent>(node)
                    .is_some()
                {
                    if world
                        .get_component_by_id_as::<Camera2DComponent>(child)
                        .is_some()
                    {
                        camera_system
                            .update_camera_2d_from_parent_transform(world, visuals, child, node);
                    }

                    if world
                        .get_component_by_id_as::<Camera3DComponent>(child)
                        .is_some()
                    {
                        camera_system
                            .update_camera_3d_from_parent_transform(world, visuals, child, node);
                    }

                    if world
                        .get_component_by_id_as::<CollisionComponent>(child)
                        .is_some()
                    {
                        collision_system.update_from_transform(world, child, node);
                    }
                }

                if let Some(handle) = world
                    .get_component_by_id_as::<RenderableComponent>(child)
                    .and_then(|r| r.get_handle())
                {
                    visuals.update_model(handle, next_world);
                }

                stack.push((child, next_world));
            }
        }
    }

    fn update_transform_parent_dependents(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        changed_component: ComponentId,
        transform_stream_system: &mut TransformStreamSystem,
        camera_system: &mut crate::engine::ecs::system::CameraSystem,
        light_system: &mut crate::engine::ecs::system::LightSystem,
        collision_system: &mut CollisionSystem,
    ) {
        let dependents: Vec<ComponentId> = world
            .all_components()
            .filter(|&cid| {
                world
                    .get_component_by_id_as::<TransformParentComponent>(cid)
                    .is_some()
            })
            .filter(|&cid| !Self::is_descendant_of(world, cid, changed_component))
            .filter(|&cid| {
                world
                    .get_component_by_id_as::<TransformParentComponent>(cid)
                    .and_then(|tp| tp.resolve_target_component(world))
                    .and_then(|target| Self::nearest_transform_self_or_ancestor(world, target))
                    == Some(changed_component)
            })
            .collect();

        for dependent in dependents {
            let Some(inherited_world) = world
                .get_component_by_id_as::<TransformParentComponent>(dependent)
                .and_then(|tp| tp.resolve_target_component(world))
                .and_then(|target| Self::world_model(world, target))
            else {
                continue;
            };

            self.propagate_subtree(
                world,
                visuals,
                dependent,
                inherited_world,
                transform_stream_system,
                camera_system,
                collision_system,
            );

            let child_transform_roots: Vec<ComponentId> = world
                .children_of(dependent)
                .iter()
                .copied()
                .filter(|&cid| {
                    world
                        .get_component_by_id_as::<TransformComponent>(cid)
                        .is_some()
                })
                .collect();
            for root in child_transform_roots {
                light_system.transform_changed(world, visuals, root);
            }
        }
    }

    /// Compute the world-space model matrix for a component by walking up the component tree
    /// and multiplying all ancestor `TransformComponent` model matrices.
    ///
    /// Returns `None` if there are no ancestor transforms.
    pub fn world_model(world: &World, cid: ComponentId) -> Option<TransformMatrix> {
        // If this node is a transform, its cached world matrix is the answer.
        if let Some(t) = world.get_component_by_id_as::<TransformComponent>(cid) {
            return Some(t.transform.matrix_world);
        }

        // Otherwise, return the cached world matrix of the nearest ancestor TransformComponent.
        let mut cur = cid;
        while let Some(parent) = world.parent_of(cur) {
            if let Some(t) = world.get_component_by_id_as::<TransformComponent>(parent) {
                return Some(t.transform.matrix_world);
            }
            cur = parent;
        }
        None
    }

    /// Compute the world-space position (translation) for a component.
    pub fn world_position(world: &World, cid: ComponentId) -> Option<[f32; 3]> {
        let model = Self::world_model(world, cid)?;
        // Column-major translation lives in the last column.
        let p = model[3];
        Some([p[0], p[1], p[2]])
    }

    /// Called by TransformComponent when its values change.
    ///
    /// This updates camera translation if the transform has a Camera2D child, and updates
    /// VisualWorld instance model matrices for any `RenderableComponent` descendants.
    pub fn transform_changed(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        component: ComponentId,
        transform_stream_system: &mut TransformStreamSystem,
        camera_system: &mut crate::engine::ecs::system::CameraSystem,
        light_system: &mut crate::engine::ecs::system::LightSystem,
        collision_system: &mut CollisionSystem,
    ) {
        // Recompute cached world matrices for this transform and all descendant transforms.
        // Then update any dependent renderables/cameras under the subtree.

        // Build the chain of ancestor transforms (including `component`) from root -> leaf,
        // stopping at any TC whose immediate non-TC ancestors include a transform-stream
        // boundary node. Such a TC's `matrix_world` is owned by that boundary's computed
        // basis: walking further up and recomputing from local matrices would bypass the
        // operator and overwrite its output with incorrect values. Instead we treat that TC
        // as the chain root and start the chain-world from its cached `matrix_world`.
        let mut transform_chain: Vec<ComponentId> = Vec::new();
        let mut stream_boundary = false; // true → transform_chain[0] is stream-operator-managed
        let mut cur = component;
        'chain: loop {
            if world
                .get_component_by_id_as::<TransformComponent>(cur)
                .is_some()
            {
                transform_chain.push(cur);
                // Check whether this TC sits directly under a transform-stream boundary node
                // (i.e., any non-TC node on the path to the next TC ancestor changes the
                // inherited world basis). If so, its world is operator-managed — stop here.
                let mut probe = cur;
                while let Some(p) = world.parent_of(probe) {
                    if transform_stream_system.is_transform_stream_boundary(world, p) {
                        stream_boundary = true;
                        break 'chain;
                    }
                    if world
                        .get_component_by_id_as::<TransformComponent>(p)
                        .is_some()
                    {
                        break; // reached next TC ancestor without finding a stream boundary
                    }
                    probe = p;
                }
            }
            let Some(parent) = world.parent_of(cur) else {
                break;
            };
            cur = parent;
        }
        transform_chain.reverse();

        // Compute world matrices down the chain and write them back.
        //
        // If `stream_boundary` is set, transform_chain[0] is under a stream operator.
        // Its cached `matrix_world` is stream-managed — use it as the starting world and
        // skip recomputing it from local matrices (which would bypass the operator).
        let (start_idx, mut chain_world) = if stream_boundary && !transform_chain.is_empty() {
            let cached = world
                .get_component_by_id_as::<TransformComponent>(transform_chain[0])
                .map(|t| t.transform.matrix_world)
                .unwrap_or_else(Self::mat4_identity);
            (1, cached)
        } else {
            (0, Self::mat4_identity())
        };
        for tid in transform_chain[start_idx..].iter().copied() {
            let local = match world
                .get_component_by_id_as::<TransformComponent>(tid)
                .map(|t| t.transform.model)
            {
                Some(m) => m,
                None => continue,
            };
            chain_world = Self::mat4_mul(chain_world, local);
            if let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(tid) {
                t.transform.matrix_world = chain_world;
            }
        }

        // Start propagation from this transform's world matrix.
        let root_world = match world
            .get_component_by_id_as::<TransformComponent>(component)
            .map(|t| t.transform.matrix_world)
        {
            Some(m) => m,
            None => return,
        };

        self.propagate_subtree(
            world,
            visuals,
            component,
            root_world,
            transform_stream_system,
            camera_system,
            collision_system,
        );

        // If any point lights live under this transform, update their world-space position.
        // LightSystem uses TransformSystem::world_position(), which now reads cached matrices.
        light_system.transform_changed(world, visuals, component);
        self.update_transform_parent_dependents(
            world,
            visuals,
            component,
            transform_stream_system,
            camera_system,
            light_system,
            collision_system,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::TransformSystem;
    use crate::engine::ecs::World;
    use crate::engine::ecs::component::{TransformComponent, TransformParentComponent};
    use crate::engine::ecs::system::{
        CameraSystem, CollisionSystem, LightSystem, TransformStreamSystem,
    };
    use crate::engine::graphics::VisualWorld;

    #[test]
    fn transform_parent_updates_cross_tree_child_when_target_changes() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut transform_system = TransformSystem::new();
        let mut transform_stream_system = TransformStreamSystem::new();
        let mut camera_system = CameraSystem::new();
        let mut light_system = LightSystem::new();
        let mut collision_system = CollisionSystem::new();

        let source = world.add_component(TransformComponent::new().with_position(1.0, 0.0, 0.0));
        let dependent_root = world.add_component(TransformComponent::new());
        let transform_parent =
            world.add_component(TransformParentComponent::new().with_target_source(
                crate::engine::ecs::component::ComponentRef::Query("#source".to_string()),
            ));
        let child = world.add_component(TransformComponent::new().with_position(0.0, 2.0, 0.0));

        world.get_component_record_mut(source).unwrap().name = "source".to_string();
        world.add_child(dependent_root, transform_parent).unwrap();
        world.add_child(transform_parent, child).unwrap();

        transform_system.transform_changed(
            &mut world,
            &mut visuals,
            source,
            &mut transform_stream_system,
            &mut camera_system,
            &mut light_system,
            &mut collision_system,
        );

        assert_eq!(
            TransformSystem::world_position(&world, child),
            Some([1.0, 2.0, 0.0])
        );
    }
}

impl System for TransformSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
        // No-op. Transform updates are event-driven via `transform_changed`.
    }
}
