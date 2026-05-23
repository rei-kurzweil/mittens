use std::collections::HashSet;

use crate::engine::ecs::component::TextComponent;
use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, SignalKind, World};
use crate::meow_meow::component_registry::spawn_tree_uninitialized;
use crate::meow_meow::object::{CeChild, MaterializedCE, Value};
use crate::meow_meow::runner::MeowMeowRunner;

const WORLD_PANEL_MOUNT_NAME: &str = "editor_world_panel_mount";
const WORLD_PANEL_ROOT_SELECTOR: &str = "#world_panel_root";
const PANEL_STATUS_VALUE_SELECTOR: &str = "#panel_status_value";
const SAVE_BUTTON_SELECTOR: &str = "#save_button";
const LOAD_BUTTON_SELECTOR: &str = "#load_button";
const ITEM_PREFIX: &str = "item_";

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorldPanelModel {
    title: String,
    items: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct InspectorSystemStopgapMmsAdapter {
    reconciler: InspectorSystemStopgapMmsReconciler,
    installed_editor_roots: HashSet<ComponentId>,
}

#[derive(Debug, Default)]
struct InspectorSystemStopgapMmsReconciler;

impl InspectorSystemStopgapMmsAdapter {
    pub fn setup_panels_for_editor(
        &mut self,
        rx: &mut RxWorld,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        editor_root: ComponentId,
        world_panel_pos: (f32, f32, f32),
    ) {
        self.reconciler
            .reconcile_world_panel(world, emit, editor_root, world_panel_pos);

        self.install_scoped_handlers_for_editor(rx, editor_root);
    }

    fn install_scoped_handlers_for_editor(&mut self, rx: &mut RxWorld, editor_root: ComponentId) {
        if self.installed_editor_roots.contains(&editor_root) {
            return;
        }
        self.installed_editor_roots.insert(editor_root);

        rx.add_handler_closure(SignalKind::Click, editor_root, move |world, emit, signal| {
            let Some(EventSignal::Click { renderable, .. }) = signal.event.as_ref() else {
                return;
            };

            let Some(panel_root) = world.find_component(editor_root, WORLD_PANEL_ROOT_SELECTOR) else {
                return;
            };
            if !is_descendant_or_self(world, panel_root, *renderable) {
                return;
            }

            let Some(status_id) = world.find_component(editor_root, PANEL_STATUS_VALUE_SELECTOR) else {
                return;
            };

            let status_text = panel_click_status(world, editor_root, *renderable)
                .unwrap_or_else(|| "idle".to_string());

            let needs_update = world
                .get_component_by_id_as::<TextComponent>(status_id)
                .map(|text| text.text != status_text)
                .unwrap_or(true);
            if !needs_update {
                return;
            }

            emit.push_intent_now(
                status_id,
                IntentValue::SetText {
                    component_ids: vec![status_id],
                    text: status_text,
                },
            );
        });
    }
}

impl InspectorSystemStopgapMmsReconciler {
    fn reconcile_world_panel(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        editor_root: ComponentId,
        world_panel_pos: (f32, f32, f32),
    ) {
        if self
            .find_world_panel_node(world, editor_root, WORLD_PANEL_ROOT_SELECTOR)
            .is_some()
        {
            return;
        }

        let model = self.build_world_panel_model(world, editor_root);
        self.spawn_world_panel(world, emit, editor_root, world_panel_pos, &model);
    }

    fn find_world_panel_node(
        &self,
        world: &World,
        editor_root: ComponentId,
        selector: &str,
    ) -> Option<ComponentId> {
        world.find_component(editor_root, selector)
    }

    fn build_world_panel_model(&self, world: &World, editor_root: ComponentId) -> WorldPanelModel {
        let mut items: Vec<String> = world
            .all_components()
            .filter(|&component_id| {
                world.parent_of(component_id).is_none() && component_id != editor_root
            })
            .map(|component_id| world_panel_item_label(world, component_id))
            .collect();

        if items.is_empty() {
            items.push("<empty>".to_string());
        }

        WorldPanelModel {
            title: "World".to_string(),
            items,
        }
    }

    fn spawn_world_panel(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        editor_root: ComponentId,
        world_panel_pos: (f32, f32, f32),
        model: &WorldPanelModel,
    ) {
        let module = match MeowMeowRunner::load_module_file(world_panel_asset_path()) {
            Ok(module) => module,
            Err(error) => {
                eprintln!("[InspectorSystemStopgapMmsAdapter] world panel module load error: {error}");
                return;
            }
        };

        let args = vec![
            Value::String(model.title.clone()),
            Value::Array(model.items.iter().cloned().map(Value::String).collect()),
        ];
        let panel_value = match MeowMeowRunner::call_mms_module_fn(
            &module,
            "world_panel",
            args,
            None,
            Some(world),
            Some(emit),
        ) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("[InspectorSystemStopgapMmsAdapter] world panel export call error: {error}");
                return;
            }
        };

        let Value::ComponentExpr(panel_root) = panel_value else {
            eprintln!(
                "[InspectorSystemStopgapMmsAdapter] world panel export did not return a component tree"
            );
            return;
        };

        let mount_ce = MaterializedCE {
            component_type: "T".to_string(),
            ctor_method: Some("position".to_string()),
            ctor_args: vec![
                Value::Number(world_panel_pos.0 as f64),
                Value::Number(world_panel_pos.1 as f64),
                Value::Number(world_panel_pos.2 as f64),
            ],
            calls: Vec::new(),
            named: vec![(
                "name".to_string(),
                Value::String(WORLD_PANEL_MOUNT_NAME.to_string()),
            )],
            positionals: Vec::new(),
            children: vec![CeChild::Spawn(*panel_root)],
        };

        let panel_mount_root = match spawn_tree_uninitialized(&mount_ce, world, emit) {
            Ok(component_id) => component_id,
            Err(error) => {
                eprintln!("[InspectorSystemStopgapMmsAdapter] world panel spawn error: {error}");
                return;
            }
        };

        emit.push_intent_now(
            panel_mount_root,
            IntentValue::Attach {
                parents: vec![editor_root],
                child: panel_mount_root,
            },
        );
    }
}

fn panel_click_status(world: &World, editor_root: ComponentId, renderable: ComponentId) -> Option<String> {
    let save_button = world.find_component(editor_root, SAVE_BUTTON_SELECTOR);
    if save_button.is_some_and(|button| is_descendant_or_self(world, button, renderable)) {
        return Some("save requested".to_string());
    }

    let load_button = world.find_component(editor_root, LOAD_BUTTON_SELECTOR);
    if load_button.is_some_and(|button| is_descendant_or_self(world, button, renderable)) {
        return Some("load requested".to_string());
    }

    clicked_named_ancestor(world, renderable, ITEM_PREFIX)
        .map(|row_name| format!("selected {row_name}"))
}

fn clicked_named_ancestor(world: &World, node: ComponentId, prefix: &str) -> Option<String> {
    let mut current = Some(node);
    while let Some(component_id) = current {
        if let Some(label) = world.component_label(component_id) {
            if label.starts_with(prefix) {
                return Some(label.to_string());
            }
        }
        current = world.parent_of(component_id);
    }
    None
}

fn is_descendant_or_self(world: &World, ancestor: ComponentId, node: ComponentId) -> bool {
    let mut current = Some(node);
    while let Some(component_id) = current {
        if component_id == ancestor {
            return true;
        }
        current = world.parent_of(component_id);
    }
    false
}

fn world_panel_item_label(world: &World, component_id: ComponentId) -> String {
    if let Some(label) = world.component_label(component_id) {
        if !label.is_empty() {
            return label.to_string();
        }
    }

    world
        .component_name(component_id)
        .map(|name| name.to_string())
        .unwrap_or_else(|| format!("component_{:?}", component_id))
}

fn world_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/world_panel.mms")
}