use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::engine::ecs::system::bounds_system::BoundsSystem;
use crate::engine::ecs::{component::TransformComponent, ComponentId, SignalEmitter, World};
use crate::meow_meow::object::Value;
use crate::meow_meow::runner::{LoadedMmsModule, MeowMeowRunner};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetModuleId(u32);

#[derive(Debug)]
pub struct AssetModule {
    pub id: AssetModuleId,
    pub path: PathBuf,
    pub module: LoadedMmsModule,
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

        let module = MeowMeowRunner::load_module_file(
            normalized_path
                .to_str()
                .ok_or_else(|| format!("non-UTF8 asset path: {}", normalized_path.display()))?,
        )?;

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
                let title = if file_stem == name {
                    name.clone()
                } else if name.starts_with(file_stem) && name.chars().nth(file_stem.len()) == Some('_') {
                    name.clone()
                } else {
                    format!("{}::{}", file_stem, name)
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
            .map(|module| &module.module)
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
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/assets.mms")
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
            "assets",
            vec![
                Value::String(panel_title),
                Value::Array(Vec::new()),
                Value::Array(vec![
                    Value::Number(0.90),
                    Value::Number(1.00),
                    Value::Number(0.92),
                    Value::Number(1.0),
                ]),
                Value::Array(vec! [
                    Value::Number(0.18),
                    Value::Number(0.78),
                    Value::Number(0.22),
                    Value::Number(0.95),
                ]),
                Value::Array(vec! [
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

        emit.push_intent_now(
            wrapper,
            crate::engine::ecs::IntentValue::Attach {
                parents: vec![parent],
                child: wrapper,
            },
        );
        Ok(wrapper)
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

        let item_root = MeowMeowRunner::spawn_mms_module_component_from_file(
            asset_item_path,
            "asset_item",
            vec![
                Value::String(item.title.clone()),
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
                // Find the preview slot inside the spawned item shell.
                // We use a named slot to avoid manual coordinate math in Rust
                // that might diverge from the MMS layout.
                let preview_slot = world
                    .find_component(item_root, "#preview_slot")
                    .unwrap_or(item_root);

                // Calculate the aggregate bounds of the spawned asset so we can
                // auto-scale it to fit the tile.
                let bounds = BoundsSystem::calculate_subtree_local_bounds(world, render_assets, preview_root);
                let scale: f32;
                let offset: [f32; 3];

                if let Some(b) = bounds {
                    // Previews are still way too big, let's scale them down much more aggressively.
                    // The user specifically asked for "exactly.. 0.2 by 0.2 by 0.2 units max"
                    let target_max_gu = 0.2_f32;
                    let current_max_gu = b.max_dimension().max(1e-6);
                    scale = target_max_gu / current_max_gu;

                    let center = b.center();
                    // offset = -center * scale
                    offset = [-center[0] * scale, -center[1] * scale, -center[2] * scale];
                } else {
                    // Fallback for assets with no renderables or unknown bounds (e.g. logic modules, or GLTF loading)
                    // If we don't know the bounds, assume it might be a 1m object and scale accordingly.
                    scale = 0.5; // Aggressive reduction for safety
                    offset = [0.0, 0.0, 0.0];
                }

                // We center the asset mesh around its own local origin.
                // The `preview_slot` in `asset_item.mms` uses `Style { text_align("center"), vertical_align("middle") }`
                // which the `LayoutSystem` (specifically in `block.rs`) now uses to center the immediate child's 
                // origin within the slot's content box.
                let preview_shell = world.add_component_boxed_named(
                    "asset_preview_shell",
                    Box::new(
                        TransformComponent::new()
                            .with_position(offset[0], offset[1], offset[2] + 0.05)
                            .with_scale(scale, scale, scale),
                    ),
                );

                world
                    .add_child(preview_shell, preview_root)
                    .map_err(|e| format!("attach preview failed: {e}"))?;
                world
                    .add_child(preview_slot, preview_shell)
                    .map_err(|e| format!("attach preview shell failed: {e}"))?;
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::command_queue::CommandQueue;
    use crate::engine::ecs::component::TextComponent;
    use crate::engine::ecs::World;
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
        let mut emit = CommandQueue::new();
        let item_root = system
            .build_asset_item_shell(&mut world, &mut emit, item, 0)
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
        let mut emit = CommandQueue::new();
        let item_root = system
            .build_asset_item_shell(&mut world, &mut emit, item, 0)
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
        let mut emit = CommandQueue::new();
        let parent = world.add_component_boxed_named("parent", Box::new(TransformComponent::new()));

        let wrapper = system
            .spawn_assets_panel(&mut world, &mut emit, parent, (0.0, 0.0, 0.0))
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
