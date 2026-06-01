use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::engine::ecs::{ComponentId, SignalEmitter, World};
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
pub struct AssetsSystem {
    modules: HashMap<AssetModuleId, AssetModule>,
    module_paths: HashMap<PathBuf, AssetModuleId>,
    pub items: Vec<AssetItem>,
    next_module_id: u32,
}

impl AssetsSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scan_assets_dir(&mut self, path: &Path) -> Result<(), String> {
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

    pub fn materialize_asset_component_expr(
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::{EventSignal, IntentSignal, World};
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

        let mut system = AssetsSystem::new();
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

        let mut system = AssetsSystem::new();
        system.load_module(asset_path.clone()).expect("load module");

        let item = &system.items[0];
        let _world = World::default();
        let expr = system
            .materialize_asset_component_expr(item, vec![], None, None)
            .expect("materialize expr");

        assert_eq!(expr.component_type, "T");
        assert_eq!(expr.children.len(), 0);
    }
}
