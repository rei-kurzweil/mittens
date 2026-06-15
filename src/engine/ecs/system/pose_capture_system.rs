use crate::engine::ecs::{ComponentId, IntentValue, World};
use crate::engine::ecs::component::{
    EditorComponent, PoseBoneEntry, PoseCaptureComponent, PoseCaptureLibraryComponent,
    PoseCapturePoseComponent, PoseTargetRef, TransformComponent,
};
use crate::engine::ecs::rx::SignalEmitter;
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct PoseCaptureSystem;

impl PoseCaptureSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn handle_capture(&self, world: &mut World, emit: &mut dyn SignalEmitter, target: ComponentId, pose_name: Option<String>) {
        let targets = self.resolve_capture_targets(world, target);
        if targets.is_empty() {
            println!(
                "[PoseCaptureSystem] no PoseCaptureComponent targets resolved from {:?}",
                target
            );
            return;
        }

        for target in targets {
            let pose_id = match self.capture_target_pose(world, emit, target, pose_name.clone()) {
                Some(pose_id) => pose_id,
                None => continue,
            };

            use crate::engine::ecs::EventSignal;
            emit.push_event(
                target,
                EventSignal::DataEvent {
                    name: "pose_captured".to_string(),
                    payload: Some(pose_id),
                },
            );
        }
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

    fn capture_target_pose(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        target: ComponentId,
        pose_name: Option<String>,
    ) -> Option<ComponentId> {
        if world
            .get_component_by_id_as::<PoseCaptureComponent>(target)
            .is_none()
        {
            return None;
        }

        let library_id = self.ensure_library(world, target, emit);
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

        let existing_pose_count = world
            .children_of(library_id)
            .iter()
            .filter(|&&child| {
                world
                    .get_component_by_id_as::<PoseCapturePoseComponent>(child)
                    .is_some()
            })
            .count();
        let name = pose_name.unwrap_or_else(|| format!("pose_{existing_pose_count}"));
        println!(
            "[PoseCaptureSystem] capturing pose '{}' for target {:?} with {} entries",
            name,
            target,
            entries.len()
        );

        let pose = PoseCapturePoseComponent::new(
            name,
            PoseTargetRef::Query("TODO".to_string()),
            entries,
        );
        let pose_id = world.add_component(pose);
        if let Err(e) = world.add_child(library_id, pose_id) {
            println!("[PoseCaptureSystem] failed to add pose to library: {}", e);
            return None;
        }
        world.init_component_tree(pose_id, emit);
        Some(pose_id)
    }

    fn resolve_capture_targets(&self, world: &World, request_target: ComponentId) -> Vec<ComponentId> {
        let selected_targets = self.selected_pose_capture_targets(world);
        if !selected_targets.is_empty() {
            return selected_targets;
        }

        if let Some(target) = self.pose_capture_ancestor(world, request_target) {
            return vec![target];
        }

        world
            .all_components()
            .filter(|&id| world.get_component_by_id_as::<PoseCaptureComponent>(id).is_some())
            .collect()
    }

    fn selected_pose_capture_targets(&self, world: &World) -> Vec<ComponentId> {
        let mut seen = HashSet::new();
        let mut targets = Vec::new();
        for id in world.all_components() {
            let Some(editor) = world.get_component_by_id_as::<EditorComponent>(id) else {
                continue;
            };
            let Some(selected) = editor.selected else {
                continue;
            };
            let Some(target) = self.pose_capture_ancestor(world, selected) else {
                continue;
            };
            if seen.insert(target) {
                targets.push(target);
            }
        }
        targets
    }

    fn pose_capture_ancestor(&self, world: &World, start: ComponentId) -> Option<ComponentId> {
        let mut current = Some(start);
        while let Some(id) = current {
            if world.get_component_by_id_as::<PoseCaptureComponent>(id).is_some() {
                return Some(id);
            }
            current = world.parent_of(id);
        }
        None
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
