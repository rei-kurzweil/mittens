use crate::engine::ecs::component::{
    EditorComponent, PoseBoneEntry, PoseCaptureComponent, PoseCaptureLibraryComponent,
    PoseCapturePoseComponent, PoseTargetRef, TransformComponent,
};
use crate::engine::ecs::rx::SignalEmitter;
use crate::engine::ecs::{ComponentId, IntentValue, World};
use std::collections::HashSet;

#[derive(Debug, Default)]
pub struct PoseCaptureSystem;

impl PoseCaptureSystem {
    pub fn new() -> Self {
        Self
    }

    pub fn handle_capture(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        request_target: ComponentId,
        pose_name: Option<String>,
    ) {
        let targets = self.resolve_capture_targets(world, request_target);
        if targets.is_empty() {
            println!(
                "[PoseCaptureSystem] no PoseCaptureComponent targets resolved from {:?}",
                request_target
            );
            return;
        }

        for capture_target in targets {
            let pose_id =
                match self.capture_target_pose(world, emit, capture_target, pose_name.clone()) {
                    Some(pose_id) => pose_id,
                    None => continue,
                };

            use crate::engine::ecs::EventSignal;
            // Publish completion from the request origin. Editor panel handlers are
            // scoped to their panel tree; publishing from the captured GLTF target
            // leaves that tree and the new pose row never gets projected.
            emit.push_event(
                request_target,
                EventSignal::DataEvent {
                    name: "pose_captured".to_string(),
                    payload: Some(pose_id),
                },
            );
        }
    }

    pub fn handle_apply(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        target: ComponentId,
        pose_id: ComponentId,
    ) {
        let Some(pose) = world.get_component_by_id_as::<PoseCapturePoseComponent>(pose_id) else {
            println!(
                "[PoseCaptureSystem] pose {:?} has no PoseCapturePoseComponent",
                pose_id
            );
            return;
        };

        println!(
            "[PoseCaptureSystem] applying pose '{}' to target {:?}",
            pose.name, target
        );

        // Resolution is a validation phase. Do not enqueue any mutations unless
        // every query resolves uniquely inside this particular GLTF instance.
        let Some(gltf_id) = self.owning_gltf(world, target) else {
            println!("[PoseCaptureSystem] target {:?} has no owning GLTF", target);
            return;
        };
        let mut resolved = Vec::with_capacity(pose.entries.len());
        for entry in &pose.entries {
            let matches = self.resolve_joint_query(world, gltf_id, &entry.query);
            if matches.len() != 1 {
                println!(
                    "[PoseCaptureSystem] joint query '{}' resolved {} times; pose not applied",
                    entry.query,
                    matches.len()
                );
                return;
            }
            resolved.push((matches[0], entry));
        }
        for (tc_id, entry) in resolved {
            emit.push_intent_now(
                tc_id,
                IntentValue::UpdateTransform {
                    component_ids: vec![tc_id],
                    translation: entry.translation,
                    rotation_quat_xyzw: entry.rotation,
                    scale: entry.scale,
                },
            );
        }
    }

    fn ensure_library(
        &self,
        world: &mut World,
        target: ComponentId,
        emit: &mut dyn SignalEmitter,
    ) -> ComponentId {
        // Find existing library as child of target
        for &child in world.children_of(target) {
            if world
                .get_component_by_id_as::<PoseCaptureLibraryComponent>(child)
                .is_some()
            {
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
        let gltf_id = self.owning_gltf(world, target)?;
        let transforms = world
            .get_component_by_id_as::<crate::engine::ecs::component::GLTFComponent>(gltf_id)?
            .armature_joint_transforms
            .clone();
        let mut entries = Vec::new();
        let mut queries = HashSet::new();
        for tc_id in transforms {
            if let Some(tc) = world.get_component_by_id_as::<TransformComponent>(tc_id) {
                if let Some(label) = world
                    .component_label(tc_id)
                    .filter(|label| !label.is_empty())
                {
                    let query = format!("#{label}");
                    if !queries.insert(query.clone()) {
                        println!(
                            "[PoseCaptureSystem] duplicate captured joint query '{query}'; pose not captured"
                        );
                        return None;
                    }
                    entries.push(PoseBoneEntry {
                        query,
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

        let pose =
            PoseCapturePoseComponent::new(name, PoseTargetRef::Query("TODO".to_string()), entries);
        let pose_id = world.add_component(pose);
        if let Err(e) = world.add_child(library_id, pose_id) {
            println!("[PoseCaptureSystem] failed to add pose to library: {}", e);
            return None;
        }
        world.init_component_tree(pose_id, emit);
        Some(pose_id)
    }

    fn resolve_capture_targets(
        &self,
        world: &World,
        request_target: ComponentId,
    ) -> Vec<ComponentId> {
        let selected_targets = self.selected_pose_capture_targets(world);
        if !selected_targets.is_empty() {
            return selected_targets;
        }

        if let Some(target) = self.pose_capture_ancestor(world, request_target) {
            return vec![target];
        }

        world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<PoseCaptureComponent>(id)
                    .is_some()
            })
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
            if world
                .get_component_by_id_as::<PoseCaptureComponent>(id)
                .is_some()
            {
                return Some(id);
            }
            current = world.parent_of(id);
        }
        None
    }

    fn owning_gltf(&self, world: &World, start: ComponentId) -> Option<ComponentId> {
        let mut current = Some(start);
        while let Some(id) = current {
            if world
                .get_component_by_id_as::<crate::engine::ecs::component::GLTFComponent>(id)
                .is_some()
            {
                return Some(id);
            }
            current = world.parent_of(id);
        }
        None
    }

    fn resolve_joint_query(
        &self,
        world: &World,
        gltf_id: ComponentId,
        query: &str,
    ) -> Vec<ComponentId> {
        let Some(gltf) =
            world.get_component_by_id_as::<crate::engine::ecs::component::GLTFComponent>(gltf_id)
        else {
            return Vec::new();
        };
        let query_root = world.parent_of(gltf_id).unwrap_or(gltf_id);
        let owned: HashSet<ComponentId> = gltf.armature_joint_transforms.iter().copied().collect();
        world
            .find_all_components(query_root, query)
            .into_iter()
            .filter(|id| owned.contains(id))
            .collect()
    }
}
