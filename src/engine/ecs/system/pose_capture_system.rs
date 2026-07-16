use crate::engine::ecs::component::{
    EditorComponent, GLTFComponent, PoseBoneEntry, PoseCaptureComponent,
    PoseCaptureLibraryComponent, PoseCapturePoseComponent, PoseCaptureReconciliationState,
    PoseTargetRef, TransformComponent,
};
use crate::engine::ecs::rx::SignalEmitter;
use crate::engine::ecs::{ComponentId, IntentValue, World};
use crate::scripting::component_registry::spawn_tree_uninitialized;
use crate::scripting::object::{CeChild, MaterializedCE};
use crate::scripting::runner::MeowMeowRunner;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

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
        if let Some(capture) = world.get_component_by_id_as_mut::<PoseCaptureComponent>(target) {
            capture.mark_unsaved();
        }
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

pub fn pose_assets_root() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/components/poses"
    ))
}

pub fn sanitize_pose_asset_name(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.bytes().any(|byte| byte.is_ascii_alphanumeric()) {
        sanitized
    } else {
        "pose_library".to_string()
    }
}

pub fn gltf_uri_stem(uri: &str) -> Option<String> {
    let without_fragment = uri.split(['?', '#']).next().unwrap_or(uri);
    Path::new(without_fragment)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .map(str::to_string)
}

pub fn pose_asset_name_for_gltf_uri(uri: &str) -> String {
    gltf_uri_stem(uri)
        .map(|stem| sanitize_pose_asset_name(&stem))
        .unwrap_or_else(|| "pose_library".to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoseSelectionActivation {
    pub gltf: ComponentId,
    pub captures: Vec<ComponentId>,
    pub created_capture: Option<ComponentId>,
}

pub fn ensure_pose_capture_for_gltf_selection(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    selected: ComponentId,
) -> Result<Option<PoseSelectionActivation>, String> {
    ensure_pose_capture_for_gltf_selection_at_root(world, emit, selected, &pose_assets_root())
}

pub fn ensure_pose_capture_for_gltf_selection_at_root(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    selected: ComponentId,
    assets_root: &Path,
) -> Result<Option<PoseSelectionActivation>, String> {
    let Some(gltf) = gltf_for_visual_selection(world, selected) else {
        return Ok(None);
    };

    let mut captures: Vec<_> = world
        .all_components()
        .filter(|&id| {
            world
                .get_component_by_id_as::<PoseCaptureComponent>(id)
                .is_some()
                && owning_gltf(world, id) == Some(gltf)
        })
        .collect();
    let mut created_capture = None;

    if captures.is_empty() {
        let (uri, label) = {
            let gltf_component = world
                .get_component_by_id_as::<GLTFComponent>(gltf)
                .ok_or_else(|| format!("resolved glTF {gltf:?} no longer exists"))?;
            let label = world
                .component_label(gltf)
                .filter(|label| !label.is_empty())
                .map(str::to_string)
                .or_else(|| gltf_uri_stem(&gltf_component.uri))
                .unwrap_or_else(|| "Pose Library".to_string());
            (gltf_component.uri.clone(), label)
        };
        let capture = world.add_component(
            PoseCaptureComponent::new()
                .with_label(label)
                .with_asset_name(pose_asset_name_for_gltf_uri(&uri)),
        );
        if let Err(error) = world.add_child(gltf, capture) {
            let _ = world.remove_component_subtree(capture);
            return Err(format!(
                "cannot attach pose capture to selected glTF: {error}"
            ));
        }
        world.init_component_tree(capture, emit);
        captures.push(capture);
        created_capture = Some(capture);
    }

    let mut errors = Vec::new();
    for &capture in &captures {
        if let Err(error) = reconcile_pose_capture_at_root(world, emit, capture, assets_root) {
            errors.push(format!("{capture:?}: {error}"));
        }
    }
    if !errors.is_empty() {
        return Err(format!(
            "pose capture reconciliation failed: {}",
            errors.join("; ")
        ));
    }

    Ok(Some(PoseSelectionActivation {
        gltf,
        captures,
        created_capture,
    }))
}

pub fn reconcile_pose_captures(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
) -> Vec<(ComponentId, Result<ComponentId, String>)> {
    reconcile_pose_captures_at_root(world, emit, &pose_assets_root())
}

pub fn reconcile_pose_captures_at_root(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    assets_root: &Path,
) -> Vec<(ComponentId, Result<ComponentId, String>)> {
    let targets: Vec<_> = world
        .all_components()
        .filter(|&id| {
            world
                .get_component_by_id_as::<PoseCaptureComponent>(id)
                .is_some_and(|capture| {
                    matches!(
                        capture.runtime.state,
                        PoseCaptureReconciliationState::Unreconciled
                    )
                })
        })
        .collect();
    targets
        .into_iter()
        .map(|target| {
            let result = reconcile_pose_capture_at_root(world, emit, target, assets_root);
            (target, result)
        })
        .collect()
}

pub fn reconcile_pose_capture_at_root(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    target: ComponentId,
    assets_root: &Path,
) -> Result<ComponentId, String> {
    if world
        .get_component_by_id_as::<PoseCaptureComponent>(target)
        .is_none()
    {
        return Err(format!("component {target:?} is not a PoseCapture"));
    }
    if !matches!(
        world
            .get_component_by_id_as::<PoseCaptureComponent>(target)
            .unwrap()
            .runtime
            .state,
        PoseCaptureReconciliationState::Unreconciled
    ) {
        return world
            .children_of(target)
            .iter()
            .copied()
            .find(|&child| {
                world
                    .get_component_by_id_as::<PoseCaptureLibraryComponent>(child)
                    .is_some()
            })
            .ok_or_else(|| format!("reconciled PoseCapture {target:?} has no library"));
    }

    let gltf = owning_gltf(world, target);
    let derived_name = gltf
        .and_then(|gltf| world.get_component_by_id_as::<GLTFComponent>(gltf))
        .map(|gltf| pose_asset_name_for_gltf_uri(&gltf.uri))
        .unwrap_or_else(|| "pose_library".to_string());
    {
        let capture = world
            .get_component_by_id_as_mut::<PoseCaptureComponent>(target)
            .unwrap();
        if capture.asset_name.is_none() {
            capture.asset_name = Some(derived_name);
        }
        capture.runtime.asset_name_draft = capture.asset_name.clone();
    }

    let mut authored_libraries: Vec<_> = world
        .children_of(target)
        .iter()
        .copied()
        .filter(|&child| {
            world
                .get_component_by_id_as::<PoseCaptureLibraryComponent>(child)
                .is_some()
        })
        .collect();
    if let Some(library) = authored_libraries.first().copied() {
        for duplicate in authored_libraries.drain(1..) {
            let _ = world.remove_component_subtree(duplicate);
        }
        let capture = world
            .get_component_by_id_as_mut::<PoseCaptureComponent>(target)
            .unwrap();
        capture.runtime.state = PoseCaptureReconciliationState::Authored;
        return Ok(library);
    }

    let asset_name = world
        .get_component_by_id_as::<PoseCaptureComponent>(target)
        .and_then(|capture| capture.asset_name.clone())
        .unwrap_or_else(|| "pose_library".to_string());
    let manifest = assets_root.join(&asset_name).join("library.mms");
    if !manifest.exists() {
        let library = attach_empty_library(world, emit, target)?;
        world
            .get_component_by_id_as_mut::<PoseCaptureComponent>(target)
            .unwrap()
            .runtime
            .state = PoseCaptureReconciliationState::New;
        return Ok(library);
    }

    match load_validated_pose_library(&manifest, world, emit) {
        Ok(library) => {
            world
                .add_child(target, library)
                .map_err(|error| format!("cannot attach hydrated pose library: {error}"))?;
            if world.is_initialized(target) {
                world.init_component_tree(library, emit);
            }
            world
                .get_component_by_id_as_mut::<PoseCaptureComponent>(target)
                .unwrap()
                .runtime
                .state = PoseCaptureReconciliationState::Hydrated;
            Ok(library)
        }
        Err(error) => {
            let library = attach_empty_library(world, emit, target)?;
            world
                .get_component_by_id_as_mut::<PoseCaptureComponent>(target)
                .unwrap()
                .runtime
                .state = PoseCaptureReconciliationState::LoadFailed {
                asset_name,
                error: error.clone(),
                overwrite_warning_issued: false,
            };
            Ok(library)
        }
    }
}

fn attach_empty_library(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    target: ComponentId,
) -> Result<ComponentId, String> {
    let library = world.add_component(PoseCaptureLibraryComponent::new(PoseTargetRef::Query(
        "TODO".to_string(),
    )));
    world
        .add_child(target, library)
        .map_err(|error| format!("cannot attach empty pose library: {error}"))?;
    if world.is_initialized(target) {
        world.init_component_tree(library, emit);
    }
    Ok(library)
}

fn load_validated_pose_library(
    manifest: &Path,
    world: &mut World,
    emit: &mut dyn SignalEmitter,
) -> Result<ComponentId, String> {
    let manifest_text = manifest.to_str().ok_or_else(|| {
        format!(
            "pose manifest path is not valid UTF-8: {}",
            manifest.display()
        )
    })?;
    let module = MeowMeowRunner::load_module_file(manifest_text)?;
    if module.sequence.len() != 1 {
        return Err(format!(
            "pose manifest must contain exactly one library root; found {}",
            module.sequence.len()
        ));
    }
    validate_library_materialized(&module.sequence[0])?;

    let mut validation_world = World::default();
    let mut validation_emit = NullEmitter;
    let validation_root = spawn_tree_uninitialized(
        &module.sequence[0],
        &mut validation_world,
        &mut validation_emit,
    )
    .map_err(|error| format!("pose manifest validation failed: {error}"))?;
    validate_library_world(&validation_world, validation_root)?;

    spawn_tree_uninitialized(&module.sequence[0], world, emit)
        .map_err(|error| format!("pose manifest hydration failed: {error}"))
}

fn validate_library_materialized(root: &MaterializedCE) -> Result<(), String> {
    if !matches!(
        root.component_type.as_str(),
        "PoseCaptureLibrary" | "PoseCaptureLibraryComponent"
    ) {
        return Err(format!(
            "pose manifest root must be PoseCaptureLibrary, found {}",
            root.component_type
        ));
    }
    for (index, child) in root.children.iter().enumerate() {
        let CeChild::Spawn(child) = child else {
            return Err(format!(
                "pose manifest child {index} must be an authored pose"
            ));
        };
        if !matches!(
            child.component_type.as_str(),
            "PoseCapturePose" | "PoseCapturePoseComponent"
        ) {
            return Err(format!(
                "pose manifest child {index} must be PoseCapturePose, found {}",
                child.component_type
            ));
        }
        if !child.children.is_empty() {
            return Err(format!(
                "pose manifest pose child {index} may not contain nested components"
            ));
        }
    }
    Ok(())
}

fn validate_library_world(world: &World, root: ComponentId) -> Result<(), String> {
    if world
        .get_component_by_id_as::<PoseCaptureLibraryComponent>(root)
        .is_none()
    {
        return Err("pose manifest did not materialize a pose library".to_string());
    }
    for (index, &child) in world.children_of(root).iter().enumerate() {
        if world
            .get_component_by_id_as::<PoseCapturePoseComponent>(child)
            .is_none()
        {
            return Err(format!(
                "pose manifest child {index} did not materialize a pose"
            ));
        }
    }
    Ok(())
}

struct NullEmitter;

impl SignalEmitter for NullEmitter {
    fn push_event(&mut self, _scope: ComponentId, _event: crate::engine::ecs::EventSignal) {}

    fn push_intent(&mut self, _scope: ComponentId, _intent: crate::engine::ecs::IntentSignal) {}
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
    use crate::engine::ecs::component::{RenderableComponent, save_pose_library_asset};
    use std::time::{SystemTime, UNIX_EPOCH};

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

    fn test_directory(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("mittens-{name}-{nonce}"))
    }

    fn write_pose_library(root: &Path, asset_name: &str, pose_name: &str) {
        let mut source = World::default();
        let library = source.add_component(PoseCaptureLibraryComponent::new(PoseTargetRef::Query(
            "TODO".into(),
        )));
        let pose = source.add_component(PoseCapturePoseComponent::new(
            pose_name,
            PoseTargetRef::Query("TODO".into()),
            Vec::new(),
        ));
        source.add_child(library, pose).unwrap();
        let manifest = root.join(asset_name).join("library.mms");
        save_pose_library_asset(&source, library, &manifest).unwrap();
    }

    #[test]
    fn derives_sanitized_asset_names_from_gltf_uri_stems() {
        assert_eq!(
            pose_asset_name_for_gltf_uri("assets/models/bisket.11.0.glb"),
            "bisket_11_0"
        );
        assert_eq!(
            pose_asset_name_for_gltf_uri("https://example.test/cat girl.glb?v=2"),
            "cat_girl"
        );
        assert_eq!(pose_asset_name_for_gltf_uri(""), "pose_library");
        assert_eq!(sanitize_pose_asset_name("..."), "pose_library");
    }

    #[test]
    fn reconciliation_preserves_authored_library_and_is_idempotent() {
        let mut world = World::default();
        let gltf = world.add_component(GLTFComponent::new("models/avatar.glb"));
        let target = world.add_component(PoseCaptureComponent::new());
        let authored = world.add_component(PoseCaptureLibraryComponent::new(PoseTargetRef::Query(
            "TODO".into(),
        )));
        world.add_child(gltf, target).unwrap();
        world.add_child(target, authored).unwrap();
        let root = test_directory("pose-authored");
        write_pose_library(&root, "avatar", "Disk");
        let mut emit = NullEmitter;

        assert_eq!(
            reconcile_pose_capture_at_root(&mut world, &mut emit, target, &root).unwrap(),
            authored
        );
        assert!(matches!(
            world
                .get_component_by_id_as::<PoseCaptureComponent>(target)
                .unwrap()
                .runtime
                .state,
            PoseCaptureReconciliationState::Authored
        ));
        assert_eq!(
            reconcile_pose_capture_at_root(&mut world, &mut emit, target, &root).unwrap(),
            authored
        );
        assert_eq!(
            world
                .children_of(target)
                .iter()
                .filter(|&&id| world
                    .get_component_by_id_as::<PoseCaptureLibraryComponent>(id)
                    .is_some())
                .count(),
            1
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn valid_manifests_hydrate_independent_runtime_copies() {
        let root = test_directory("pose-hydrate");
        write_pose_library(&root, "shared", "Idle");
        let mut world = World::default();
        let gltf_a = world.add_component(GLTFComponent::new("a.glb"));
        let gltf_b = world.add_component(GLTFComponent::new("b.glb"));
        let target_a = world.add_component(PoseCaptureComponent::new().with_asset_name("shared"));
        let target_b = world.add_component(PoseCaptureComponent::new().with_asset_name("shared"));
        world.add_child(gltf_a, target_a).unwrap();
        world.add_child(gltf_b, target_b).unwrap();
        let mut emit = NullEmitter;

        let library_a =
            reconcile_pose_capture_at_root(&mut world, &mut emit, target_a, &root).unwrap();
        let library_b =
            reconcile_pose_capture_at_root(&mut world, &mut emit, target_b, &root).unwrap();
        assert_ne!(library_a, library_b);
        assert_eq!(world.children_of(library_a).len(), 1);
        assert_eq!(world.children_of(library_b).len(), 1);
        let extra = world.add_component(PoseCapturePoseComponent::new(
            "Extra",
            PoseTargetRef::Query("TODO".into()),
            Vec::new(),
        ));
        world.add_child(library_a, extra).unwrap();
        assert_eq!(world.children_of(library_a).len(), 2);
        assert_eq!(world.children_of(library_b).len(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_manifest_attaches_empty_library_and_preserves_load_error() {
        let root = test_directory("pose-malformed");
        let directory = root.join("broken");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            directory.join("library.mms"),
            "PoseCaptureLibrary.new()\nPoseCaptureLibrary.new()\n",
        )
        .unwrap();
        let mut world = World::default();
        let gltf = world.add_component(GLTFComponent::new("broken.glb"));
        let target = world.add_component(PoseCaptureComponent::new());
        world.add_child(gltf, target).unwrap();
        let mut emit = NullEmitter;

        let library = reconcile_pose_capture_at_root(&mut world, &mut emit, target, &root).unwrap();
        assert!(world.children_of(library).is_empty());
        assert!(matches!(
            &world
                .get_component_by_id_as::<PoseCaptureComponent>(target)
                .unwrap()
                .runtime
                .state,
            PoseCaptureReconciliationState::LoadFailed {
                asset_name,
                error,
                overwrite_warning_issued: false,
            } if asset_name == "broken" && error.contains("exactly one")
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn visual_selection_activation_creates_hydrates_and_reuses_one_capture() {
        let root = test_directory("pose-selection-hydrate");
        write_pose_library(&root, "selected", "Idle");
        let mut fixture = fixture();
        let marker = fixture.world.add_component_boxed_named(
            "armature_joint_marker",
            Box::new(TransformComponent::new()),
        );
        let marker_renderable = fixture.world.add_component(RenderableComponent::cube());
        fixture
            .world
            .add_child(fixture.selected_joint, marker)
            .unwrap();
        fixture.world.add_child(marker, marker_renderable).unwrap();
        let mut emit = NullEmitter;

        let first = ensure_pose_capture_for_gltf_selection_at_root(
            &mut fixture.world,
            &mut emit,
            marker_renderable,
            &root,
        )
        .unwrap()
        .unwrap();
        let capture = first.created_capture.unwrap();
        assert_eq!(first.gltf, fixture.selected_gltf);
        assert_eq!(first.captures, vec![capture]);
        assert_eq!(
            fixture.world.parent_of(capture),
            Some(fixture.selected_gltf)
        );
        let library = fixture
            .world
            .children_of(capture)
            .iter()
            .copied()
            .find(|&id| {
                fixture
                    .world
                    .get_component_by_id_as::<PoseCaptureLibraryComponent>(id)
                    .is_some()
            })
            .unwrap();
        assert_eq!(fixture.world.children_of(library).len(), 1);
        assert!(matches!(
            fixture
                .world
                .get_component_by_id_as::<PoseCaptureComponent>(capture)
                .unwrap()
                .runtime
                .state,
            PoseCaptureReconciliationState::Hydrated
        ));

        for selected in [marker, fixture.selected_node, fixture.selected_gltf] {
            let activation = ensure_pose_capture_for_gltf_selection_at_root(
                &mut fixture.world,
                &mut emit,
                selected,
                &root,
            )
            .unwrap()
            .unwrap();
            assert_eq!(activation.created_capture, None);
            assert_eq!(activation.captures, vec![capture]);
        }
        assert_eq!(
            fixture
                .world
                .all_components()
                .filter(|&id| fixture
                    .world
                    .get_component_by_id_as::<PoseCaptureComponent>(id)
                    .is_some()
                    && owning_gltf(&fixture.world, id) == Some(fixture.selected_gltf))
                .count(),
            1
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn visual_selection_activation_is_unrelated_safe_and_keeps_gltfs_independent() {
        let root = test_directory("pose-selection-independent");
        let mut fixture = fixture();
        let unrelated = fixture.world.add_component(TransformComponent::new());
        let mut emit = NullEmitter;
        let captures_before = fixture
            .world
            .all_components()
            .filter(|&id| {
                fixture
                    .world
                    .get_component_by_id_as::<PoseCaptureComponent>(id)
                    .is_some()
            })
            .count();

        assert!(
            ensure_pose_capture_for_gltf_selection_at_root(
                &mut fixture.world,
                &mut emit,
                unrelated,
                &root,
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(
            fixture
                .world
                .all_components()
                .filter(|&id| fixture
                    .world
                    .get_component_by_id_as::<PoseCaptureComponent>(id)
                    .is_some())
                .count(),
            captures_before
        );

        let activation = ensure_pose_capture_for_gltf_selection_at_root(
            &mut fixture.world,
            &mut emit,
            fixture.selected_gltf,
            &root,
        )
        .unwrap()
        .unwrap();
        let selected_capture = activation.created_capture.unwrap();
        assert_ne!(selected_capture, fixture.owner_target);
        assert_eq!(
            owning_gltf(&fixture.world, fixture.owner_target),
            Some(fixture.owner_gltf)
        );
        assert_eq!(
            owning_gltf(&fixture.world, selected_capture),
            Some(fixture.selected_gltf)
        );
        std::fs::remove_dir_all(root).unwrap_or(());
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
