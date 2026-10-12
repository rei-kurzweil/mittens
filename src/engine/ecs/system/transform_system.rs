use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{
    Camera2DComponent, Camera3DComponent, CollisionComponent, LayoutBoundsComponent,
    LayoutVisualPlacementComponent, RenderableComponent, TransformComponent,
    TransformOperatorComponent,
};
use crate::engine::ecs::system::CollisionSystem;
use crate::engine::ecs::system::System;
use crate::engine::ecs::system::TransformOperatorSystem;
use crate::engine::ecs::system::bounds_system::BoundsSystem;
use crate::engine::graphics::VisualWorld;
use crate::engine::graphics::primitives::InstanceHandle;
use crate::engine::transform::{TransformMatrix, TransformTrs, TransformTrsError};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformAccessError {
    NotTransform(ComponentId),
    InvalidWorldMatrix(TransformTrsError),
    InvalidDesiredTrs(TransformTrsError),
    UnresolvedTransformParent(ComponentId),
    TransformOperatorOwned(ComponentId),
    SingularEffectiveParent(ComponentId),
    InvalidLocalMatrix(TransformTrsError),
}

impl std::fmt::Display for TransformAccessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotTransform(component) => {
                write!(formatter, "component {component:?} is not a transform")
            }
            Self::InvalidWorldMatrix(error) => {
                write!(formatter, "invalid world transform: {error}")
            }
            Self::InvalidDesiredTrs(error) => write!(formatter, "invalid desired TRS: {error}"),
            Self::UnresolvedTransformParent(component) => write!(
                formatter,
                "transform-parent boundary {component:?} has no resolved target"
            ),
            Self::TransformOperatorOwned(component) => write!(
                formatter,
                "world transform for {component:?} is owned by a transform operator boundary"
            ),
            Self::SingularEffectiveParent(component) => write!(
                formatter,
                "effective parent of transform {component:?} is not invertible"
            ),
            Self::InvalidLocalMatrix(error) => {
                write!(
                    formatter,
                    "world pose cannot be represented as local TRS: {error}"
                )
            }
        }
    }
}

impl std::error::Error for TransformAccessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidWorldMatrix(error)
            | Self::InvalidDesiredTrs(error)
            | Self::InvalidLocalMatrix(error) => Some(error),
            Self::NotTransform(_)
            | Self::UnresolvedTransformParent(_)
            | Self::TransformOperatorOwned(_)
            | Self::SingularEffectiveParent(_) => None,
        }
    }
}

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

    fn effective_local_model(
        world: &World,
        transform_id: ComponentId,
        authored_local: TransformMatrix,
    ) -> TransformMatrix {
        let placements: Vec<[f32; 3]> = world
            .children_of(transform_id)
            .iter()
            .filter_map(|&child| {
                world
                    .get_component_by_id_as::<LayoutVisualPlacementComponent>(child)
                    .map(|placement| placement.translation_parent_local)
            })
            .collect();
        debug_assert!(
            placements.len() <= 1,
            "a transform may have at most one layout visual placement"
        );
        let Some([x, y, z]) = placements.first().copied() else {
            return authored_local;
        };
        let translation = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [x, y, z, 1.0],
        ];
        Self::mat4_mul(translation, authored_local)
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
        transform_operator: &mut TransformOperatorSystem,
        camera_system: &mut crate::engine::ecs::system::CameraSystem,
        collision_system: &mut CollisionSystem,
    ) {
        // The root may already have been written by its caller. Descendant
        // sources are marked below only when their propagated matrix changes.
        transform_operator.mark_translation_source_changed(root_node);
        let mut stack: Vec<(ComponentId, TransformMatrix)> = vec![(root_node, inherited_world)];
        while let Some((node, current_world)) = stack.pop() {
            let operator_evaluated =
                transform_operator.evaluate_operator(world, node, current_world);
            let (current_world, operator_outputs) = match operator_evaluated {
                Some((processed_world, outputs)) => (processed_world, Some(outputs)),
                None => (current_world, None),
            };
            // Camera-specific anchors own the effective cached matrix as well as the
            // inherited basis supplied to their direct children.
            if operator_outputs.is_some() {
                if let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(node) {
                    t.transform.matrix_world = current_world;
                }
            }

            let children: Vec<ComponentId> = match operator_outputs {
                Some(outputs) => outputs,
                None => world.children_of(node).to_vec(),
            };
            for child in children {
                let inherited = if let Some(tp) = world
                    .get_component_by_id_as::<TransformOperatorComponent>(child)
                    .and_then(|operator| operator.parent_settings())
                {
                    let Some(target) = tp.resolve_target_component(world) else {
                        continue;
                    };
                    let Some(target_world) = Self::world_model(world, target) else {
                        continue;
                    };
                    target_world
                } else {
                    current_world
                };
                let next_world = if let Some(authored_local) = world
                    .get_component_by_id_as::<TransformComponent>(child)
                    .map(|t| t.transform.model)
                {
                    let effective_local = Self::effective_local_model(world, child, authored_local);
                    let w = Self::mat4_mul(inherited, effective_local);
                    let mut changed = false;
                    if let Some(t) = world.get_component_by_id_as_mut::<TransformComponent>(child) {
                        changed = t.transform.matrix_world != w;
                        t.transform.matrix_world = w;
                    }
                    if changed {
                        transform_operator.mark_translation_source_changed(child);
                    }
                    Self::trace_resolved_visual_placement(world, child, inherited, w);
                    Self::trace_resolved_layout_background(world, child, inherited, w);
                    w
                } else {
                    inherited
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
                    Self::trace_render_model_handoff(world, visuals, child, handle, next_world);
                }

                stack.push((child, next_world));
            }
        }
    }

    fn trace_resolved_visual_placement(
        world: &World,
        transform_id: ComponentId,
        parent_world: TransformMatrix,
        resolved_world: TransformMatrix,
    ) {
        let Some(placement) = world.children_of(transform_id).iter().find_map(|&child| {
            world
                .get_component_by_id_as::<LayoutVisualPlacementComponent>(child)
                .copied()
        }) else {
            return;
        };

        if !crate::engine::ecs::system::layout::visual_placement_trace_enabled(world, transform_id)
        {
            return;
        }

        let [x, y, z] = placement.translation_parent_local;
        let placement_matrix = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [x, y, z, 1.0],
        ];
        let predicted_world = placement
            .source_bounds_parent_local
            .transformed(Self::mat4_mul(parent_world, placement_matrix));
        let actual_world =
            BoundsSystem::measure_cached_renderable_subtree_bounds(world, transform_id, |node| {
                world
                    .component_label(node)
                    .is_some_and(|label| label.starts_with("__"))
            })
            .map(|bounds_root_local| bounds_root_local.transformed(resolved_world));

        eprintln!(
            "[InspectLayout][visual-transform] visual={}({transform_id:?}) parent_world_pos={:?} resolved_world_pos={:?} predicted_world={predicted_world:?} actual_world={actual_world:?}",
            world.component_label(transform_id).unwrap_or("<unnamed>"),
            [parent_world[3][0], parent_world[3][1], parent_world[3][2]],
            [
                resolved_world[3][0],
                resolved_world[3][1],
                resolved_world[3][2]
            ],
        );
    }

    fn trace_resolved_layout_background(
        world: &World,
        transform_id: ComponentId,
        parent_world: TransformMatrix,
        resolved_world: TransformMatrix,
    ) {
        if world.component_label(transform_id) != Some("__bg") {
            return;
        }
        let Some(item_id) = world.parent_of(transform_id) else {
            return;
        };
        if !crate::engine::ecs::system::layout::visual_placement_trace_enabled(world, item_id) {
            return;
        }
        let Some(layout_bounds) = world.children_of(item_id).iter().find_map(|&child| {
            world
                .get_component_by_id_as::<LayoutBoundsComponent>(child)
                .copied()
        }) else {
            return;
        };

        let expected_world = layout_bounds.padding_local.transformed(parent_world);
        let actual_world =
            BoundsSystem::measure_cached_renderable_subtree_bounds(world, transform_id, |_| false)
                .map(|bounds_local| bounds_local.transformed(resolved_world));

        eprintln!(
            "[InspectLayout][background-transform] item={}({item_id:?}) background={transform_id:?} expected_world={expected_world:?} actual_world={actual_world:?}",
            world.component_label(item_id).unwrap_or("<unnamed>"),
        );
    }

    fn trace_render_model_handoff(
        world: &World,
        visuals: &VisualWorld,
        renderable_id: ComponentId,
        handle: InstanceHandle,
        submitted_world: TransformMatrix,
    ) {
        let mut current = world.parent_of(renderable_id);
        let mut traced_root = None;
        while let Some(node) = current {
            let is_visual_root = world.children_of(node).iter().any(|&child| {
                world
                    .get_component_by_id_as::<LayoutVisualPlacementComponent>(child)
                    .is_some()
            });
            if is_visual_root || world.component_label(node) == Some("__bg") {
                traced_root = Some(node);
                break;
            }
            current = world.parent_of(node);
        }
        let Some(traced_root) = traced_root else {
            return;
        };
        if !crate::engine::ecs::system::layout::visual_placement_trace_enabled(world, traced_root) {
            return;
        }

        let stored_world = visuals
            .instance(handle)
            .map(|instance| instance.transform.model);
        let max_abs_diff = stored_world.map(|stored| {
            let mut max_diff = 0.0_f32;
            for column in 0..4 {
                for row in 0..4 {
                    max_diff =
                        max_diff.max((stored[column][row] - submitted_world[column][row]).abs());
                }
            }
            max_diff
        });

        eprintln!(
            "[InspectLayout][render-model] root={}({traced_root:?}) renderable={renderable_id:?} handle={handle:?} submitted_pos={:?} stored_pos={:?} max_abs_diff={max_abs_diff:?}",
            world.component_label(traced_root).unwrap_or("<unnamed>"),
            [
                submitted_world[3][0],
                submitted_world[3][1],
                submitted_world[3][2]
            ],
            stored_world.map(|stored| [stored[3][0], stored[3][1], stored[3][2]]),
        );
    }

    fn update_transform_parent_dependents(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        changed_component: ComponentId,
        transform_operator: &mut TransformOperatorSystem,
        camera_system: &mut crate::engine::ecs::system::CameraSystem,
        light_system: &mut crate::engine::ecs::system::LightSystem,
        collision_system: &mut CollisionSystem,
    ) {
        let dependents: Vec<ComponentId> = world
            .all_components()
            .filter(|&cid| {
                world
                    .get_component_by_id_as::<TransformOperatorComponent>(cid)
                    .and_then(|operator| operator.parent_settings())
                    .is_some()
            })
            .filter(|&cid| !Self::is_descendant_of(world, cid, changed_component))
            .filter(|&cid| {
                let target_transform = world
                    .get_component_by_id_as::<TransformOperatorComponent>(cid)
                    .and_then(|operator| operator.parent_settings())
                    .and_then(|tp| tp.resolve_target_component(world))
                    .and_then(|target| Self::nearest_transform_self_or_ancestor(world, target));
                target_transform.is_some_and(|target_transform| {
                    target_transform == changed_component
                        || Self::is_descendant_of(world, target_transform, changed_component)
                })
            })
            .collect();

        for dependent in dependents {
            let Some(inherited_world) = world
                .get_component_by_id_as::<TransformOperatorComponent>(dependent)
                .and_then(|operator| operator.parent_settings())
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
                transform_operator,
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

    fn update_inverse_local_dependents(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        changed_component: ComponentId,
        transform_operator: &mut TransformOperatorSystem,
        camera_system: &mut crate::engine::ecs::system::CameraSystem,
        light_system: &mut crate::engine::ecs::system::LightSystem,
        collision_system: &mut CollisionSystem,
    ) {
        let dependents = transform_operator.inverse_local_dependents(changed_component);

        for dependent in dependents {
            let Some(inherited_world) = transform_operator.inverse_local_cached_input(dependent)
            else {
                continue;
            };
            self.propagate_subtree(
                world,
                visuals,
                dependent,
                inherited_world,
                transform_operator,
                camera_system,
                collision_system,
            );
            let children = world.children_of(dependent).to_vec();
            for child in children {
                light_system.transform_changed(world, visuals, child);
            }
        }
    }

    fn update_translation_sample_dependents(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        changed: ComponentId,
        operators: &mut TransformOperatorSystem,
        cameras: &mut crate::engine::ecs::system::CameraSystem,
        lights: &mut crate::engine::ecs::system::LightSystem,
        collisions: &mut CollisionSystem,
    ) {
        // Sources changed through routing/other operators participate too.
        // Cycle validation rejects sample feedback; the bound also prevents
        // malformed mixed legacy routing from causing an endless refresh.
        for wave in 0..64 {
            let pending =
                operators.translation_sample_dependents(world, (wave == 0).then_some(changed));
            if pending.is_empty() {
                break;
            }
            for owner in pending {
                if operators.translation_sample_input(owner).is_some() {
                    let (root, input) = Self::operator_refresh_basis(world, owner, operators);
                    self.propagate_subtree(
                        world, visuals, root, input, operators, cameras, collisions,
                    );
                    lights.transform_changed(world, visuals, owner);
                }
            }
            // A direct source should seed the first wave only.
            if !operators.has_changed_translation_sources() {
                break;
            }
        }
    }

    fn operator_refresh_basis(
        world: &World,
        owner: ComponentId,
        operators: &TransformOperatorSystem,
    ) -> (ComponentId, TransformMatrix) {
        if operators.translation_sample_input_is_current(world, owner) {
            if let Some(input) = operators.translation_sample_input(owner) {
                return (owner, input);
            }
        }
        if world
            .get_component_by_id_as::<TransformOperatorComponent>(owner)
            .is_some_and(|operator| operator.inverse_local_settings().is_some())
        {
            if let Some(input) = operators.inverse_local_cached_input(owner) {
                return (owner, input);
            }
        }
        // New/reparented operators must honor intervening operators before
        // composing downstream offsets (e.g. a fork's rotation/scale drops).
        let mut root = owner;
        while let Some(parent) = world.parent_of(root) {
            if let Some(transform) = world.get_component_by_id_as::<TransformComponent>(parent) {
                return (root, transform.transform.matrix_world);
            }
            root = parent;
        }
        (root, Self::mat4_identity())
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

    fn strict_world_model(
        world: &World,
        component: ComponentId,
    ) -> Result<TransformMatrix, TransformAccessError> {
        world
            .get_component_by_id_as::<TransformComponent>(component)
            .map(|transform| transform.transform.matrix_world)
            .ok_or(TransformAccessError::NotTransform(component))
    }

    /// Read the cached world-space translation of an actual transform component.
    pub fn world_translation(
        world: &World,
        component: ComponentId,
    ) -> Result<[f32; 3], TransformAccessError> {
        let matrix = Self::strict_world_model(world, component)?;
        let translation = [matrix[3][0], matrix[3][1], matrix[3][2]];
        if translation.into_iter().all(f32::is_finite) {
            Ok(translation)
        } else {
            Err(TransformAccessError::InvalidWorldMatrix(
                TransformTrsError::NonFiniteValue,
            ))
        }
    }

    /// Decompose one coherent cached world matrix into strict TRS channels.
    pub fn world_trs(
        world: &World,
        component: ComponentId,
    ) -> Result<TransformTrs, TransformAccessError> {
        TransformTrs::from_matrix(Self::strict_world_model(world, component)?)
            .map_err(TransformAccessError::InvalidWorldMatrix)
    }

    pub fn world_rotation_quat_xyzw(
        world: &World,
        component: ComponentId,
    ) -> Result<[f32; 4], TransformAccessError> {
        Ok(Self::world_trs(world, component)?.rotation_quat_xyzw)
    }

    pub fn world_scale(
        world: &World,
        component: ComponentId,
    ) -> Result<[f32; 3], TransformAccessError> {
        Ok(Self::world_trs(world, component)?.scale)
    }

    /// Convert a desired world-space TRS into the local TRS which produces it.
    ///
    /// The effective parent follows the same structural, TransformParent, and
    /// transform operator boundaries used by transform propagation. A transform
    /// directly owned by a transform operator is rejected because changing its
    /// local value cannot promise a stable world result.
    pub fn world_to_local_trs(
        world: &World,
        transform_operator: &TransformOperatorSystem,
        component: ComponentId,
        desired_world: TransformTrs,
    ) -> Result<TransformTrs, TransformAccessError> {
        if world
            .get_component_by_id_as::<TransformComponent>(component)
            .is_none()
        {
            return Err(TransformAccessError::NotTransform(component));
        }

        let desired_matrix = desired_world
            .to_matrix()
            .map_err(TransformAccessError::InvalidDesiredTrs)?;
        let mut probe = component;
        let effective_parent = loop {
            let Some(parent) = world.parent_of(probe) else {
                break Self::mat4_identity();
            };
            if let Some(transform) = world.get_component_by_id_as::<TransformComponent>(parent) {
                break transform.transform.matrix_world;
            }
            if let Some(transform_parent) = world
                .get_component_by_id_as::<TransformOperatorComponent>(parent)
                .and_then(|operator| operator.parent_settings())
            {
                let Some(target) = transform_parent.resolve_target_component(world) else {
                    return Err(TransformAccessError::UnresolvedTransformParent(parent));
                };
                let Some(target_world) = Self::world_model(world, target) else {
                    return Err(TransformAccessError::UnresolvedTransformParent(parent));
                };
                break target_world;
            }
            if transform_operator.is_operator_boundary(world, parent) {
                return Err(TransformAccessError::TransformOperatorOwned(component));
            }
            probe = parent;
        };

        let inverse_parent = crate::utils::math::mat4_inverse(effective_parent)
            .ok_or(TransformAccessError::SingularEffectiveParent(component))?;
        let local_matrix = crate::utils::math::mat4_mul(inverse_parent, desired_matrix);
        TransformTrs::from_matrix(local_matrix).map_err(TransformAccessError::InvalidLocalMatrix)
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
        transform_operator: &mut TransformOperatorSystem,
        camera_system: &mut crate::engine::ecs::system::CameraSystem,
        light_system: &mut crate::engine::ecs::system::LightSystem,
        collision_system: &mut CollisionSystem,
    ) {
        if world
            .get_component_by_id_as::<TransformOperatorComponent>(component)
            .is_some_and(|operator| {
                operator.inverse_local_settings().is_some()
                    || operator.sample_translation_settings().is_some()
            })
        {
            let (root, inherited_world) =
                Self::operator_refresh_basis(world, component, transform_operator);
            self.propagate_subtree(
                world,
                visuals,
                root,
                inherited_world,
                transform_operator,
                camera_system,
                collision_system,
            );
            let children = world.children_of(component).to_vec();
            for child in children {
                light_system.transform_changed(world, visuals, child);
            }
            self.update_translation_sample_dependents(
                world,
                visuals,
                component,
                transform_operator,
                camera_system,
                light_system,
                collision_system,
            );
            return;
        }
        if let Some(inherited_world) = world
            .get_component_by_id_as::<TransformOperatorComponent>(component)
            .and_then(|operator| operator.parent_settings())
            .and_then(|tp| tp.resolve_target_component(world))
            .and_then(|target| Self::world_model(world, target))
        {
            self.propagate_subtree(
                world,
                visuals,
                component,
                inherited_world,
                transform_operator,
                camera_system,
                collision_system,
            );
            light_system.transform_changed(world, visuals, component);
        }
        if world
            .get_component_by_id_as::<TransformOperatorComponent>(component)
            .and_then(|operator| operator.parent_settings())
            .is_some()
        {
            self.update_translation_sample_dependents(
                world,
                visuals,
                component,
                transform_operator,
                camera_system,
                light_system,
                collision_system,
            );
            return;
        }
        // Recompute cached world matrices for this transform and all descendant transforms.
        // Then update any dependent renderables/cameras under the subtree.

        // Build the chain of ancestor transforms (including `component`) from root -> leaf,
        // stopping at any TC whose immediate non-TC ancestors include a transform operator
        // boundary node. Such a TC's `matrix_world` is owned by that boundary's computed
        // basis: walking further up and recomputing from local matrices would bypass the
        // operator and overwrite its output with incorrect values. Instead we treat that TC
        // as the chain root and start the chain-world from its cached `matrix_world`.
        let mut transform_chain: Vec<ComponentId> = Vec::new();
        let mut operator_boundary = false; // true → transform_chain[0] is operator-managed
        let mut transform_parent_basis = None;
        let mut transform_parent_boundary = false;
        let mut cur = component;
        'chain: loop {
            if world
                .get_component_by_id_as::<TransformComponent>(cur)
                .is_some()
            {
                transform_chain.push(cur);
                // Check whether this TC sits directly under a transform operator boundary node
                // (i.e., any non-TC node on the path to the next TC ancestor changes the
                // inherited world basis). If so, its world is operator-managed — stop here.
                let mut probe = cur;
                while let Some(p) = world.parent_of(probe) {
                    if let Some(tp) = world
                        .get_component_by_id_as::<TransformOperatorComponent>(p)
                        .and_then(|operator| operator.parent_settings())
                    {
                        transform_parent_boundary = true;
                        transform_parent_basis = tp
                            .resolve_target_component(world)
                            .and_then(|target| Self::world_model(world, target));
                        break 'chain;
                    }
                    if transform_operator.is_operator_boundary(world, p) {
                        operator_boundary = true;
                        break 'chain;
                    }
                    if world
                        .get_component_by_id_as::<TransformComponent>(p)
                        .is_some()
                    {
                        break; // reached next TC ancestor without finding a operator boundary
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

        // An unresolved follower boundary cannot safely fall back to structural ancestry:
        // retain the follower's last effective world matrix until its target resolves.
        if transform_parent_boundary && transform_parent_basis.is_none() {
            return;
        }

        // Compute world matrices down the chain and write them back.
        //
        // If `operator_boundary` is set, transform_chain[0] is under a transform operator.
        // Its cached `matrix_world` is stream-managed — use it as the starting world and
        // skip recomputing it from local matrices (which would bypass the operator).
        let (start_idx, mut chain_world) = if let Some(basis) = transform_parent_basis {
            (0, basis)
        } else if operator_boundary && !transform_chain.is_empty() {
            let cached = world
                .get_component_by_id_as::<TransformComponent>(transform_chain[0])
                .map(|t| t.transform.matrix_world)
                .unwrap_or_else(Self::mat4_identity);
            (1, cached)
        } else {
            (0, Self::mat4_identity())
        };
        for tid in transform_chain[start_idx..].iter().copied() {
            let authored_local = match world
                .get_component_by_id_as::<TransformComponent>(tid)
                .map(|t| t.transform.model)
            {
                Some(m) => m,
                None => continue,
            };
            let effective_local = Self::effective_local_model(world, tid, authored_local);
            chain_world = Self::mat4_mul(chain_world, effective_local);
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
            transform_operator,
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
            transform_operator,
            camera_system,
            light_system,
            collision_system,
        );
        self.update_inverse_local_dependents(
            world,
            visuals,
            component,
            transform_operator,
            camera_system,
            light_system,
            collision_system,
        );
        self.update_translation_sample_dependents(
            world,
            visuals,
            component,
            transform_operator,
            camera_system,
            light_system,
            collision_system,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{TransformAccessError, TransformSystem};
    use crate::engine::ecs::World;
    use crate::engine::ecs::component::{
        ComponentRef, LayoutVisualPlacementComponent, TransformComponent,
        TransformOperatorComponent,
    };
    use crate::engine::ecs::system::{
        CameraSystem, CollisionSystem, LightSystem, TransformOperatorSystem,
    };
    use crate::engine::graphics::VisualWorld;
    use crate::engine::graphics::bounds::Aabb;
    use crate::engine::transform::{TransformTrs, TransformTrsError};

    #[test]
    fn translation_sample_refreshes_cross_tree_chains_and_reparented_inputs() {
        use crate::engine::ecs::component::transform_operator::TranslationAxes;
        use crate::engine::ecs::system::SystemWorld;
        let mut world = World::default();
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let source =
            world.add_component(TransformComponent::new().with_position(100.0, 200.0, 300.0));
        let input_a =
            world.add_component(TransformComponent::new().with_position(10.0, 20.0, 30.0));
        let sample_a = world.add_component(
            TransformOperatorComponent::sample_translation(ComponentRef::Guid(
                world.get_component_record(source).unwrap().guid,
            ))
            .with_axes(TranslationAxes::Y),
        );
        let a = world.add_component(TransformComponent::new().with_position(0.0, 1.0, 0.0));
        world.add_child(input_a, sample_a).unwrap();
        world.add_child(sample_a, a).unwrap();
        let input_b =
            world.add_component(TransformComponent::new().with_position(40.0, 50.0, 60.0));
        let sample_b = world.add_component(
            TransformOperatorComponent::sample_translation(ComponentRef::Guid(
                world.get_component_record(a).unwrap().guid,
            ))
            .with_axes(TranslationAxes::Y),
        );
        let b = world.add_component(TransformComponent::new());
        world.add_child(input_b, sample_b).unwrap();
        world.add_child(sample_b, b).unwrap();
        for root in [source, input_a, input_b] {
            systems.transform_changed(&mut world, &mut visuals, root);
        }
        assert_eq!(
            TransformSystem::world_position(&world, b),
            Some([40.0, 201.0, 60.0])
        );
        world
            .get_component_by_id_as_mut::<TransformComponent>(source)
            .unwrap()
            .set_position(&mut systems.rx, 100.0, 500.0, 300.0);
        systems.transform_changed(&mut world, &mut visuals, source);
        assert_eq!(
            TransformSystem::world_position(&world, a),
            Some([10.0, 501.0, 30.0])
        );
        assert_eq!(
            TransformSystem::world_position(&world, b),
            Some([40.0, 501.0, 60.0])
        );
        // A topology edit must not reuse the sampler's old inherited X/Z.
        world.add_child(input_b, sample_a).unwrap();
        systems.transform_changed(&mut world, &mut visuals, sample_a);
        assert_eq!(
            TransformSystem::world_position(&world, a),
            Some([40.0, 501.0, 60.0])
        );
    }

    #[test]
    fn inverse_local_source_edit_refreshes_the_compensated_branch() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut transform_system = TransformSystem::new();
        let mut transform_operator = TransformOperatorSystem::new();
        let mut camera_system = CameraSystem::new();
        let mut light_system = LightSystem::new();
        let mut collision_system = CollisionSystem::new();

        let driver = world.add_component(TransformComponent::new().with_position(10.0, 0.0, 0.0));
        let source = world.add_component(TransformComponent::new().with_position(0.0, 3.0, 0.0));
        let source_guid = world.get_component_record(source).unwrap().guid;
        let operator = world.add_component(TransformOperatorComponent::inverse_local(
            ComponentRef::Guid(source_guid),
        ));
        let car = world.add_component(TransformComponent::new().with_position(0.0, 0.1, 0.0));
        world.add_child(driver, source).unwrap();
        world.add_child(driver, operator).unwrap();
        world.add_child(operator, car).unwrap();

        transform_system.transform_changed(
            &mut world,
            &mut visuals,
            driver,
            &mut transform_operator,
            &mut camera_system,
            &mut light_system,
            &mut collision_system,
        );
        assert_eq!(
            TransformSystem::world_position(&world, car),
            Some([10.0, -2.9, 0.0])
        );

        world
            .get_component_by_id_as_mut::<TransformComponent>(source)
            .unwrap()
            .transform = TransformComponent::new()
            .with_position(0.0, 17.0, 0.0)
            .transform;
        transform_system.transform_changed(
            &mut world,
            &mut visuals,
            source,
            &mut transform_operator,
            &mut camera_system,
            &mut light_system,
            &mut collision_system,
        );
        assert_eq!(
            TransformSystem::world_position(&world, car),
            Some([10.0, -16.9, 0.0])
        );
    }

    #[test]
    fn transform_parent_updates_cross_tree_child_when_target_changes() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut transform_system = TransformSystem::new();
        let mut transform_operator = TransformOperatorSystem::new();
        let mut camera_system = CameraSystem::new();
        let mut light_system = LightSystem::new();
        let mut collision_system = CollisionSystem::new();

        let source = world.add_component(TransformComponent::new().with_position(1.0, 0.0, 0.0));
        let dependent_root = world.add_component(TransformComponent::new());
        let transform_parent =
            world.add_component(TransformOperatorComponent::parent().with_source(
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
            &mut transform_operator,
            &mut camera_system,
            &mut light_system,
            &mut collision_system,
        );

        assert_eq!(
            TransformSystem::world_position(&world, child),
            Some([1.0, 2.0, 0.0])
        );
    }

    #[test]
    fn strict_world_getters_read_only_transform_components() {
        let mut world = World::default();
        let root = world.add_component(
            TransformComponent::new()
                .with_position(1.0, 2.0, 3.0)
                .with_rotation_quat([
                    0.0,
                    std::f32::consts::FRAC_1_SQRT_2,
                    0.0,
                    std::f32::consts::FRAC_1_SQRT_2,
                ])
                .with_scale(2.0, 3.0, 4.0),
        );
        let root_transform = world
            .get_component_by_id_as_mut::<TransformComponent>(root)
            .unwrap();
        root_transform.transform.matrix_world = root_transform.transform.model;
        let non_transform = world.add_component(TransformOperatorComponent::parent());

        assert_eq!(
            TransformSystem::world_translation(&world, root),
            Ok([1.0, 2.0, 3.0])
        );
        let world_trs = TransformSystem::world_trs(&world, root).unwrap();
        assert_eq!(world_trs.translation, [1.0, 2.0, 3.0]);
        for (actual, expected) in world_trs.scale.into_iter().zip([2.0, 3.0, 4.0]) {
            assert!((actual - expected).abs() < 1e-5);
        }
        let world_scale = TransformSystem::world_scale(&world, root).unwrap();
        for (actual, expected) in world_scale.into_iter().zip([2.0, 3.0, 4.0]) {
            assert!((actual - expected).abs() < 1e-5);
        }
        assert_eq!(
            TransformSystem::world_rotation_quat_xyzw(&world, root),
            Ok(world_trs.rotation_quat_xyzw)
        );
        assert_eq!(
            TransformSystem::world_trs(&world, non_transform),
            Err(TransformAccessError::NotTransform(non_transform))
        );
    }

    #[test]
    fn layout_visual_placement_composes_outside_authored_local_transform() {
        let mut world = World::default();
        let mut visuals = VisualWorld::default();
        let mut transform_system = TransformSystem::new();
        let mut transform_operator = TransformOperatorSystem::new();
        let mut camera_system = CameraSystem::new();
        let mut light_system = LightSystem::new();
        let mut collision_system = CollisionSystem::new();

        let root = world.add_component(TransformComponent::new());
        let visual = world.add_component(
            TransformComponent::new()
                .with_position(1.0, 2.0, 0.0)
                .with_scale(2.0, 3.0, 1.0),
        );
        let placement = world.add_component(LayoutVisualPlacementComponent::new(
            Aabb {
                min: [0.0, 0.0, 0.0],
                max: [2.0, 3.0, 1.0],
            },
            [4.0, -6.0, 0.0],
        ));
        world.add_child(root, visual).unwrap();
        world.add_child(visual, placement).unwrap();

        transform_system.transform_changed(
            &mut world,
            &mut visuals,
            visual,
            &mut transform_operator,
            &mut camera_system,
            &mut light_system,
            &mut collision_system,
        );

        let authored = world
            .get_component_by_id_as::<TransformComponent>(visual)
            .unwrap();
        assert_eq!(authored.transform.translation, [1.0, 2.0, 0.0]);
        assert_eq!(authored.transform.scale, [2.0, 3.0, 1.0]);
        assert_eq!(
            TransformSystem::world_position(&world, visual),
            Some([5.0, -4.0, 0.0])
        );
    }

    #[test]
    fn strict_world_trs_rejects_shear_but_translation_remains_exact() {
        let mut world = World::default();
        let transform = world.add_component(TransformComponent::new());
        let matrix = {
            let component = world
                .get_component_by_id_as_mut::<TransformComponent>(transform)
                .unwrap();
            component.transform.matrix_world[1][0] = 0.25;
            component.transform.matrix_world[3] = [4.0, 5.0, 6.0, 1.0];
            component.transform.matrix_world
        };

        assert_eq!(
            TransformSystem::world_translation(&world, transform),
            Ok([4.0, 5.0, 6.0])
        );
        assert_eq!(
            TransformSystem::world_trs(&world, transform),
            Err(TransformAccessError::InvalidWorldMatrix(
                TransformTrsError::ShearNotRepresentable
            ))
        );
        assert_eq!(
            TransformTrs::from_matrix(matrix),
            Err(TransformTrsError::ShearNotRepresentable)
        );
    }

    #[test]
    fn world_to_local_trs_compensates_for_the_effective_parent() {
        let mut world = World::default();
        let parent = world.add_component(
            TransformComponent::new()
                .with_position(10.0, 1.0, -2.0)
                .with_rotation_euler(0.0, std::f32::consts::FRAC_PI_2, 0.0),
        );
        let child = world.add_component(TransformComponent::new());
        world.add_child(parent, child).unwrap();
        let parent_component = world
            .get_component_by_id_as_mut::<TransformComponent>(parent)
            .unwrap();
        parent_component.transform.matrix_world = parent_component.transform.model;

        let desired = TransformTrs::new([4.0, 5.0, 6.0], [0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]);
        let local = TransformSystem::world_to_local_trs(
            &world,
            &TransformOperatorSystem::new(),
            child,
            desired,
        )
        .unwrap();
        let recomposed = crate::utils::math::mat4_mul(
            world
                .get_component_by_id_as::<TransformComponent>(parent)
                .unwrap()
                .transform
                .matrix_world,
            local.to_matrix().unwrap(),
        );
        let desired_matrix = desired.to_matrix().unwrap();
        for (actual, expected) in recomposed
            .into_iter()
            .flatten()
            .zip(desired_matrix.into_iter().flatten())
        {
            assert!((actual - expected).abs() < 1e-4, "{actual} != {expected}");
        }
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
