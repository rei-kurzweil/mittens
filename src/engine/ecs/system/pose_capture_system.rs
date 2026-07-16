use crate::engine::ecs::component::{
    EditorComponent, GLTFComponent, PoseBoneEntry, PoseCaptureComponent,
    PoseCaptureLibraryComponent, PoseCapturePoseComponent, PoseTargetRef, TransformComponent,
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
    ) -> Result<(), String> {
        let resolved = validate_pose_apply(world, target, pose_id)?;
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
        Ok(())
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
        let gltf_id = owning_gltf(world, target)?;
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
}

pub fn resolve_pose_apply_target(
    world: &World,
    selected_component: Option<ComponentId>,
    pose_id: ComponentId,
) -> Result<ComponentId, String> {
    if let Some(selected) = selected_component
        && let Some(gltf) = gltf_for_visual_selection(world, selected)
    {
        return Ok(gltf);
    }

    let library = world
        .parent_of(pose_id)
        .ok_or_else(|| format!("pose {pose_id:?} has no library parent"))?;
    world
        .get_component_by_id_as::<PoseCaptureLibraryComponent>(library)
        .ok_or_else(|| format!("pose {pose_id:?} is not inside a pose library"))?;
    let capture_target = world
        .parent_of(library)
        .ok_or_else(|| format!("pose library {library:?} has no capture target"))?;
    owning_gltf(world, capture_target)
        .ok_or_else(|| format!("pose {pose_id:?} capture target has no owning glTF"))
}

pub fn gltf_for_visual_selection(world: &World, selected: ComponentId) -> Option<ComponentId> {
    if world
        .get_component_by_id_as::<GLTFComponent>(selected)
        .is_some()
    {
        return Some(selected);
    }

    let mut ancestors = HashSet::new();
    let mut current = Some(selected);
    while let Some(id) = current {
        ancestors.insert(id);
        current = world.parent_of(id);
    }

    world.all_components().find(|&id| {
        world
            .get_component_by_id_as::<GLTFComponent>(id)
            .is_some_and(|gltf| {
                gltf.spawned_node_transforms
                    .iter()
                    .chain(&gltf.armature_joint_transforms)
                    .any(|node| ancestors.contains(node))
            })
    })
}

pub fn validate_pose_apply(
    world: &World,
    target: ComponentId,
    pose_id: ComponentId,
) -> Result<Vec<(ComponentId, PoseBoneEntry)>, String> {
    let pose = world
        .get_component_by_id_as::<PoseCapturePoseComponent>(pose_id)
        .ok_or_else(|| format!("component {pose_id:?} is not a pose"))?;
    let gltf_id = owning_gltf(world, target)
        .ok_or_else(|| format!("target {target:?} has no owning glTF"))?;
    let mut resolved = Vec::with_capacity(pose.entries.len());
    for entry in &pose.entries {
        let matches = resolve_joint_query(world, gltf_id, &entry.query);
        if matches.len() != 1 {
            return Err(format!(
                "joint query '{}' matched {} joints in the selected glTF; pose was not applied",
                entry.query,
                matches.len()
            ));
        }
        resolved.push((matches[0], entry.clone()));
    }
    Ok(resolved)
}

fn owning_gltf(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut current = Some(start);
    while let Some(id) = current {
        if world.get_component_by_id_as::<GLTFComponent>(id).is_some() {
            return Some(id);
        }
        current = world.parent_of(id);
    }
    gltf_for_visual_selection(world, start)
}

fn resolve_joint_query(world: &World, gltf_id: ComponentId, query: &str) -> Vec<ComponentId> {
    let Some(gltf) = world.get_component_by_id_as::<GLTFComponent>(gltf_id) else {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::RenderableComponent;

    struct PoseFixture {
        world: World,
        owner_gltf: ComponentId,
        selected_gltf: ComponentId,
        owner_target: ComponentId,
        pose: ComponentId,
        selected_node: ComponentId,
        selected_joint: ComponentId,
    }

    fn fixture() -> PoseFixture {
        let mut world = World::default();

        let owner_anchor = world.add_component(TransformComponent::new());
        let owner_gltf = world.add_component(GLTFComponent::new("owner.glb"));
        let owner_target = world.add_component(PoseCaptureComponent::new());
        let owner_joint =
            world.add_component_boxed_named("hips", Box::new(TransformComponent::new()));
        let _ = world.add_child(owner_anchor, owner_gltf);
        let _ = world.add_child(owner_gltf, owner_target);
        let _ = world.add_child(owner_anchor, owner_joint);
        {
            let gltf = world
                .get_component_by_id_as_mut::<GLTFComponent>(owner_gltf)
                .unwrap();
            gltf.spawned_node_transforms = vec![owner_joint];
            gltf.armature_joint_transforms = vec![owner_joint];
        }

        let selected_anchor = world.add_component(TransformComponent::new());
        let selected_gltf = world.add_component(GLTFComponent::new("selected.glb"));
        let selected_node =
            world.add_component_boxed_named("body", Box::new(TransformComponent::new()));
        let selected_joint =
            world.add_component_boxed_named("hips", Box::new(TransformComponent::new()));
        let _ = world.add_child(selected_anchor, selected_gltf);
        let _ = world.add_child(selected_anchor, selected_node);
        let _ = world.add_child(selected_node, selected_joint);
        {
            let gltf = world
                .get_component_by_id_as_mut::<GLTFComponent>(selected_gltf)
                .unwrap();
            gltf.spawned_node_transforms = vec![selected_node, selected_joint];
            gltf.armature_joint_transforms = vec![selected_joint];
        }

        let library = world.add_component(PoseCaptureLibraryComponent::new(PoseTargetRef::Query(
            "#owner".into(),
        )));
        let pose = world.add_component(PoseCapturePoseComponent::new(
            "Pose",
            PoseTargetRef::Query("#owner".into()),
            vec![PoseBoneEntry {
                query: "#hips".into(),
                translation: [1.0, 2.0, 3.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0; 3],
            }],
        ));
        let _ = world.add_child(owner_target, library);
        let _ = world.add_child(library, pose);

        PoseFixture {
            world,
            owner_gltf,
            selected_gltf,
            owner_target,
            pose,
            selected_node,
            selected_joint,
        }
    }

    #[test]
    fn visual_selection_resolves_direct_gltf_primitives_joints_and_armature_markers() {
        let mut fixture = fixture();
        let body_primitive = fixture.world.add_component(RenderableComponent::cube());
        let face_primitive = fixture.world.add_component(RenderableComponent::cube());
        let marker = fixture.world.add_component_boxed_named(
            "armature_joint_marker",
            Box::new(TransformComponent::new()),
        );
        let marker_renderable = fixture.world.add_component(RenderableComponent::cube());
        let _ = fixture
            .world
            .add_child(fixture.selected_node, body_primitive);
        let _ = fixture
            .world
            .add_child(fixture.selected_node, face_primitive);
        let _ = fixture.world.add_child(fixture.selected_joint, marker);
        let _ = fixture.world.add_child(marker, marker_renderable);

        for selected in [
            fixture.selected_gltf,
            body_primitive,
            face_primitive,
            fixture.selected_joint,
            marker_renderable,
        ] {
            assert_eq!(
                resolve_pose_apply_target(&fixture.world, Some(selected), fixture.pose).unwrap(),
                fixture.selected_gltf
            );
        }
    }

    #[test]
    fn unrelated_selection_falls_back_to_original_pose_owner() {
        let mut fixture = fixture();
        let unrelated = fixture.world.add_component(TransformComponent::new());
        assert_eq!(
            resolve_pose_apply_target(&fixture.world, Some(unrelated), fixture.pose).unwrap(),
            fixture.owner_gltf
        );
        assert_eq!(
            resolve_pose_apply_target(&fixture.world, None, fixture.pose).unwrap(),
            fixture.owner_gltf
        );
        assert_eq!(
            owning_gltf(&fixture.world, fixture.owner_target),
            Some(fixture.owner_gltf)
        );
    }

    #[test]
    fn validation_rejects_missing_and_ambiguous_joints_atomically() {
        let mut fixture = fixture();
        fixture
            .world
            .get_component_by_id_as_mut::<GLTFComponent>(fixture.selected_gltf)
            .unwrap()
            .armature_joint_transforms
            .clear();
        assert!(
            validate_pose_apply(&fixture.world, fixture.selected_gltf, fixture.pose)
                .unwrap_err()
                .contains("matched 0")
        );

        let duplicate = fixture
            .world
            .add_component_boxed_named("hips", Box::new(TransformComponent::new()));
        let selected_anchor = fixture.world.parent_of(fixture.selected_gltf).unwrap();
        let _ = fixture.world.add_child(selected_anchor, duplicate);
        fixture
            .world
            .get_component_by_id_as_mut::<GLTFComponent>(fixture.selected_gltf)
            .unwrap()
            .armature_joint_transforms = vec![fixture.selected_joint, duplicate];
        assert!(
            validate_pose_apply(&fixture.world, fixture.selected_gltf, fixture.pose)
                .unwrap_err()
                .contains("matched 2")
        );
    }
}
