use std::collections::{HashMap, HashSet};

use crate::engine::ecs::component::{
    ColorComponent, EmissiveComponent, OpacityComponent, OverlayComponent, RaycastableComponent,
    RenderableComponent, SelectableComponent, SerializeComponent, TransformComponent,
    ZoneComponent,
};
use crate::engine::ecs::system::model::collision_types::CollisionShape;
use crate::engine::ecs::system::resolve_zone_frame;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::RenderAssets;

const MARKER_COLOR: [f32; 4] = [1.0, 0.15, 0.15, 1.0];
const MARKER_OPACITY: f32 = 0.22;
const MARKER_EMISSIVE: f32 = 1.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneVisualizationRequest {
    pub scope_roots: Vec<ComponentId>,
}

#[derive(Debug, Clone, Copy)]
struct ZoneMarker {
    root: ComponentId,
    frame: ComponentId,
    shape: CollisionShape,
}

/// Union-combines editor-owned requests and retains runtime-only zone overlays.
#[derive(Debug, Default)]
pub struct ZoneVisualizationSystem {
    requests: HashMap<ComponentId, ZoneVisualizationRequest>,
    markers: HashMap<ComponentId, ZoneMarker>,
}

impl ZoneVisualizationSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_request(&mut self, owner: ComponentId, scope_roots: Vec<ComponentId>) {
        self.requests
            .insert(owner, ZoneVisualizationRequest { scope_roots });
    }

    pub fn remove_request(&mut self, owner: ComponentId) {
        self.requests.remove(&owner);
    }

    pub fn requests(&self) -> &HashMap<ComponentId, ZoneVisualizationRequest> {
        &self.requests
    }

    /// Retained marker count for opt-in runtime growth diagnostics.
    pub fn marker_count(&self) -> usize {
        self.markers.len()
    }

    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        assets: &mut RenderAssets,
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

        if self.requests.is_empty() {
            remove_all_markers(&mut self.markers, emit);
            return;
        }

        let wanted: HashSet<_> = world
            .all_components()
            .filter(|zone_id| {
                world
                    .get_component_by_id_as::<ZoneComponent>(*zone_id)
                    .is_some_and(|zone| zone.enabled)
                    && self.requests.values().any(|request| {
                        request
                            .scope_roots
                            .iter()
                            .any(|root| is_descendant_or_self(world, *root, *zone_id))
                    })
            })
            .collect();

        for zone_id in self.markers.keys().copied().collect::<Vec<_>>() {
            if !wanted.contains(&zone_id) || world.get_component_record(zone_id).is_none() {
                remove_marker(&mut self.markers, zone_id, emit);
            }
        }

        for zone_id in wanted {
            let Some(zone) = world.get_component_by_id_as::<ZoneComponent>(zone_id) else {
                continue;
            };
            let shape = zone.shape.normalized();
            let Ok(frame) = resolve_zone_frame(world, zone_id, zone) else {
                remove_marker(&mut self.markers, zone_id, emit);
                continue;
            };
            let rebuild = self
                .markers
                .get(&zone_id)
                .is_some_and(|marker| marker.frame != frame || marker.shape != shape);
            if rebuild {
                remove_marker(&mut self.markers, zone_id, emit);
            }
            if !self.markers.contains_key(&zone_id) {
                let root = spawn_marker(world, assets, emit, frame, shape);
                self.markers
                    .insert(zone_id, ZoneMarker { root, frame, shape });
            }
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

fn remove_marker(
    markers: &mut HashMap<ComponentId, ZoneMarker>,
    zone_id: ComponentId,
    emit: &mut dyn SignalEmitter,
) {
    if let Some(marker) = markers.remove(&zone_id) {
        if marker.root != ComponentId::default() {
            emit.push_intent_now(
                marker.root,
                IntentValue::RemoveSubtree {
                    component_id: marker.root,
                },
            );
        }
    }
}

fn remove_all_markers(
    markers: &mut HashMap<ComponentId, ZoneMarker>,
    emit: &mut dyn SignalEmitter,
) {
    for marker in markers.drain().map(|(_, marker)| marker) {
        emit.push_intent_now(
            marker.root,
            IntentValue::RemoveSubtree {
                component_id: marker.root,
            },
        );
    }
}

fn shape_scale(shape: CollisionShape) -> [f32; 3] {
    match shape {
        CollisionShape::Cube { half_extents } => half_extents.map(|value| value * 2.0),
        CollisionShape::Sphere { radius } => [radius * 2.0; 3],
        CollisionShape::CapsuleY { .. } => [1.0; 3],
    }
}

fn spawn_marker(
    world: &mut World,
    assets: &mut RenderAssets,
    emit: &mut dyn SignalEmitter,
    frame: ComponentId,
    shape: CollisionShape,
) -> ComponentId {
    let scale = shape_scale(shape);
    let root = world.add_component_boxed_named(
        "zone_visualization_marker",
        Box::new(TransformComponent::new().with_scale(scale[0], scale[1], scale[2])),
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
    let _ = world.add_child(frame, root);
    world.init_component_tree(root, emit);
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::component::EditorUIComponent;
    use crate::engine::ecs::system::{SystemWorld, TransformSystem};
    use crate::engine::graphics::VisualWorld;

    #[test]
    fn marker_uses_resolved_frame_and_shape_local_scale() {
        let mut world = World::default();
        let owner = world.add_component(EditorUIComponent::new());
        let scope = world.add_component(TransformComponent::new());
        let frame = world.add_component(
            TransformComponent::new()
                .with_position(2.0, 3.0, 4.0)
                .with_rotation_quat([0.0, 0.0, 0.70710677, 0.70710677])
                .with_scale(2.0, 3.0, 4.0),
        );
        let zone = world.add_component(ZoneComponent::cube([1.0, 0.5, 0.25]));
        world.add_child(scope, frame).unwrap();
        world.add_child(frame, zone).unwrap();
        let mut assets = RenderAssets::new();
        let mut queue = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        world.init_component_tree(scope, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
        systems.transform_changed(&mut world, &mut visuals, scope);

        systems.zone_visualization.set_request(owner, vec![scope]);
        systems
            .zone_visualization
            .tick_with_queue(&mut world, &mut assets, &mut queue);
        systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
        systems.transform_changed(&mut world, &mut visuals, frame);

        let marker = systems.zone_visualization.markers[&zone].root;
        assert_eq!(world.parent_of(marker), Some(frame));
        assert_eq!(
            world
                .get_component_by_id_as::<TransformComponent>(marker)
                .unwrap()
                .transform
                .scale,
            [2.0, 1.0, 0.5]
        );
        let expected = crate::utils::math::mat4_mul(
            TransformSystem::world_model(&world, frame).unwrap(),
            world
                .get_component_by_id_as::<TransformComponent>(marker)
                .unwrap()
                .transform
                .model,
        );
        assert_eq!(TransformSystem::world_model(&world, marker), Some(expected));
    }

    #[test]
    fn disabled_unresolved_and_removed_owner_cleanup_markers() {
        let mut world = World::default();
        let owner = world.add_component(EditorUIComponent::new());
        let scope = world.add_component(TransformComponent::new());
        let zone = world.add_component(ZoneComponent::sphere(1.0));
        world.add_child(scope, zone).unwrap();
        let mut assets = RenderAssets::new();
        let mut queue = CommandQueue::new();
        let mut system = ZoneVisualizationSystem::default();

        system.set_request(owner, vec![scope]);
        system.tick_with_queue(&mut world, &mut assets, &mut queue);
        assert_eq!(system.markers.len(), 1);
        world
            .get_component_by_id_as_mut::<ZoneComponent>(zone)
            .unwrap()
            .enabled = false;
        system.tick_with_queue(&mut world, &mut assets, &mut queue);
        assert!(system.markers.is_empty());

        world.remove_component_leaf(owner).unwrap();
        system.tick_with_queue(&mut world, &mut assets, &mut queue);
        assert!(system.requests.is_empty());
    }
}
