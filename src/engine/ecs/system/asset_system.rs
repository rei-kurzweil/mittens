use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::engine::ecs::{
    component::{TransformComponent, TextComponent, StyleComponent},
    ComponentId, SignalEmitter, World,
};
use crate::engine::ecs::component::style::{Display, EdgeInsets, SizeDimension};
use crate::meow_meow::object::{MaterializedCE, Value};
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
}

#[derive(Debug, Clone)]
pub enum AssetSource {
    MmsModule { module_id: AssetModuleId, export_name: String },
    RustFactory { factory_name: String },
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
        let entries = std::fs::read_dir(path).map_err(|e| format!("cannot read assets dir '{}': {e}", path.display()))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("cannot read assets dir entry: {e}"))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("mms") {
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

        let module = MeowMeowRunner::load_module_file(normalized_path.to_str().ok_or_else(|| {
            format!("non-UTF8 asset path: {}", normalized_path.display())
        })?)?;

        let module_id = AssetModuleId(self.next_module_id);
        self.next_module_id += 1;

        let file_stem = normalized_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("asset");

        for (name, value) in &module.named_exports {
            if matches!(value, Value::Function { .. }) {
                self.items.push(AssetItem {
                    module_id,
                    export_name: name.clone(),
                    title: format!("{}::{}", file_stem, name),
                    description: None,
                    category: None,
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
        self.modules.get(&item.module_id).map(|module| &module.module)
    }

    pub fn asset_function(&self, item: &AssetItem) -> Option<&Value> {
        self.get_item_module(item)
            .and_then(|module| module.named_export(&item.export_name))
    }

    fn materialize_asset_component_expr(
        &self,
        item: &AssetItem,
        args: Vec<Value>,
        world_host: Option<&mut World>,
        emit: Option<&mut dyn SignalEmitter>,
    ) -> Result<MaterializedCE, String> {
        let module = self.get_item_module(item).ok_or_else(|| {
            format!("asset module not loaded for item '{}::{}'", item.title, item.export_name)
        })?;

        MeowMeowRunner::materialize_mms_module_component(module, &item.export_name, args, world_host, emit)
    }

    pub fn spawn_asset_component(
        &self,
        item: &AssetItem,
        args: Vec<Value>,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
    ) -> Result<ComponentId, String> {
        let module = self.modules.get(&item.module_id).ok_or_else(|| {
            format!("asset module not loaded for item '{}::{}'", item.title, item.export_name)
        })?;

        MeowMeowRunner::spawn_mms_module_component(&module.module, &item.export_name, args, None, world, emit)
    }

    pub fn spawn_asset_component_uninitialized(
        &self,
        item: &AssetItem,
        args: Vec<Value>,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
    ) -> Result<ComponentId, String> {
        let module = self
            .modules
            .get(&item.module_id)
            .ok_or_else(|| format!("asset module not loaded for item '{}::{}'", item.title, item.export_name))?;

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
        emit: &mut dyn SignalEmitter,
        parent: ComponentId,
        position: (f32, f32, f32),
    ) -> Result<ComponentId, String> {
        if self.items.is_empty() {
            return Err("no asset items available".to_string());
        }

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
                Value::Array(vec![Value::Number(1.0), Value::Number(1.0), Value::Number(1.0), Value::Number(1.0)]),
                Value::Array(vec![Value::Number(0.15), Value::Number(0.15), Value::Number(0.15), Value::Number(1.0)]),
                Value::Array(vec![Value::Number(0.25), Value::Number(0.25), Value::Number(0.25), Value::Number(1.0)]),
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

        let selection_root = world
            .find_component(panel_root, "#assets_selection")
            .ok_or_else(|| "assets panel missing Selection root".to_string())?;
        let content_root = world
            .find_component(selection_root, "#assets_content_area")
            .ok_or_else(|| "assets panel missing content area".to_string())?;

        for (index, item) in self.items.iter().enumerate() {
            let item_root = self.build_asset_item_shell(world, emit, item, index)?;
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

    fn build_asset_item_shell(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        item: &AssetItem,
        _index: usize,
    ) -> Result<ComponentId, String> {
        let item_root = world.add_component_boxed_named(
            "asset_item",
            Box::new(TransformComponent::new()),
        );

        let mut style = StyleComponent::new();
        style.display = Some(Display::InlineBlock);
        style.width = SizeDimension::GlyphUnits(18.0);
        style.height = SizeDimension::GlyphUnits(20.0);
        style.margin = EdgeInsets::all(1.0);
        style.background_color = Some([0.25, 0.25, 0.25, 1.0]);
        style.font_size = SizeDimension::GlyphUnits(1.0);
        style.color = Some([0.9, 0.9, 0.9, 1.0]);

        let style_id = world.add_component_boxed(Box::new(style));
        world
            .add_child(item_root, style_id)
            .map_err(|e| format!("attach asset item style failed: {e}"))?;

        let label_root = world.add_component_boxed_named(
            "asset_item_label",
            Box::new(TransformComponent::new().with_position(1.0, 1.0, 0.0)),
        );
        world
            .add_child(item_root, label_root)
            .map_err(|e| format!("attach asset item label failed: {e}"))?;

        let text_id = world.add_component_boxed(Box::new(TextComponent::new(item.title.clone())));
        world
            .add_child(label_root, text_id)
            .map_err(|e| format!("attach asset item text failed: {e}"))?;

        let _preview_root = self.spawn_asset_component(item, vec![], world, emit)?;
        Ok(item_root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::command_queue::CommandQueue;
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
    fn load_module_can_materialize_component_expr() {
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
        let _world = World::default();
        let expr = system
            .materialize_asset_component_expr(item, vec![], None, None)
            .expect("materialize expr");

        assert_eq!(expr.component_type, "T");
        assert_eq!(expr.children.len(), 0);
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
