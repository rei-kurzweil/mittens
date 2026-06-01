# Asset discovery

## Asset discovery ownership

The asset discovery and enumeration logic should live in an editor asset system, not in a stopgap MMS adapter.
That means:

- the editor maintains an `AssetsSystem` that scans `assets/components/` for `.mms` files
- it discovers named exports and registers them as available asset factories
- it exposes asset metadata to UI panels and preview generators
- it does not bake the discovery semantics into the MMS evaluator itself

This keeps `MMS -> component` calling as a narrower bridge that only needs to instantiate a selected factory, instead of making the whole asset browser depend on MMS internals.

## Important constraints

- The discovery path should not require factory invocation.
- Preview metadata can be gathered without fully materializing the live component tree.
- The editor asset system should handle caching of module exports and asset metadata.

## Open questions

- Should `AssetFactory` be a first-class concept in the editor world? e.g. `AssetFactoryComponent` or registry entries with opaque handles.
- How do we represent discovered assets in a way that is easy to display in UI panels?
- How much of the current MMS adapter should be kept versus replaced by a generic discovery service?

## Draft Rust shape

```rust
use std::collections::HashMap;
use std::path::PathBuf;

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

#[derive(Debug)]
pub struct AssetSystem {
    modules: HashMap<AssetModuleId, AssetModule>,
    items: Vec<AssetItem>,
    next_module_id: u32,
}

impl AssetSystem {
    pub fn new() -> Self {
        Self {
            modules: HashMap::new(),
            items: Vec::new(),
            next_module_id: 0,
        }
    }

    pub fn scan_assets_dir(&mut self, path: &Path) -> Result<(), String> {
        for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("mms") {
                self.load_module(path)?;
            }
        }
        Ok(())
    }

    pub fn load_module(&mut self, path: PathBuf) -> Result<(), String> {
        let module = MeowMeowRunner::load_module_file(path.to_str().unwrap())?;
        let module_id = AssetModuleId(self.next_module_id);
        self.next_module_id += 1;

        let item_base_title = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        for (name, value) in &module.named_exports {
            if matches!(value, Value::Function { .. }) {
                self.items.push(AssetItem {
                    module_id,
                    export_name: name.clone(),
                    title: format!("{}::{}", item_base_title, name),
                    description: None,
                    category: None,
                });
            }
        }

        self.modules.insert(module_id, AssetModule { id: module_id, path, module });
        Ok(())
    }

    pub fn get_item_module(&self, item: &AssetItem) -> Option<&LoadedMmsModule> {
        self.modules.get(&item.module_id).map(|module| &module.module)
    }
}

pub struct AssetPreview {
    pub asset_item_index: usize,
    pub preview_root: ComponentId,
}
```

### Notes

- `AssetItem` is only metadata; it references a cached module via `module_id`.
- The module cache stores `LoadedMmsModule` once per `.mms` file.
- Preview instantiation is a separate step and may be held in a distinct `AssetPreview` cache.
- `AssetSource::MmsModule` is the minimal reference needed for actual materialization.
