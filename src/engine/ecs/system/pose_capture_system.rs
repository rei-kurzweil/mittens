use crate::engine::ecs::{ComponentId, World, IntentValue};
use crate::engine::ecs::component::{PoseCaptureComponent, PoseCapturePoseComponent, PoseCaptureLibraryComponent, PoseTargetRef, PoseBoneEntry, TransformComponent};
use crate::engine::ecs::rx::SignalEmitter;

#[derive(Debug, Default)]
pub struct PoseCaptureSystem;

impl PoseCaptureSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn handle_capture(&self, world: &mut World, emit: &mut dyn SignalEmitter, target: ComponentId, pose_name: Option<String>) {
        if world.get_component_by_id_as::<PoseCaptureComponent>(target).is_none() {
            println!("[PoseCaptureSystem] target {:?} has no PoseCaptureComponent", target);
            return;
        };

        // 1. Resolve library or create it
        let library_id = self.ensure_library(world, target, emit);

        // 2. Capture all transforms in subtree
        let transforms = world.find_all_components(target, "Transform");
        let mut entries = Vec::new();
        for tc_id in transforms {
            if let Some(tc) = world.get_component_by_id_as::<TransformComponent>(tc_id) {
                if let Some(path) = self.get_subtree_path(world, target, tc_id) {
                    entries.push(PoseBoneEntry {
                        path,
                        translation: tc.transform.translation,
                        rotation: tc.transform.rotation,
                        scale: tc.transform.scale,
                    });
                }
            }
        }

        // 3. Create PoseCapturePoseComponent
        let name = pose_name.unwrap_or_else(|| format!("pose_{}", world.children_of(library_id).len()));
        println!("[PoseCaptureSystem] capturing pose '{}' for target {:?} with {} entries", name, target, entries.len());
        
        let pose = PoseCapturePoseComponent::new(name, PoseTargetRef::Query("TODO".to_string()), entries);
        let pose_id = world.add_component(pose);
        if let Err(e) = world.add_child(library_id, pose_id) {
            println!("[PoseCaptureSystem] failed to add pose to library: {}", e);
            return;
        }
        world.init_component_tree(pose_id, emit);

        // 4. Emit event
        use crate::engine::ecs::EventSignal;
        emit.push_event(
            target,
            EventSignal::DataEvent {
                name: "pose_captured".to_string(),
                payload: Some(pose_id),
            },
        );
    }

    pub fn handle_apply(&self, world: &mut World, emit: &mut dyn SignalEmitter, target: ComponentId, pose_id: ComponentId) {
        let Some(pose) = world.get_component_by_id_as::<PoseCapturePoseComponent>(pose_id) else {
            println!("[PoseCaptureSystem] pose {:?} has no PoseCapturePoseComponent", pose_id);
            return;
        };

        println!("[PoseCaptureSystem] applying pose '{}' to target {:?}", pose.name, target);

        for entry in &pose.entries {
            if let Some(tc_id) = self.resolve_path(world, target, &entry.path) {
                emit.push_intent_now(tc_id, IntentValue::UpdateTransform {
                    component_ids: vec![tc_id],
                    translation: entry.translation,
                    rotation_quat_xyzw: entry.rotation,
                    scale: entry.scale,
                });
            }
        }
    }

    fn ensure_library(&self, world: &mut World, target: ComponentId, emit: &mut dyn SignalEmitter) -> ComponentId {
        // Find existing library as child of target
        for &child in world.children_of(target) {
            if world.get_component_by_id_as::<PoseCaptureLibraryComponent>(child).is_some() {
                return child;
            }
        }

        // Create new library
        let library = PoseCaptureLibraryComponent::new(PoseTargetRef::Query("TODO".to_string()));
        let library_id = world.add_component(library);
        let _ = world.add_child(target, library_id);
        world.init_component_tree(library_id, emit);
        library_id
    }

    fn get_subtree_path(&self, world: &World, root: ComponentId, target: ComponentId) -> Option<String> {
        if target == root {
            return Some("".to_string());
        }
        let mut path = Vec::new();
        let mut curr = target;
        while curr != root {
            let name = world.component_label(curr).unwrap_or("node");
            path.push(name.to_string());
            let Some(parent) = world.parent_of(curr) else {
                return None; // Not in subtree
            };
            curr = parent;
        }
        path.reverse();
        Some(path.join("/"))
    }

    fn resolve_path(&self, world: &World, root: ComponentId, path: &str) -> Option<ComponentId> {
        if path.is_empty() {
            return Some(root);
        }
        let parts: Vec<&str> = path.split('/').collect();
        let mut curr = root;
        for part in parts {
            let mut found = false;
            for &child in world.children_of(curr) {
                if world.component_label(child) == Some(part) {
                    curr = child;
                    found = true;
                    break;
                }
            }
            if !found {
                return None;
            }
        }
        Some(curr)
    }
}
