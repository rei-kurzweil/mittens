use std::collections::{HashMap, HashSet};

use crate::engine::ecs::component::{
    AvatarControlComponent, CollisionComponent, ColorComponent, EmissiveComponent, GLTFComponent,
    OpacityComponent, OverlayComponent, RaycastableComponent, RenderableComponent,
    SelectableComponent, SerializeComponent, TransformComponent,
};
use crate::engine::ecs::system::collision_shape_resolver::resolve_collision_shape;
use crate::engine::ecs::system::model::collision_types::CollisionShape;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::{RenderAssets, VisualWorld};

const MARKER_COLOR: [f32; 4] = [0.2, 0.8, 1.0, 1.0];
const MARKER_OPACITY: f32 = 0.25;
const MARKER_EMISSIVE: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollisionVisualizationMode {
    All,
    GltfOwned,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollisionVisualizationRequest {
    pub scope_roots: Vec<ComponentId>,
    pub mode: CollisionVisualizationMode,
}

#[derive(Debug, Clone, Copy)]
struct CollisionMarker {
    root: ComponentId,
    shape: CollisionShape,
}

/// Union-combines editor-owned requests and maintains runtime-only collider overlays.
#[derive(Debug, Default)]
pub struct CollisionVisualizationSystem {
    requests: HashMap<ComponentId, CollisionVisualizationRequest>,
    markers: HashMap<ComponentId, CollisionMarker>,
}

impl CollisionVisualizationSystem {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set_request(
        &mut self,
        owner: ComponentId,
        scope_roots: Vec<ComponentId>,
        mode: CollisionVisualizationMode,
    ) {
        self.requests
            .insert(owner, CollisionVisualizationRequest { scope_roots, mode });
    }
    pub fn remove_request(&mut self, owner: ComponentId) {
        self.requests.remove(&owner);
    }
    pub fn requests(&self) -> &HashMap<ComponentId, CollisionVisualizationRequest> {
        &self.requests
    }

    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        _visuals: &mut VisualWorld,
        render_assets: &mut RenderAssets,
        emit: &mut dyn SignalEmitter,
    ) {
        self.requests.retain(|owner, request| {
            if world.get_component_record(*owner).is_none() {
                return false;
            }
            request
                .scope_roots
                .retain(|root| world.get_component_record(*root).is_some());
            true
        });

        let wanted: HashSet<_> = world
            .all_components()
            .filter(|collision| {
                world
                    .get_component_by_id_as::<CollisionComponent>(*collision)
                    .is_some()
                    && world.parent_of(*collision).is_some_and(|parent| {
                        world
                            .get_component_by_id_as::<TransformComponent>(parent)
                            .is_some()
                    })
                    && self
                        .requests
                        .values()
                        .any(|request| request_matches(world, request, *collision))
            })
            .collect();

        for collision in self.markers.keys().copied().collect::<Vec<_>>() {
            let valid =
                wanted.contains(&collision) && world.get_component_record(collision).is_some();
            if !valid {
                if let Some(marker) = self.markers.remove(&collision) {
                    emit.push_intent_now(
                        marker.root,
                        IntentValue::RemoveSubtree {
                            component_ids: vec![marker.root],
                        },
                    );
                }
            }
        }

        for collision in wanted {
            let shape =
                resolve_collision_shape(world, collision).unwrap_or_else(CollisionShape::CUBE);
            if self
                .markers
                .get(&collision)
                .is_some_and(|marker| marker.shape != shape)
            {
                let marker = self.markers.remove(&collision).unwrap();
                emit.push_intent_now(
                    marker.root,
                    IntentValue::RemoveSubtree {
                        component_ids: vec![marker.root],
                    },
                );
            }
            if !self.markers.contains_key(&collision) {
                let root = spawn_marker(world, render_assets, emit, shape);
                self.markers
                    .insert(collision, CollisionMarker { root, shape });
            }
            let marker = self.markers[&collision];
            let position = world
                .parent_of(collision)
                .and_then(|transform| TransformSystem::world_position(world, transform))
                .unwrap_or([0.0; 3]);
            let scale = shape_scale(shape);
            emit.push_intent_now(
                marker.root,
                IntentValue::UpdateTransform {
                    component_ids: vec![marker.root],
                    translation: position,
                    rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                    scale,
                },
            );
        }
    }
}

fn is_descendant_or_self(world: &World, ancestor: ComponentId, node: ComponentId) -> bool {
    let mut current = Some(node);
    while let Some(id) = current {
        if id == ancestor {
            return true;
        }
        current = world.parent_of(id);
    }
    false
}

fn gltf_or_avatar_owned(world: &World, collision: ComponentId) -> bool {
    let mut current = Some(collision);
    while let Some(id) = current {
        if world.get_component_by_id_as::<GLTFComponent>(id).is_some()
            || world
                .get_component_by_id_as::<AvatarControlComponent>(id)
                .is_some()
        {
            return true;
        }
        current = world.parent_of(id);
    }
    world.all_components().any(|id| {
        world
            .get_component_by_id_as::<AvatarControlComponent>(id)
            .and_then(|avc| avc.capsule_transform_id)
            .is_some_and(|capsule| is_descendant_or_self(world, capsule, collision))
    })
}

fn request_matches(
    world: &World,
    request: &CollisionVisualizationRequest,
    collision: ComponentId,
) -> bool {
    request
        .scope_roots
        .iter()
        .any(|root| is_descendant_or_self(world, *root, collision))
        && (request.mode == CollisionVisualizationMode::All
            || gltf_or_avatar_owned(world, collision))
}

fn shape_scale(shape: CollisionShape) -> [f32; 3] {
    match shape {
        CollisionShape::Cube { half_extents } => half_extents.map(|v| v * 2.0),
        CollisionShape::Sphere { radius } => [radius * 2.0; 3],
        CollisionShape::CapsuleY { .. } => [1.0; 3],
    }
}

fn spawn_marker(
    world: &mut World,
    assets: &mut RenderAssets,
    emit: &mut dyn SignalEmitter,
    shape: CollisionShape,
) -> ComponentId {
    let root = world.add_component_boxed_named(
        "collision_visualization_marker",
        Box::new(TransformComponent::new()),
    );
    let serialize = world.add_component(SerializeComponent::off());
    let selectable = world.add_component(SelectableComponent::off());
    let overlay = world.add_component(OverlayComponent::new());
    let renderable = match shape {
        CollisionShape::Cube { .. } => RenderableComponent::cube(),
        CollisionShape::Sphere { .. } => RenderableComponent::sphere(),
        CollisionShape::CapsuleY {
            radius,
            half_segment,
        } => RenderableComponent::from_cpu_mesh_handle(
            assets.capsule_y_mesh(radius, half_segment),
            crate::engine::graphics::primitives::MaterialHandle::TOON_MESH,
        ),
    };
    let renderable = world.add_component(renderable);
    let raycastable = world.add_component(RaycastableComponent::disabled());
    let color = world.add_component(ColorComponent::rgba(
        MARKER_COLOR[0],
        MARKER_COLOR[1],
        MARKER_COLOR[2],
        MARKER_COLOR[3],
    ));
    let opacity = world.add_component(OpacityComponent::new().with_opacity(MARKER_OPACITY));
    let emissive = world.add_component(EmissiveComponent::new(MARKER_EMISSIVE));
    let _ = world.add_child(root, serialize);
    let _ = world.add_child(root, selectable);
    let _ = world.add_child(root, overlay);
    let _ = world.add_child(overlay, renderable);
    let _ = world.add_child(renderable, raycastable);
    let _ = world.add_child(renderable, color);
    let _ = world.add_child(renderable, opacity);
    let _ = world.add_child(renderable, emissive);
    world.init_component_tree(root, emit);
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::CollisionShapeComponent;
    use crate::engine::ecs::system::SystemWorld;
    use crate::engine::ecs::CommandQueue;

    #[test]
    fn request_spawns_styled_marker_and_removal_cleans_it_up() {
        let mut world = World::default();
        let owner = world.add_component(crate::engine::ecs::component::EditorUIComponent::new());
        let scope = world.add_component(TransformComponent::new());
        let transform = world.add_component(TransformComponent::new().with_position(1.0, 2.0, 3.0));
        let collision = world.add_component(CollisionComponent::KINEMATIC());
        let shape = world.add_component(CollisionShapeComponent::capsule_y(0.3, 0.8));
        world.add_child(scope, transform).unwrap();
        world.add_child(transform, collision).unwrap();
        world.add_child(collision, shape).unwrap();
        let mut visuals = VisualWorld::default();
        let mut assets = RenderAssets::new();
        let mut queue = CommandQueue::new();
        let mut systems = SystemWorld::default();
        world.init_component_tree(scope, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
        systems.collision_visualization.set_request(
            owner,
            vec![scope],
            CollisionVisualizationMode::All,
        );
        systems.collision_visualization.tick_with_queue(
            &mut world,
            &mut visuals,
            &mut assets,
            &mut queue,
        );
        systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
        let marker = world
            .all_components()
            .find(|id| world.component_label(*id) == Some("collision_visualization_marker"))
            .unwrap();
        let marker_transform = world
            .get_component_by_id_as::<TransformComponent>(marker)
            .unwrap();
        assert_eq!(marker_transform.transform.translation, [1.0, 2.0, 3.0]);
        assert_eq!(marker_transform.transform.rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(marker_transform.transform.scale, [1.0; 3]);
        assert!(world.all_components().any(|id| {
            is_descendant_or_self(&world, marker, id)
                && world
                    .get_component_by_id_as::<OpacityComponent>(id)
                    .is_some()
        }));
        systems.collision_visualization.remove_request(owner);
        systems.collision_visualization.tick_with_queue(
            &mut world,
            &mut visuals,
            &mut assets,
            &mut queue,
        );
        systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
        assert!(world.get_component_record(marker).is_none());
    }

    #[test]
    fn gltf_owned_includes_generated_avatar_capsule_and_excludes_unrelated_colliders() {
        let mut world = World::default();
        let owner = world.add_component(crate::engine::ecs::component::EditorUIComponent::new());
        let scope = world.add_component(TransformComponent::new());
        let avc = world.add_component(AvatarControlComponent::new());
        let capsule_transform = world.add_component(TransformComponent::new());
        let capsule_collision = world.add_component(CollisionComponent::KINEMATIC());
        let capsule_shape = world.add_component(CollisionShapeComponent::capsule_y(0.25, 0.75));
        let unrelated_transform = world.add_component(TransformComponent::new());
        let unrelated_collision = world.add_component(CollisionComponent::KINEMATIC());
        world.add_child(scope, avc).unwrap();
        world.add_child(scope, capsule_transform).unwrap();
        world
            .add_child(capsule_transform, capsule_collision)
            .unwrap();
        world.add_child(capsule_collision, capsule_shape).unwrap();
        world.add_child(scope, unrelated_transform).unwrap();
        world
            .add_child(unrelated_transform, unrelated_collision)
            .unwrap();
        world
            .get_component_by_id_as_mut::<AvatarControlComponent>(avc)
            .unwrap()
            .capsule_transform_id = Some(capsule_transform);

        let mut visuals = VisualWorld::default();
        let mut assets = RenderAssets::new();
        let mut queue = CommandQueue::new();
        let mut systems = SystemWorld::default();
        systems.collision_visualization.set_request(
            owner,
            vec![scope],
            CollisionVisualizationMode::GltfOwned,
        );
        systems.collision_visualization.tick_with_queue(
            &mut world,
            &mut visuals,
            &mut assets,
            &mut queue,
        );
        assert!(systems
            .collision_visualization
            .markers
            .contains_key(&capsule_collision));
        assert!(!systems
            .collision_visualization
            .markers
            .contains_key(&unrelated_collision));
    }
}
