use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{
    Camera2DComponent, Camera3DComponent, CollisionComponent, RenderableComponent,
    TransformComponent,
};
use crate::engine::ecs::system::CollisionSystem;
use crate::engine::ecs::system::System;
use crate::engine::graphics::VisualWorld;
use crate::engine::graphics::primitives::TransformMatrix;
use crate::engine::user_input::InputState;

/// System responsible for syncing `TransformComponent` changes into `VisualWorld`.
///
/// Key points:
/// - An entity can have multiple TransformComponents.
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
        camera_system: &mut crate::engine::ecs::system::CameraSystem,
        light_system: &mut crate::engine::ecs::system::LightSystem,
        collision_system: &mut CollisionSystem,
    ) {
        // Recompute cached world matrices for this transform and all descendant transforms.
        // Then update any dependent renderables/cameras under the subtree.

        // Build the chain of ancestor transforms (including `component`) from root -> leaf,
        // and update cached `matrix_world` along that chain so we can start propagation from
        // a correct world matrix even if registration order was odd.
        let mut transform_chain: Vec<ComponentId> = Vec::new();
        let mut cur = component;
        loop {
            if world
                .get_component_by_id_as::<TransformComponent>(cur)
                .is_some()
            {
                transform_chain.push(cur);
            }
            let Some(parent) = world.parent_of(cur) else {
                break;
            };
            cur = parent;
        }
        transform_chain.reverse();

        // Compute world matrices down the chain and write them back.
        let mut chain_world = Self::mat4_identity();
        for tid in transform_chain.iter().copied() {
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

        // DFS the component subtree. `current_world` is the world matrix of the nearest
        // TransformComponent ancestor along the path.
        let mut stack: Vec<(ComponentId, TransformMatrix)> = vec![(component, root_world)];
        while let Some((node, current_world)) = stack.pop() {
            let children: Vec<ComponentId> = world.children_of(node).to_vec();
            for child in children {
                // If we encounter a TransformComponent, update its cached world matrix and
                // use it for its subtree.
                let next_world = if let Some(t) =
                    world.get_component_by_id_as_mut::<TransformComponent>(child)
                {
                    let w = Self::mat4_mul(current_world, t.transform.model);
                    t.transform.matrix_world = w;
                    w
                } else {
                    current_world
                };

                // If `node` is a TransformComponent and it directly parents a camera component,
                // update that camera (the update methods themselves guard on active handle).
                if world
                    .get_component_by_id_as::<TransformComponent>(node)
                    .is_some()
                {
                    if world
                        .get_component_by_id_as::<Camera2DComponent>(child)
                        .is_some()
                    {
                        camera_system.update_camera_2d_from_parent_transform(
                            world, visuals, child, node,
                        );
                    }

                    if world
                        .get_component_by_id_as::<Camera3DComponent>(child)
                        .is_some()
                    {
                        camera_system.update_camera_3d_from_parent_transform(
                            world, visuals, child, node,
                        );
                    }

                    // If this transform directly parents a CollisionComponent, update it.
                    if world
                        .get_component_by_id_as::<CollisionComponent>(child)
                        .is_some()
                    {
                        collision_system.update_from_transform(world, child, node);
                    }
                }

                // Update VisualWorld model matrices for any renderables in the subtree.
                if let Some(handle) = world
                    .get_component_by_id_as::<RenderableComponent>(child)
                    .and_then(|r| r.get_handle())
                {
                    visuals.update_model(handle, next_world);
                }

                stack.push((child, next_world));
            }
        }

        // If any point lights live under this transform, update their world-space position.
        // LightSystem uses TransformSystem::world_position(), which now reads cached matrices.
        light_system.transform_changed(world, visuals, component);
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
