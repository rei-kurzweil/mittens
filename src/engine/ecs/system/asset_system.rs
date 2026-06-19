use std::collections::HashMap;
use std::path::{Path, PathBuf};

use std::cell::RefCell;
use std::collections::HashSet;
use std::sync::Arc;

use crate::engine::ecs::component::{
    LayoutComponent, RaycastableComponent, StyleComponent, TransformComponent,
};
use crate::engine::ecs::system::bounds_system::{BoundsSystem, RenderableBoundsMeasure};
use crate::engine::ecs::system::editor_paint_system::PaintAssetTemplate;
use crate::engine::ecs::{ComponentId, SignalEmitter, World};
use crate::meow_meow::object::Value;
use crate::meow_meow::runner::{LoadedMmsModule, MeowMeowRunner};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetModuleId(u32);

#[derive(Debug)]
pub struct AssetModule {
    pub id: AssetModuleId,
    pub path: PathBuf,
    pub module: Arc<LoadedMmsModule>,
}

#[derive(Debug, Clone)]
pub struct AssetItem {
    pub module_id: AssetModuleId,
    pub export_name: String,
    pub title: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub param_names: Vec<String>,
}

impl AssetItem {
    pub fn asset_key(&self, module_path: &Path) -> String {
        format!("{}::{}", module_path.display(), self.export_name)
    }
}

#[derive(Debug, Clone)]
pub enum AssetSource {
    MmsModule {
        module_id: AssetModuleId,
        export_name: String,
    },
    RustFactory {
        factory_name: String,
    },
}

#[derive(Debug, Default)]
pub struct AssetSystem {
    modules: HashMap<AssetModuleId, AssetModule>,
    module_paths: HashMap<PathBuf, AssetModuleId>,
    pub items: Vec<AssetItem>,
    next_module_id: u32,
    asset_dir: Option<PathBuf>,
    /// Preview shells whose styled content needs remeasurement after layout resolves.
    /// Each entry is `(preview_shell_id, preview_root_id)`.
    pending_remeasure: RefCell<Vec<(ComponentId, ComponentId)>>,
}

impl AssetSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scan_assets_dir(&mut self, path: &Path) -> Result<(), String> {
        self.asset_dir = Some(path.to_path_buf());
        let entries = std::fs::read_dir(path)
            .map_err(|e| format!("cannot read assets dir '{}': {e}", path.display()))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("cannot read assets dir entry: {e}"))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("mms") {
                println!("[AssetSystem][debug] scanning asset module: {:?}", path);
                self.load_module(path)?;
            }
        }

        Ok(())
    }

    pub fn load_module(&mut self, path: PathBuf) -> Result<(), String> {
        let normalized_path = path;
        if self.module_paths.contains_key(&normalized_path) {
            return Ok(());
        }
        let module = Arc::new(MeowMeowRunner::load_module_file(
            normalized_path
                .to_str()
                .ok_or_else(|| format!("non-UTF8 asset path: {}", normalized_path.display()))?,
        )?);

        let module_id = AssetModuleId(self.next_module_id);
        self.next_module_id += 1;

        let file_stem = normalized_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("asset");

        for (name, value) in &module.named_exports {
            if let Value::Function { params, .. } = value {
                println!(
                    "[AssetSystem][debug]   found export function: {} (params: {:?})",
                    name, params
                );

                let title = if name == "main" || name == file_stem {
                    file_stem.to_string()
                } else {
                    format!("{}: {}", file_stem, name)
                };

                self.items.push(AssetItem {
                    module_id,
                    export_name: name.clone(),
                    title,
                    description: None,
                    category: None,
                    param_names: params.clone(),
                });
            }
        }
        self.module_paths.insert(normalized_path.clone(), module_id);
        self.modules.insert(
            module_id,
            AssetModule {
                id: module_id,
                path: normalized_path,
                module,
            },
        );

        Ok(())
    }

    pub fn get_item_module(&self, item: &AssetItem) -> Option<&LoadedMmsModule> {
        self.modules
            .get(&item.module_id)
            .map(|module| module.module.as_ref())
    }

    pub fn asset_function(&self, item: &AssetItem) -> Option<&Value> {
        self.get_item_module(item)
            .and_then(|module| module.named_export(&item.export_name))
    }

    pub fn get_module_name(&self, module_id: AssetModuleId) -> Option<String> {
        self.modules.get(&module_id).and_then(|m| {
            m.path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        })
    }

    pub fn paint_templates(&self) -> Vec<PaintAssetTemplate> {
        self.items
            .iter()
            .filter_map(|item| {
                let module = self.modules.get(&item.module_id)?;
                Some(PaintAssetTemplate {
                    key: item.asset_key(&module.path),
                    title: item.title.clone(),
                    module: Arc::clone(&module.module),
                    export_name: item.export_name.clone(),
                    param_names: item.param_names.clone(),
                })
            })
            .collect()
    }

    pub fn build_asset_module_header(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        module_name: &str,
    ) -> Result<ComponentId, String> {
        let asset_header_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/components/asset_module_header.mms"
        );

        MeowMeowRunner::spawn_mms_module_component_from_file(
            asset_header_path,
            "asset_module_header",
            vec![Value::String(module_name.to_string())],
            None,
            world,
            emit,
        )
    }

    pub fn spawn_asset_component(
        &self,
        item: &AssetItem,
        args: Vec<Value>,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
    ) -> Result<ComponentId, String> {
        let module = self.modules.get(&item.module_id).ok_or_else(|| {
            format!(
                "asset module not loaded for item '{}::{}'",
                item.title, item.export_name
            )
        })?;

        MeowMeowRunner::spawn_mms_module_component(
            &module.module,
            &item.export_name,
            args,
            None,
            world,
            emit,
        )
    }

    pub fn spawn_asset_component_uninitialized(
        &self,
        item: &AssetItem,
        args: Vec<Value>,
        world: &mut World,
        render_assets: &crate::engine::graphics::RenderAssets,
        emit: &mut dyn SignalEmitter,
    ) -> Result<ComponentId, String> {
        let module = self.modules.get(&item.module_id).ok_or_else(|| {
            format!(
                "asset module not loaded for item '{}::{}'",
                item.title, item.export_name
            )
        })?;

        MeowMeowRunner::spawn_mms_module_component_uninitialized(
            &module.module,
            &item.export_name,
            args,
            world,
            emit,
        )
    }

    fn assets_panel_asset_path() -> &'static str {
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/panels.mms")
    }

    pub fn spawn_assets_panel(
        &self,
        world: &mut World,
        render_assets: &crate::engine::graphics::RenderAssets,
        emit: &mut dyn SignalEmitter,
        parent: ComponentId,
        position: (f32, f32, f32),
    ) -> Result<ComponentId, String> {
        let panel_title = match self.asset_dir.as_ref() {
            Some(path) => format!("Assets: {}", path.display()),
            None => "Assets".to_string(),
        };

        let panel_root = MeowMeowRunner::spawn_mms_module_component_from_file(
            Self::assets_panel_asset_path(),
            "asset_panel",
            vec![
                Value::String(panel_title),
                Value::Array(Vec::new()),
                Value::Array(vec![
                    Value::Number(0.90),
                    Value::Number(1.00),
                    Value::Number(0.92),
                    Value::Number(1.0),
                ]),
                Value::Array(vec![
                    Value::Number(0.18),
                    Value::Number(0.78),
                    Value::Number(0.22),
                    Value::Number(0.95),
                ]),
                Value::Array(vec![
                    Value::Number(0.92),
                    Value::Number(0.97),
                    Value::Number(0.92),
                    Value::Number(1.0),
                ]),
            ],
            None,
            world,
            emit,
        )?;

        let wrapper = world.add_component_boxed_named(
            "assets_panel_shell",
            Box::new(TransformComponent::new().with_position(position.0, position.1, position.2)),
        );

        world
            .add_child(wrapper, panel_root)
            .map_err(|e| format!("attach assets panel child failed: {e}"))?;

        let content_root = world
            .find_component(panel_root, "#assets_content_area")
            .ok_or_else(|| "assets panel missing content area".to_string())?;

        let mut last_module_id = None;
        for (index, item) in self.items.iter().enumerate() {
            if last_module_id != Some(item.module_id) {
                last_module_id = Some(item.module_id);
                if let Some(module_name) = self.get_module_name(item.module_id) {
                    let header_root = self.build_asset_module_header(world, emit, &module_name)?;
                    world
                        .add_child(content_root, header_root)
                        .map_err(|e| format!("attach asset header failed: {e}"))?;
                }
            }

            let item_root = self.build_asset_item_shell(world, render_assets, emit, item, index)?;
            world
                .add_child(content_root, item_root)
                .map_err(|e| format!("attach asset item failed: {e}"))?;
        }

        world.init_component_tree(wrapper, emit);

        // Dirty the nearest ancestor LayoutComponent so the layout system
        // re-measures the content area now that all items are attached.
        let mut cur = world.parent_of(content_root);
        while let Some(ancestor) = cur {
            if let Some(lc) = world.get_component_by_id_as_mut::<LayoutComponent>(ancestor) {
                lc.dirty = true;
                break;
            }
            cur = world.parent_of(ancestor);
        }

        emit.push_intent_now(
            wrapper,
            crate::engine::ecs::IntentValue::Attach {
                parents: vec![parent],
                child: wrapper,
            },
        );
        Ok(wrapper)
    }

    /// Check if a subtree contains styled elements (`StyleComponent`) without a
    /// `LayoutComponent` ancestor — meaning it needs a layout root to resolve properly.
    fn subtree_needs_layout_root(world: &World, root: ComponentId) -> bool {
        let mut stack = vec![root];
        let mut visited = HashSet::new();

        while let Some(node) = stack.pop() {
            if !visited.insert(node) {
                continue;
            }

            if world
                .get_component_by_id_as::<StyleComponent>(node)
                .is_some()
            {
                let mut has_layout = false;
                let mut current = world.parent_of(node);
                while let Some(ancestor) = current {
                    if world
                        .get_component_by_id_as::<LayoutComponent>(ancestor)
                        .is_some()
                    {
                        has_layout = true;
                        break;
                    }
                    current = world.parent_of(ancestor);
                }
                if !has_layout {
                    return true;
                }
            }

            for &child in world.children_of(node) {
                stack.push(child);
            }
        }

        false
    }

    /// If `root`'s subtree contains styled elements that need layout resolution,
    /// create a `LayoutComponent` and return its id. Returns `None` if no layout root is needed.
    pub fn ensure_layout_root_if_needed(
        world: &mut World,
        root: ComponentId,
    ) -> Option<ComponentId> {
        if !Self::subtree_needs_layout_root(world, root) {
            return None;
        }

        let layout_root = world
            .add_component_boxed_named("preview_layout_root", Box::new(LayoutComponent::new(20.0)));

        Some(layout_root)
    }

    /// Walk the subtree rooted at `root` and set every `RaycastableComponent`
    /// to disabled, so preview content doesn't steal pointer events from the
    /// assets panel's scroll container.
    fn disable_raycast_in_subtree(world: &mut World, root: ComponentId) {
        let mut stack = vec![root];
        let mut visited = HashSet::new();
        while let Some(node) = stack.pop() {
            if !visited.insert(node) {
                continue;
            }
            if let Some(rc) = world.get_component_by_id_as_mut::<RaycastableComponent>(node) {
                rc.enable = false;
            }
            for &child in world.children_of(node) {
                stack.push(child);
            }
        }
    }

    fn remove_preview_placeholder(world: &mut World, item_root: ComponentId) {
        if let Some(placeholder) = world.find_component(item_root, "#preview_placeholder") {
            let _ = world.remove_component_subtree(placeholder);
        }
    }

    pub fn build_asset_item_shell(
        &self,
        world: &mut World,
        render_assets: &crate::engine::graphics::RenderAssets,
        emit: &mut dyn SignalEmitter,
        item: &AssetItem,
        _index: usize,
    ) -> Result<ComponentId, String> {
        let asset_item_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/components/asset_item.mms"
        );
        let module = self.modules.get(&item.module_id).ok_or_else(|| {
            format!(
                "asset module not loaded for item '{}::{}'",
                item.title, item.export_name
            )
        })?;

        let item_root = MeowMeowRunner::spawn_mms_module_component_from_file(
            asset_item_path,
            "asset_item",
            vec![
                Value::String(item.title.clone()),
                Value::String(item.asset_key(&module.path)),
                Value::Array(vec![
                    Value::Number(0.92),
                    Value::Number(0.97),
                    Value::Number(0.92),
                    Value::Number(1.0),
                ]),
            ],
            None,
            world,
            emit,
        )?;

        // Pass dummy arguments based on param_names
        let mut args = Vec::new();
        for name in &item.param_names {
            let lower_name = name.to_lowercase();
            if lower_name.contains("color") {
                args.push(Value::Array(vec![
                    Value::Number(0.5),
                    Value::Number(0.5),
                    Value::Number(0.5),
                    Value::Number(1.0),
                ]));
            } else if lower_name.contains("items") || lower_name.contains("sequence") {
                args.push(Value::Array(Vec::new()));
            } else if lower_name.contains("path")
                || lower_name.contains("url")
                || lower_name.contains("uri")
            {
                args.push(Value::String("assets/world/default.mms".to_string()));
            } else if lower_name.contains("title")
                || lower_name.contains("label")
                || lower_name.contains("name")
                || lower_name.contains("text")
            {
                args.push(Value::String("Preview".to_string()));
            } else {
                args.push(Value::Null);
            }
        }

        match self.spawn_asset_component_uninitialized(item, args, world, render_assets, emit) {
            Ok(preview_root) => {
                // Disable raycasting on all preview content so it doesn't steal
                // pointer events from the assets panel's scroll container.
                Self::disable_raycast_in_subtree(world, preview_root);

                // TEMP: skip preview rendering for items from panel modules
                // to isolate the scrolling issue.
                let is_panel = self
                    .get_module_name(item.module_id)
                    .map(|name| name.contains("panel"))
                    .unwrap_or(false);
                if is_panel {
                    // panel module item — skip preview rendering
                } else {
                    // Geometry-based preview (icons, meshes, etc.)
                    let preview_slot = world
                        .find_component(item_root, "#preview_slot")
                        .unwrap_or(item_root);

                    let preview_shell = world.add_component_boxed_named(
                        "asset_preview_shell",
                        Box::new(TransformComponent::new().with_position(0.0, 0.0, 0.05)),
                    );

                    let bounds = BoundsSystem::measure_renderable_subtree_bounds(
                        world,
                        render_assets,
                        preview_root,
                    );

                    if let RenderableBoundsMeasure::Measured(b) = bounds {
                        let s = 0.2_f32 / b.max_dimension().max(1e-6);
                        let center = b.center();
                        emit.push_intent_now(
                            preview_shell,
                            crate::engine::ecs::IntentValue::UpdateTransform {
                                component_ids: vec![preview_shell],
                                translation: [
                                    -center[0] * s,
                                    -center[1] * s,
                                    -center[2] * s + 0.05,
                                ],
                                rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                                scale: [s, s, s],
                            },
                        );
                    } else {
                        emit.push_intent_now(
                            preview_shell,
                            crate::engine::ecs::IntentValue::UpdateTransform {
                                component_ids: vec![preview_shell],
                                translation: [0.0, 0.0, 0.05],
                                rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                                scale: [0.5, 0.5, 0.5],
                            },
                        );
                    }

                    world
                        .add_child(preview_shell, preview_root)
                        .map_err(|e| format!("attach preview failed: {e}"))?;
                    world
                        .add_child(preview_slot, preview_shell)
                        .map_err(|e| format!("attach preview shell failed: {e}"))?;
                    Self::remove_preview_placeholder(world, item_root);
                }
            }
            Err(e) => {
                eprintln!(
                    "[AssetSystem] preview unavailable for '{}::{}': {e}",
                    item.title, item.export_name
                );
            }
        }

        Ok(item_root)
    }

    /// After layout has ticked, remeasure any pending preview subtrees that
    /// were spawned with styled content but no `RenderableComponent` bounds.
    /// Layout-generated background quads now exist, so bounds should return
    /// real AABBs that we can use to compute proper scale/offset.
    pub fn remeasure_pending_previews(
        &mut self,
        world: &mut World,
        render_assets: &crate::engine::graphics::RenderAssets,
        emit: &mut dyn SignalEmitter,
    ) {
        let pending = self.pending_remeasure.replace(Vec::new());
        for (preview_shell, preview_root) in pending {
            let bounds =
                BoundsSystem::calculate_subtree_local_bounds(world, render_assets, preview_root);

            if let Some(b) = bounds {
                // Scale so the preview is 0.2 units wide, preserving aspect ratio.
                let target_width_gu = 0.2_f32;
                let current_width = b.width().max(1e-6);
                let s = target_width_gu / current_width;
                let center = b.center();

                emit.push_intent_now(
                    preview_shell,
                    crate::engine::ecs::IntentValue::UpdateTransform {
                        component_ids: vec![preview_shell],
                        translation: [-center[0] * s, -center[1] * s, -center[2] * s + 0.05],
                        rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                        scale: [s, s, s],
                    },
                );
            } else {
                // Still no bounds — unlikely but possible. Log and leave as-is.
                eprintln!(
                    "[AssetSystem] remeasure: still no bounds for preview shell {:?}",
                    preview_shell,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::World;
    use crate::engine::ecs::command_queue::CommandQueue;
    use crate::engine::ecs::component::TextComponent;
    use crate::engine::graphics::RenderAssets;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_asset_directory() -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let tmp_dir = std::env::temp_dir().join(format!("cat_engine_assets_{}", now));
        std::fs::create_dir_all(&tmp_dir).expect("create temp dir");
        tmp_dir
    }

    fn first_text_under(world: &World, root: ComponentId) -> Option<String> {
        if let Some(text) = world.get_component_by_id_as::<TextComponent>(root) {
            return Some(text.text.clone());
        }

        for child in world.children_of(root) {
            if let Some(text) = first_text_under(world, *child) {
                return Some(text);
            }
        }

        None
    }

    #[test]
    fn scan_assets_dir_loads_mms_function_exports() {
        let tmp_dir = temp_asset_directory();
        let asset_path = tmp_dir.join("test_asset.mms");
        std::fs::write(
            &asset_path,
            r#"
                export fn example() {
                    let root = T {}
                    return root
                }
            "#,
        )
        .expect("write asset file");

        let mut system = AssetSystem::new();
        system.scan_assets_dir(&tmp_dir).expect("scan assets dir");

        assert_eq!(system.items.len(), 1);
        assert_eq!(system.items[0].export_name, "example");
        assert!(system.items[0].title.contains("test_asset::example"));
    }

    #[test]
    fn load_module_can_spawn_component() {
        let tmp_dir = temp_asset_directory();
        let asset_path = tmp_dir.join("test_asset.mms");
        std::fs::write(
            &asset_path,
            r#"
                export fn example() {
                    let root = T {}
                    return root
                }
            "#,
        )
        .expect("write asset file");

        let mut system = AssetSystem::new();
        system.load_module(asset_path.clone()).expect("load module");

        let item = &system.items[0];
        let mut world = World::default();
        let render_assets = RenderAssets::new();
        let mut emit = CommandQueue::new();
        let _id = system
            .spawn_asset_component(item, vec![], &mut world, &mut emit)
            .expect("spawn component");
    }

    #[test]
    fn build_asset_item_shell_uses_asset_title_as_label() {
        let tmp_dir = temp_asset_directory();
        let asset_path = tmp_dir.join("test_asset.mms");
        std::fs::write(
            &asset_path,
            r#"
                export fn example() {
                    return T {}
                }
            "#,
        )
        .expect("write asset file");

        let mut system = AssetSystem::new();
        system.load_module(asset_path).expect("load module");

        let item = &system.items[0];
        let mut world = World::default();
        let render_assets = RenderAssets::new();
        let mut emit = CommandQueue::new();
        let item_root = system
            .build_asset_item_shell(&mut world, &render_assets, &mut emit, item, 0)
            .expect("build asset item");

        assert_eq!(
            first_text_under(&world, item_root),
            Some("test_asset::example".to_string())
        );
    }

    #[test]
    fn build_asset_item_shell_keeps_item_when_preview_fails() {
        let tmp_dir = temp_asset_directory();
        let asset_path = tmp_dir.join("bad_preview.mms");
        std::fs::write(
            &asset_path,
            r#"
                export fn broken_preview() {
                    return null
                }
            "#,
        )
        .expect("write asset file");

        let mut system = AssetSystem::new();
        system.load_module(asset_path).expect("load module");

        let item = &system.items[0];
        let mut world = World::default();
        let render_assets = RenderAssets::new();
        let mut emit = CommandQueue::new();
        let item_root = system
            .build_asset_item_shell(&mut world, &render_assets, &mut emit, item, 0)
            .expect("build asset item despite preview failure");

        assert_eq!(world.component_label(item_root), Some("asset_item"));
        assert_eq!(
            first_text_under(&world, item_root),
            Some("bad_preview::broken_preview".to_string())
        );
    }

    #[test]
    fn spawn_assets_panel_shows_assets_dir_in_title() {
        let tmp_dir = temp_asset_directory();
        let asset_path = tmp_dir.join("test_asset.mms");
        std::fs::write(
            &asset_path,
            r#"
                export fn example() {
                    let root = T {}
                    return root
                }
            "#,
        )
        .expect("write asset file");

        let mut system = AssetSystem::new();
        system.scan_assets_dir(&tmp_dir).expect("scan assets dir");

        let mut world = World::default();
        let render_assets = RenderAssets::new();
        let mut emit = CommandQueue::new();
        let parent = world.add_component_boxed_named("parent", Box::new(TransformComponent::new()));

        let wrapper = system
            .spawn_assets_panel(
                &mut world,
                &render_assets,
                &mut emit,
                parent,
                (0.0, 0.0, 0.0),
            )
            .expect("spawn assets panel");

        let title_bar = world
            .find_component(wrapper, "#title_bar")
            .expect("expected title bar component");

        let title_position_transform = world
            .children_of(title_bar)
            .iter()
            .copied()
            .find(|&child| {
                world
                    .get_component_record(child)
                    .map(|node| node.component_type == "transform")
                    .unwrap_or(false)
            })
            .expect("expected title position transform child");

        let title_text = world
            .children_of(title_position_transform)
            .iter()
            .copied()
            .find(|&child| {
                world
                    .get_component_record(child)
                    .map(|node| node.component_type == "text")
                    .unwrap_or(false)
            })
            .expect("expected title text component");

        let text_value = world
            .get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(title_text)
            .expect("expected text component")
            .text
            .clone();

        assert_eq!(text_value, format!("Assets: {}", tmp_dir.display()));
    }
}
