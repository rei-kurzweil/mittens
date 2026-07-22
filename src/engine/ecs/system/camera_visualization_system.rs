use std::collections::{HashMap, HashSet};

use crate::engine::ecs::component::{
    Camera3DComponent, CameraXRComponent, ColorComponent, EmissiveComponent, OpacityComponent,
    OverlayComponent, RaycastableComponent, RenderableComponent, SerializeComponent,
    TransformComponent,
};
use crate::engine::ecs::system::CameraSystem;
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraVisualizationRequest {
    pub scope_roots: Vec<ComponentId>,
}

/// Union-combines editor-owned requests and maintains runtime-only camera icons.
#[derive(Debug, Default)]
pub struct CameraVisualizationSystem {
    requests: HashMap<ComponentId, CameraVisualizationRequest>,
    markers: HashMap<ComponentId, ComponentId>,
}

impl CameraVisualizationSystem {
    pub fn set_request(&mut self, owner: ComponentId, scope_roots: Vec<ComponentId>) {
        self.requests
            .insert(owner, CameraVisualizationRequest { scope_roots });
    }

    pub fn remove_request(&mut self, owner: ComponentId) {
        self.requests.remove(&owner);
    }

    pub fn requests(&self) -> &HashMap<ComponentId, CameraVisualizationRequest> {
        &self.requests
    }

    pub fn marker_for_camera(&self, camera: ComponentId) -> Option<ComponentId> {
        self.markers.get(&camera).copied()
    }

    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        cameras: &CameraSystem,
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

        let active_window = cameras.active_window_camera_component();
        let active_xr = cameras.active_xr_camera_component();
        let wanted: HashSet<ComponentId> = world
            .all_components()
            .filter(|id| {
                let is_3d = world
                    .get_component_by_id_as::<Camera3DComponent>(*id)
                    .is_some();
                let is_xr = world
                    .get_component_by_id_as::<CameraXRComponent>(*id)
                    .is_some();
                (is_3d || is_xr)
                    && Some(*id) != active_window
                    && Some(*id) != active_xr
                    && self.requests.values().any(|request| {
                        request
                            .scope_roots
                            .iter()
                            .any(|root| is_descendant_or_self(world, *root, *id))
                    })
                    && authored_camera_transform(world, *id).is_some()
            })
            .collect();

        for camera in self.markers.keys().copied().collect::<Vec<_>>() {
            let marker_exists = self.markers.get(&camera).is_some_and(|marker| {
                world.get_component_record(*marker).is_some()
                    && authored_camera_transform(world, camera)
                        .is_some_and(|owner| world.parent_of(*marker) == Some(owner))
            });
            if !wanted.contains(&camera) || !marker_exists {
                if let Some(marker) = self.markers.remove(&camera)
                    && world.get_component_record(marker).is_some()
                {
                    emit.push_intent_now(
                        marker,
                        IntentValue::RemoveSubtree {
                            component_ids: vec![marker],
                        },
                    );
                }
            }
        }

        for camera in wanted {
            if self.markers.contains_key(&camera) {
                continue;
            }
            let Some(owner) = authored_camera_transform(world, camera) else {
                continue;
            };
            let marker = spawn_camera_marker(world, emit);
            if world.add_child(owner, marker).is_ok() {
                world.init_component_tree(marker, emit);
                self.markers.insert(camera, marker);
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

fn authored_camera_transform(world: &World, camera: ComponentId) -> Option<ComponentId> {
    let mut current = world.parent_of(camera);
    while let Some(id) = current {
        if world
            .get_component_by_id_as::<TransformComponent>(id)
            .is_some()
        {
            return Some(id);
        }
        current = world.parent_of(id);
    }
    None
}

fn spawn_camera_marker(world: &mut World, _emit: &mut dyn SignalEmitter) -> ComponentId {
    const COLOR: [f32; 4] = [0.94, 0.43, 0.12, 0.72];
    let root = world.add_component_boxed_named(
        "camera_visualization_marker",
        Box::new(TransformComponent::new().with_scale(0.32, 0.32, 0.32)),
    );
    let serialize = world.add_component(SerializeComponent::off());
    let overlay = world.add_component(OverlayComponent::new());
    let body_t = world.add_component(
        TransformComponent::new()
            .with_position(0.0, 0.0, 0.30)
            .with_scale(0.70, 0.70, 0.70),
    );
    let body = world.add_component(RenderableComponent::cube());
    let cone_t = world.add_component(
        TransformComponent::new()
            .with_position(0.0, 0.0, -0.42)
            .with_scale(0.42, 0.42, 0.72),
    );
    let cone = world.add_component(RenderableComponent::cone());
    for renderable in [body, cone] {
        let raycastable = world.add_component(RaycastableComponent::enabled());
        let color =
            world.add_component(ColorComponent::rgba(COLOR[0], COLOR[1], COLOR[2], COLOR[3]));
        let opacity = world.add_component(OpacityComponent::new().with_opacity(COLOR[3]));
        let emissive = world.add_component(EmissiveComponent::new(0.35));
        let _ = world.add_child(renderable, raycastable);
        let _ = world.add_child(renderable, color);
        let _ = world.add_child(renderable, opacity);
        let _ = world.add_child(renderable, emissive);
    }
    let _ = world.add_child(root, serialize);
    let _ = world.add_child(root, overlay);
    let _ = world.add_child(overlay, body_t);
    let _ = world.add_child(body_t, body);
    let _ = world.add_child(overlay, cone_t);
    let _ = world.add_child(cone_t, cone);
    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::component::EditorComponent;
    use crate::engine::graphics::VisualWorld;

    #[test]
    fn request_marks_only_inactive_cameras_in_scope_including_disabled() {
        let mut world = World::default();
        let editor = world.add_component(EditorComponent::new());
        let active_t = world.add_component(TransformComponent::new());
        let active = world.add_component(Camera3DComponent::default());
        let disabled_t = world.add_component(TransformComponent::new());
        let mut disabled_camera = Camera3DComponent::default();
        disabled_camera.enabled = false;
        let disabled = world.add_component(disabled_camera);
        let outside_t = world.add_component(TransformComponent::new());
        let outside = world.add_component(CameraXRComponent::off());
        world.add_child(editor, active_t).unwrap();
        world.add_child(active_t, active).unwrap();
        world.add_child(editor, disabled_t).unwrap();
        world.add_child(disabled_t, disabled).unwrap();
        world.add_child(outside_t, outside).unwrap();

        let mut cameras = CameraSystem::default();
        let mut visuals = VisualWorld::default();
        cameras.register_camera(&mut world, &mut visuals, active);
        let mut system = CameraVisualizationSystem::default();
        let owner = world.add_component(TransformComponent::new());
        system.set_request(owner, vec![editor]);
        let mut queue = CommandQueue::new();
        system.tick_with_queue(&mut world, &cameras, &mut queue);

        assert!(system.marker_for_camera(active).is_none());
        assert!(system.marker_for_camera(disabled).is_some());
        assert!(system.marker_for_camera(outside).is_none());
        assert_eq!(
            world.parent_of(system.marker_for_camera(disabled).unwrap()),
            Some(disabled_t)
        );
    }
}
