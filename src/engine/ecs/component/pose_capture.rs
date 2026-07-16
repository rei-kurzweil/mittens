use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::Component;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PoseCaptureTargetMode {
    WholeSubtree,
    SkinnedJointsOnly,
    NamedRoot { selector_or_name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseCaptureComponent {
    pub label: Option<String>,
    pub asset_name: Option<String>,
    pub target_mode: PoseCaptureTargetMode,
    pub include_scale: bool,
    pub store_rest_deltas: bool,
    #[serde(skip)]
    component: Option<ComponentId>,
}

impl PoseCaptureComponent {
    pub fn new() -> Self {
        Self {
            label: None,
            asset_name: None,
            target_mode: PoseCaptureTargetMode::WholeSubtree,
            include_scale: true,
            store_rest_deltas: false,
            component: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_asset_name(mut self, asset_name: impl Into<String>) -> Self {
        let asset_name = asset_name.into();
        assert!(
            is_valid_pose_asset_name(&asset_name),
            "pose asset_name must contain only ASCII letters, digits, '_' or '-'"
        );
        self.asset_name = Some(asset_name);
        self
    }
}

impl Component for PoseCaptureComponent {
    fn name(&self) -> &'static str {
        "pose_capture"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut ce = ce_call("PoseCapture", "new", vec![]);
        if let Some(label) = &self.label {
            ce = ce.with_call("with_label", vec![s(label)]);
        }
        if let Some(asset_name) = &self.asset_name {
            ce = ce.with_call("with_asset_name", vec![s(asset_name)]);
        }
        ce
    }
}

pub fn is_valid_pose_asset_name(asset_name: &str) -> bool {
    !asset_name.is_empty()
        && asset_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PoseTargetRef {
    Query(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseBoneEntry {
    /// Query identifying one joint inside the owning GLTF instance.
    pub query: String,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub scale: [f32; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseCapturePoseComponent {
    pub name: String,
    pub target_root_ref: PoseTargetRef,
    pub entries: Vec<PoseBoneEntry>,
    #[serde(skip)]
    component: Option<ComponentId>,
}

impl PoseCapturePoseComponent {
    pub fn new(
        name: impl Into<String>,
        target_root_ref: PoseTargetRef,
        entries: Vec<PoseBoneEntry>,
    ) -> Self {
        let mut pose = Self {
            name: name.into(),
            target_root_ref,
            entries: Vec::with_capacity(entries.len()),
            component: None,
        };
        for entry in entries {
            // Keep the long-standing infallible constructor useful to runtime callers,
            // but never allow duplicate entries into the component.
            assert!(
                pose.push_joint(entry).is_ok(),
                "duplicate joint query in pose"
            );
        }
        pose
    }

    pub fn push_joint(&mut self, entry: PoseBoneEntry) -> Result<&mut Self, String> {
        if self
            .entries
            .iter()
            .any(|existing| existing.query == entry.query)
        {
            return Err(format!("duplicate joint query '{}'", entry.query));
        }
        self.entries.push(entry);
        Ok(self)
    }
}

impl Component for PoseCapturePoseComponent {
    fn name(&self) -> &'static str {
        "pose_capture_pose"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        let mut ce = ce_call("PoseCapturePose", "new", vec![s(&self.name)]);
        for entry in &self.entries {
            ce = ce.with_call(
                "joint",
                vec![
                    s(&entry.query),
                    array(nums(entry.translation.map(f64::from))),
                    array(nums(entry.rotation.map(f64::from))),
                    array(nums(entry.scale.map(f64::from))),
                ],
            );
        }
        ce
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseCaptureLibraryComponent {
    pub target_root_ref: PoseTargetRef,
    #[serde(skip)]
    component: Option<ComponentId>,
}

impl PoseCaptureLibraryComponent {
    pub fn new(target_root_ref: PoseTargetRef) -> Self {
        Self {
            target_root_ref,
            component: None,
        }
    }
}

impl Component for PoseCaptureLibraryComponent {
    fn name(&self) -> &'static str {
        "pose_capture_library"
    }

    fn set_id(&mut self, component: ComponentId) {
        self.component = Some(component);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn to_mms_ast(
        &self,
        _world: &crate::engine::ecs::World,
    ) -> crate::scripting::ast::ComponentExpression {
        use crate::engine::ecs::component::ce_helpers::*;
        ce_call("PoseCaptureLibrary", "new", vec![])
    }
}

/// Write one pose as an independently importable MMS module.
pub fn save_pose_asset(
    world: &crate::engine::ecs::World,
    pose_id: ComponentId,
    path: &std::path::Path,
) -> Result<(), String> {
    let pose = world
        .get_component_by_id_as::<PoseCapturePoseComponent>(pose_id)
        .ok_or_else(|| format!("component {pose_id:?} is not a pose"))?;
    let expression = crate::scripting::unparser::unparse_component(&pose.to_mms_ast(world));
    let text = format!("export fn pose() {{\n    return {expression}\n}}\n");
    write_asset_atomically(path, &text)
}

/// Save every ordered pose child to its own module, then rewrite the library manifest.
/// Pose filenames are stable by library order and sanitized pose name.
pub fn save_pose_library_asset(
    world: &crate::engine::ecs::World,
    library_id: ComponentId,
    manifest_path: &std::path::Path,
) -> Result<Vec<std::path::PathBuf>, String> {
    if world
        .get_component_by_id_as::<PoseCaptureLibraryComponent>(library_id)
        .is_none()
    {
        return Err(format!("component {library_id:?} is not a pose library"));
    }
    let parent = manifest_path.parent().unwrap_or(std::path::Path::new("."));
    let mut paths = Vec::new();
    for &child in world.children_of(library_id) {
        let Some(pose) = world.get_component_by_id_as::<PoseCapturePoseComponent>(child) else {
            continue;
        };
        let slug: String = pose
            .name
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();
        let slug = if slug.is_empty() {
            "pose".to_string()
        } else {
            slug
        };
        let path = parent.join(format!("{:03}-{slug}.pose.mms", paths.len()));
        save_pose_asset(world, child, &path)?;
        paths.push(path);
    }

    let mut manifest = String::new();
    for (index, path) in paths.iter().enumerate() {
        let relative = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| format!("pose asset path is not valid UTF-8: {}", path.display()))?;
        manifest.push_str(&format!(
            "import {{ pose as pose_{index} }} from \"{}\"\n",
            relative.replace('\\', "\\\\").replace('"', "\\\"")
        ));
    }
    manifest.push_str("\nPoseCaptureLibrary.new() {\n");
    for index in 0..paths.len() {
        manifest.push_str(&format!("    pose_{index}()\n"));
    }
    manifest.push_str("}\n");
    write_asset_atomically(manifest_path, &manifest)?;

    let generated: std::collections::HashSet<_> = paths.iter().cloned().collect();
    let entries = std::fs::read_dir(parent)
        .map_err(|error| format!("cannot list {}: {error}", parent.display()))?;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("cannot read entry in {}: {error}", parent.display()))?;
        let path = entry.path();
        let is_generated_pose =
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.len() >= "000-.pose.mms".len()
                        && name.as_bytes()[0..3].iter().all(u8::is_ascii_digit)
                        && name.as_bytes().get(3) == Some(&b'-')
                        && name.ends_with(".pose.mms")
                });
        if is_generated_pose && !generated.contains(&path) {
            std::fs::remove_file(&path)
                .map_err(|error| format!("cannot remove stale {}: {error}", path.display()))?;
        }
    }
    Ok(paths)
}

fn write_asset_atomically(path: &std::path::Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let tmp = path.with_extension(format!(
        "{}tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
    ));
    std::fs::write(&tmp, text)
        .map_err(|error| format!("cannot write {}: {error}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .map_err(|error| format!("cannot replace {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::World;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_directory(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("mittens-{name}-{nonce}"))
    }

    fn entry(query: &str, x: f32) -> PoseBoneEntry {
        PoseBoneEntry {
            query: query.to_string(),
            translation: [x, 2.0, 3.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        }
    }

    #[test]
    fn saves_ordered_pose_modules_manifest_and_removes_stale_generated_files() {
        let mut world = World::default();
        let library = world.add_component(PoseCaptureLibraryComponent::new(PoseTargetRef::Query(
            "#avatar".into(),
        )));
        let first = world.add_component(PoseCapturePoseComponent::new(
            "Idle Pose",
            PoseTargetRef::Query("#avatar".into()),
            vec![entry("#hips", 1.0)],
        ));
        let second = world.add_component(PoseCapturePoseComponent::new(
            "Wave",
            PoseTargetRef::Query("#avatar".into()),
            vec![entry("#hand", 4.0)],
        ));
        let _ = world.add_child(library, first);
        let _ = world.add_child(library, second);

        let directory = test_directory("pose-library-save");
        std::fs::create_dir_all(&directory).unwrap();
        let manifest = directory.join("library.mms");
        std::fs::write(&manifest, "old manifest").unwrap();
        std::fs::write(directory.join("009-stale.pose.mms"), "stale").unwrap();
        std::fs::write(directory.join("notes.mms"), "keep").unwrap();

        let paths = save_pose_library_asset(&world, library, &manifest).unwrap();
        assert_eq!(
            paths,
            vec![
                directory.join("000-Idle_Pose.pose.mms"),
                directory.join("001-Wave.pose.mms"),
            ]
        );
        let first_text = std::fs::read_to_string(&paths[0]).unwrap();
        assert!(first_text.contains("PoseCapturePose.new(\"Idle Pose\")"));
        assert!(first_text.contains("\"#hips\""));
        let manifest_text = std::fs::read_to_string(&manifest).unwrap();
        assert!(
            manifest_text.contains("import { pose as pose_0 } from \"000-Idle_Pose.pose.mms\"")
        );
        assert!(manifest_text.contains("import { pose as pose_1 } from \"001-Wave.pose.mms\""));
        assert!(manifest_text.find("pose_0()").unwrap() < manifest_text.find("pose_1()").unwrap());
        assert!(manifest_text.contains("PoseCaptureLibrary.new()"));
        assert!(!directory.join("009-stale.pose.mms").exists());
        assert_eq!(
            std::fs::read_to_string(directory.join("notes.mms")).unwrap(),
            "keep"
        );
        assert!(!directory.join("library.mmstmp").exists());

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn pose_asset_name_validation_is_strict_ascii() {
        for valid in ["bisket", "Bisket_2", "avatar-v3", "0"] {
            assert!(is_valid_pose_asset_name(valid), "{valid}");
        }
        for invalid in ["", "two words", "../escape", "café", "a/b"] {
            assert!(!is_valid_pose_asset_name(invalid), "{invalid}");
        }
    }
}
