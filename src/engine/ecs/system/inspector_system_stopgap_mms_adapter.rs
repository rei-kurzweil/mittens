use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::engine::ecs::component::TransformComponent;
use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::system::editor_system::select_editor_target;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, SignalKind, World};
use crate::meow_meow::component_registry::{spawn_tree_uninitialized, subtree_to_ce_ast};
use crate::meow_meow::object::{CeChild, MaterializedCE, Value};
use crate::meow_meow::runner::MeowMeowRunner;
use crate::meow_meow::unparser::unparse_component;

const PANEL_LAYOUT_MOUNT_NAME: &str = "editor_panel_layout_mount";
const PANEL_LAYOUT_ROOT_NAME: &str = "editor_panel_layout_root";
const EDITOR_RUNTIME_UI_ROOT_NAME: &str = "editor_runtime_ui_root";
const WORLD_PANEL_SHELL_NAME: &str = "editor_world_panel_shell";
const INSPECTOR_PANEL_SHELL_NAME: &str = "editor_inspector_panel_shell";
const WORLD_PANEL_ROOT_SELECTOR: &str = "#world_panel_root";
const WORLD_PANEL_CONTENT_ROOT_SELECTOR: &str = "#world_panel_content_root";
const WORLD_CONTENT_SLOT_SELECTOR: &str = "#world_panel_root #content_slot";
const INSPECTOR_PANEL_ROOT_SELECTOR: &str = "#inspector_panel_root";
const INSPECTOR_PANEL_CONTENT_ROOT_SELECTOR: &str = "#inspector_panel_content_root";
const INSPECTOR_CONTENT_SLOT_SELECTOR: &str = "#inspector_panel_root #content_slot";
const PANEL_STATUS_ROOT_SELECTOR: &str = "#panel_status_root";
const PANEL_STATUS_WRAP_SELECTOR: &str = "#save_status_wrap";
const PANEL_STATUS_VALUE_SELECTOR: &str = "#panel_status_value";
const SAVE_BUTTON_SELECTOR: &str = "#save_button";
const LOAD_BUTTON_SELECTOR: &str = "#load_button";
const ITEM_PREFIX: &str = "item_";
const PANEL_LAYOUT_TEXT_SCALE: f64 = 0.08;
const WORLD_PANEL_WIDTH_GU: f64 = 29.5;
const WORLD_PANEL_TOTAL_HEIGHT_GU: f64 = 60.5;
const INSPECTOR_PANEL_WIDTH_GU: f64 = 22.0;
const INSPECTOR_PANEL_TOTAL_HEIGHT_GU: f64 = 57.5;
const PANEL_LAYOUT_GAP_GU: f64 = 2.0;

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorldPanelModel {
    title: String,
    items: Vec<String>,
    selected_index: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorldPanelItem {
    component_id: ComponentId,
    label: String,
    depth: usize,
    has_children: bool,
}

#[derive(Debug)]
pub(crate) struct InspectorSystemStopgapMmsAdapter {
    reconciler: InspectorSystemStopgapMmsReconciler,
    installed_editor_roots: HashSet<ComponentId>,
    selected_components: Arc<Mutex<HashMap<ComponentId, Option<ComponentId>>>>,
    runtime_ui_roots: Arc<Mutex<HashMap<ComponentId, ComponentId>>>,
}

impl Default for InspectorSystemStopgapMmsAdapter {
    fn default() -> Self {
        Self {
            reconciler: InspectorSystemStopgapMmsReconciler,
            installed_editor_roots: HashSet::new(),
            selected_components: Arc::new(Mutex::new(HashMap::new())),
            runtime_ui_roots: Arc::new(Mutex::new(HashMap::new())),
        }
    }
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
        inspector_panel_pos: (f32, f32, f32),
    ) {
        {
            let mut selected_components = self.selected_components.lock().expect("selected component mutex poisoned");
            selected_components
                .entry(editor_root)
                .or_insert(None);
        }

        let runtime_ui_root = self.get_or_create_runtime_ui_root(world, editor_root);

        println!(
            "[InspectorSystem][debug] setup_panels_for_editor editor_root={editor_root:?} runtime_ui_root={runtime_ui_root:?} world_panel_pos={:?} inspector_panel_pos={:?}",
            world_panel_pos,
            inspector_panel_pos,
        );

        let model = build_world_panel_model(world, editor_root, runtime_ui_root, None);
        let inspector_model = build_inspector_panel_model(None);

        self.reconciler
            .reconcile_panel_layout(
            world,
            emit,
            editor_root,
            runtime_ui_root,
            world_panel_pos,
            inspector_panel_pos,
            &model,
            &inspector_model,
        );

        self.install_scoped_handlers_for_editor(rx, editor_root);
    }

    fn install_scoped_handlers_for_editor(&mut self, rx: &mut RxWorld, editor_root: ComponentId) {
        if self.installed_editor_roots.contains(&editor_root) {
            return;
        }
        self.installed_editor_roots.insert(editor_root);

        let selected_components = Arc::clone(&self.selected_components);
        let runtime_ui_roots = Arc::clone(&self.runtime_ui_roots);

        rx.add_handler_closure(SignalKind::Click, editor_root, move |world, emit, signal| {
            let Some(EventSignal::Click { renderable, .. }) = signal.event.as_ref() else {
                return;
            };

            let Some(panel_query_root) = runtime_ui_roots
                .lock()
                .expect("runtime ui roots mutex poisoned")
                .get(&editor_root)
                .copied()
            else {
                return;
            };

            let Some(panel_root) = world.find_component(panel_query_root, WORLD_PANEL_ROOT_SELECTOR) else {
                return;
            };
            if !is_descendant_or_self(world, panel_root, *renderable) {
                return;
            }

            let Some(status_wrap) = world.find_component(panel_query_root, PANEL_STATUS_WRAP_SELECTOR) else {
                return;
            };

            let Some(content_slot) = world.find_component(panel_query_root, WORLD_CONTENT_SLOT_SELECTOR) else {
                return;
            };
            let Some(inspector_content_slot) = world.find_component(panel_query_root, INSPECTOR_CONTENT_SLOT_SELECTOR) else {
                return;
            };

            if let Some(status_text) = panel_click_status(world, panel_query_root, *renderable) {
                if panel_status_text(world, panel_query_root).as_deref() != Some(status_text.as_str()) {
                    rerender_world_panel_status(world, emit, panel_query_root, status_wrap, &status_text);
                }
                return;
            }

            let Some(row_name) = clicked_named_ancestor(world, *renderable, ITEM_PREFIX) else {
                return;
            };
            let Some(row_index) = parse_item_index(&row_name) else {
                return;
            };

            let visible_items = fully_expanded_world_panel_items(world, &[editor_root, panel_query_root]);
            let Some(row) = visible_items.get(row_index).cloned() else {
                return;
            };

            {
                let mut selected = selected_components.lock().expect("selected component mutex poisoned");
                selected.insert(editor_root, Some(row.component_id));
            }

            if let Some(target_transform) = nearest_transform_ancestor(world, row.component_id) {
                select_editor_target(world, emit, editor_root, target_transform, true);
            }

            let world_panel_model = build_world_panel_model(world, editor_root, panel_query_root, Some(row.component_id));
            rerender_world_panel_content(
                world,
                emit,
                panel_query_root,
                content_slot,
                &world_panel_model.items,
                world_panel_model.selected_index,
            );

            let inspector_model = build_inspector_panel_model(selected_component_mms(world, row.component_id));
            rerender_inspector_panel_content(
                world,
                emit,
                panel_query_root,
                inspector_content_slot,
                &inspector_model.items,
            );

            let status_text = format!("selected {}", row.label);

            if panel_status_text(world, panel_query_root).as_deref() == Some(status_text.as_str()) {
                return;
            }

            rerender_world_panel_status(world, emit, panel_query_root, status_wrap, &status_text);
        });
    }

    fn get_or_create_runtime_ui_root(&self, world: &mut World, editor_root: ComponentId) -> ComponentId {
        {
            let runtime_ui_roots = self.runtime_ui_roots.lock().expect("runtime ui roots mutex poisoned");
            if let Some(root) = runtime_ui_roots.get(&editor_root).copied() {
                return root;
            }
        }

        let runtime_ui_root = world.add_component_boxed_named(
            EDITOR_RUNTIME_UI_ROOT_NAME,
            Box::new(TransformComponent::new()),
        );

        self.runtime_ui_roots
            .lock()
            .expect("runtime ui roots mutex poisoned")
            .insert(editor_root, runtime_ui_root);

        println!(
            "[InspectorSystem][debug] created runtime ui root editor_root={editor_root:?} runtime_ui_root={runtime_ui_root:?}"
        );

        runtime_ui_root
    }
}

impl InspectorSystemStopgapMmsReconciler {
    fn reconcile_panel_layout(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        editor_root: ComponentId,
        panel_query_root: ComponentId,
        world_panel_pos: (f32, f32, f32),
        inspector_panel_pos: (f32, f32, f32),
        model: &WorldPanelModel,
        inspector_model: &InspectorPanelModel,
    ) {
        let existing_world_panel = self.find_world_panel_node(world, panel_query_root, WORLD_PANEL_ROOT_SELECTOR);
        let existing_inspector_panel = self.find_world_panel_node(world, panel_query_root, INSPECTOR_PANEL_ROOT_SELECTOR);

        println!(
            "[InspectorSystem][debug] reconcile_panel_layout editor_root={editor_root:?} panel_query_root={panel_query_root:?} existing_world_panel={existing_world_panel:?} existing_inspector_panel={existing_inspector_panel:?}"
        );

        if existing_world_panel.is_some() && existing_inspector_panel.is_some() {
            println!(
                "[InspectorSystem][debug] panel layout already present for editor_root={editor_root:?}; skipping spawn"
            );
            return;
        }

        self.spawn_panel_layout(
            world,
            emit,
            panel_query_root,
            world_panel_pos,
            inspector_panel_pos,
            model,
            inspector_model,
        );
    }

    fn find_world_panel_node(
        &self,
        world: &World,
        panel_query_root: ComponentId,
        selector: &str,
    ) -> Option<ComponentId> {
        world.find_component(panel_query_root, selector)
    }

    fn spawn_panel_layout(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        panel_query_root: ComponentId,
        world_panel_pos: (f32, f32, f32),
        inspector_panel_pos: (f32, f32, f32),
        model: &WorldPanelModel,
        inspector_model: &InspectorPanelModel,
    ) {
        println!(
            "[InspectorSystem][debug] spawn_panel_layout panel_query_root={panel_query_root:?} world_panel_pos={:?} inspector_panel_pos={:?}",
            world_panel_pos,
            inspector_panel_pos,
        );

        let world_panel = match build_panel_component_expr(
            world,
            emit,
            world_panel_asset_path(),
            "world_panel",
            vec![
                Value::String(model.title.clone()),
                Value::Array(model.items.iter().cloned().map(Value::String).collect()),
            ],
            "world panel",
        ) {
            Some(panel) => panel,
            None => return,
        };

        let inspector_panel = match build_panel_component_expr(
            world,
            emit,
            inspector_panel_asset_path(),
            "inspector_panel",
            vec![
                Value::String(inspector_model.title.clone()),
                Value::Array(inspector_model.items.iter().cloned().map(Value::String).collect()),
            ],
            "inspector panel",
        ) {
            Some(panel) => panel,
            None => return,
        };

        let _ = inspector_panel_pos;
        let anchor_pos = world_panel_pos;

        let total_width_gu = WORLD_PANEL_WIDTH_GU + PANEL_LAYOUT_GAP_GU + INSPECTOR_PANEL_WIDTH_GU;
        let total_height_gu = WORLD_PANEL_TOTAL_HEIGHT_GU.max(INSPECTOR_PANEL_TOTAL_HEIGHT_GU);

        let world_shell = panel_shell_ce(
            WORLD_PANEL_SHELL_NAME,
            WORLD_PANEL_WIDTH_GU,
            WORLD_PANEL_TOTAL_HEIGHT_GU,
            0.0,
            world_panel,
        );
        let inspector_shell = panel_shell_ce(
            INSPECTOR_PANEL_SHELL_NAME,
            INSPECTOR_PANEL_WIDTH_GU,
            INSPECTOR_PANEL_TOTAL_HEIGHT_GU,
            PANEL_LAYOUT_GAP_GU,
            inspector_panel,
        );

        let shared_layout_root = MaterializedCE {
            component_type: "LayoutRoot".to_string(),
            ctor_method: None,
            ctor_args: Vec::new(),
            calls: vec![
                ("available_width".to_string(), vec![Value::Number(total_width_gu)]),
                ("available_height".to_string(), vec![Value::Number(total_height_gu)]),
                ("unit_scale".to_string(), vec![Value::Number(PANEL_LAYOUT_TEXT_SCALE)]),
            ],
            named: vec![("name".to_string(), Value::String(PANEL_LAYOUT_ROOT_NAME.to_string()))],
            positionals: Vec::new(),
            children: vec![CeChild::Spawn(world_shell), CeChild::Spawn(inspector_shell)],
        };

        let overlay_ce = MaterializedCE {
            component_type: "Overlay".to_string(),
            ctor_method: None,
            ctor_args: Vec::new(),
            calls: Vec::new(),
            named: Vec::new(),
            positionals: Vec::new(),
            children: vec![CeChild::Spawn(shared_layout_root)],
        };

        let mount_ce = MaterializedCE {
            component_type: "T".to_string(),
            ctor_method: Some("position".to_string()),
            ctor_args: vec![
                Value::Number(anchor_pos.0 as f64),
                Value::Number(anchor_pos.1 as f64),
                Value::Number(anchor_pos.2 as f64),
            ],
            calls: Vec::new(),
            named: vec![(
                "name".to_string(),
                Value::String(PANEL_LAYOUT_MOUNT_NAME.to_string()),
            )],
            positionals: Vec::new(),
            children: vec![CeChild::Spawn(overlay_ce)],
        };

        let panel_mount_root = match spawn_tree_uninitialized(&mount_ce, world, emit) {
            Ok(component_id) => component_id,
            Err(error) => {
                eprintln!("[InspectorSystemStopgapMmsAdapter] panel layout spawn error: {error}");
                return;
            }
        };

        println!(
            "[InspectorSystem][debug] spawned panel mount root={panel_mount_root:?} name={} anchor_pos={:?}",
            PANEL_LAYOUT_MOUNT_NAME,
            anchor_pos,
        );

        emit.push_intent_now(
            panel_mount_root,
            IntentValue::Attach {
                parents: vec![panel_query_root],
                child: panel_mount_root,
            },
        );

        println!(
            "[InspectorSystem][debug] queued attach panel_mount_root={panel_mount_root:?} -> panel_query_root={panel_query_root:?}"
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InspectorPanelModel {
    title: String,
    items: Vec<String>,
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

    None
}

fn build_world_panel_model(
    world: &World,
    editor_root: ComponentId,
    runtime_ui_root: ComponentId,
    selected_component: Option<ComponentId>,
) -> WorldPanelModel {
    let visible_items = fully_expanded_world_panel_items(world, &[editor_root, runtime_ui_root]);
    let items: Vec<String> = visible_items
        .iter()
        .map(|item| format!("{}{}", "  ".repeat(item.depth), item.label))
        .collect();
    let items = if items.is_empty() { vec!["<empty>".to_string()] } else { items };
    let selected_index = selected_component.and_then(|selected| {
        visible_items
            .iter()
            .position(|item| item.component_id == selected)
            .map(|index| index as i64)
    });

    WorldPanelModel {
        title: "World".to_string(),
        items,
        selected_index,
    }
}

fn build_inspector_panel_model(serialized_mms: Option<String>) -> InspectorPanelModel {
    InspectorPanelModel {
        title: "Inspector".to_string(),
        items: vec![serialized_mms.unwrap_or_else(|| "<nothing selected>".to_string())],
    }
}

fn fully_expanded_world_panel_items(world: &World, excluded_roots: &[ComponentId]) -> Vec<WorldPanelItem> {
    let mut roots: Vec<ComponentId> = world
        .all_components()
        .filter(|&component_id| {
            world.parent_of(component_id).is_none() && !excluded_roots.contains(&component_id)
        })
        .collect();
    roots.sort_by_key(|component_id| format!("{:?}", component_id));

    let mut out = Vec::new();
    for root in roots {
        push_fully_expanded_world_panel_items(world, root, 0, &mut out);
    }
    out
}

fn push_fully_expanded_world_panel_items(
    world: &World,
    component_id: ComponentId,
    depth: usize,
    out: &mut Vec<WorldPanelItem>,
) {
    out.push(WorldPanelItem {
        component_id,
        label: world_panel_item_label(world, component_id),
        depth,
        has_children: !world.children_of(component_id).is_empty(),
    });

    for &child in world.children_of(component_id) {
        push_fully_expanded_world_panel_items(world, child, depth + 1, out);
    }
}

fn parse_item_index(row_name: &str) -> Option<usize> {
    row_name.strip_prefix(ITEM_PREFIX)?.parse().ok()
}

fn panel_status_text(world: &World, panel_query_root: ComponentId) -> Option<String> {
    world
        .find_component(panel_query_root, PANEL_STATUS_VALUE_SELECTOR)
        .and_then(|status_id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(status_id)
                .map(|text| text.text.clone())
        })
}

fn rerender_world_panel_status(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    status_wrap: ComponentId,
    label: &str,
) {
    let module = match MeowMeowRunner::load_module_file(world_panel_status_asset_path()) {
        Ok(module) => module,
        Err(error) => {
            eprintln!("[InspectorSystemStopgapMmsAdapter] world panel status module load error: {error}");
            return;
        }
    };

    let status_value = match MeowMeowRunner::call_mms_module_fn(
        &module,
        "world_panel_status",
        vec![Value::String(label.to_string())],
        None,
        Some(world),
        Some(emit),
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("[InspectorSystemStopgapMmsAdapter] world panel status export call error: {error}");
            return;
        }
    };

    let Value::ComponentExpr(status_root) = status_value else {
        eprintln!(
            "[InspectorSystemStopgapMmsAdapter] world panel status export did not return a component tree"
        );
        return;
    };

    if let Some(existing_status_root) = world.find_component(panel_query_root, PANEL_STATUS_ROOT_SELECTOR) {
        emit.push_intent_now(
            existing_status_root,
            IntentValue::RemoveSubtree {
                component_ids: vec![existing_status_root],
            },
        );
    }

    let spawned_status_root = match spawn_tree_uninitialized(status_root.as_ref(), world, emit) {
        Ok(component_id) => component_id,
        Err(error) => {
            eprintln!("[InspectorSystemStopgapMmsAdapter] world panel status spawn error: {error}");
            return;
        }
    };

    emit.push_intent_now(
        spawned_status_root,
        IntentValue::Attach {
            parents: vec![status_wrap],
            child: spawned_status_root,
        },
    );
}

fn rerender_world_panel_content(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    content_slot: ComponentId,
    items: &[String],
    selected_index: Option<i64>,
) {
    let module = match MeowMeowRunner::load_module_file(world_panel_content_asset_path()) {
        Ok(module) => module,
        Err(error) => {
            eprintln!("[InspectorSystemStopgapMmsAdapter] world panel content module load error: {error}");
            return;
        }
    };

    let content_value = match MeowMeowRunner::call_mms_module_fn(
        &module,
        "world_panel_content_selected",
        vec![
            Value::Array(items.iter().cloned().map(Value::String).collect()),
            Value::Number(selected_index.unwrap_or(-1) as f64),
        ],
        None,
        Some(world),
        Some(emit),
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("[InspectorSystemStopgapMmsAdapter] world panel content export call error: {error}");
            return;
        }
    };

    let Value::ComponentExpr(content_root) = content_value else {
        eprintln!(
            "[InspectorSystemStopgapMmsAdapter] world panel content export did not return a component tree"
        );
        return;
    };

    if let Some(existing_content_root) = world.find_component(panel_query_root, WORLD_PANEL_CONTENT_ROOT_SELECTOR) {
        emit.push_intent_now(
            existing_content_root,
            IntentValue::RemoveSubtree {
                component_ids: vec![existing_content_root],
            },
        );
    }

    let spawned_content_root = match spawn_tree_uninitialized(content_root.as_ref(), world, emit) {
        Ok(component_id) => component_id,
        Err(error) => {
            eprintln!("[InspectorSystemStopgapMmsAdapter] world panel content spawn error: {error}");
            return;
        }
    };

    emit.push_intent_now(
        spawned_content_root,
        IntentValue::Attach {
            parents: vec![content_slot],
            child: spawned_content_root,
        },
    );
}

fn rerender_inspector_panel_content(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    content_slot: ComponentId,
    items: &[String],
) {
    let module = match MeowMeowRunner::load_module_file(inspector_panel_asset_path()) {
        Ok(module) => module,
        Err(error) => {
            eprintln!("[InspectorSystemStopgapMmsAdapter] inspector panel module load error: {error}");
            return;
        }
    };

    let content_value = match MeowMeowRunner::call_mms_module_fn(
        &module,
        "inspector_panel_content",
        vec![Value::Array(items.iter().cloned().map(Value::String).collect())],
        None,
        Some(world),
        Some(emit),
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("[InspectorSystemStopgapMmsAdapter] inspector panel content export call error: {error}");
            return;
        }
    };

    let Value::ComponentExpr(content_root) = content_value else {
        eprintln!(
            "[InspectorSystemStopgapMmsAdapter] inspector panel content export did not return a component tree"
        );
        return;
    };

    if let Some(existing_content_root) = world.find_component(panel_query_root, INSPECTOR_PANEL_CONTENT_ROOT_SELECTOR) {
        emit.push_intent_now(
            existing_content_root,
            IntentValue::RemoveSubtree {
                component_ids: vec![existing_content_root],
            },
        );
    }

    let spawned_content_root = match spawn_tree_uninitialized(content_root.as_ref(), world, emit) {
        Ok(component_id) => component_id,
        Err(error) => {
            eprintln!("[InspectorSystemStopgapMmsAdapter] inspector panel content spawn error: {error}");
            return;
        }
    };

    emit.push_intent_now(
        spawned_content_root,
        IntentValue::Attach {
            parents: vec![content_slot],
            child: spawned_content_root,
        },
    );
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

fn world_panel_status_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/world_panel_status.mms")
}

fn world_panel_content_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/world_panel_content.mms")
}

fn inspector_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/inspector_panel.mms")
}

fn nearest_transform_ancestor(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut cur = Some(start);
    while let Some(node) = cur {
        if world
            .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(node)
            .is_some()
        {
            return Some(node);
        }
        cur = world.parent_of(node);
    }
    None
}

fn selected_component_mms(world: &World, component_id: ComponentId) -> Option<String> {
    let ce = subtree_to_ce_ast(world, component_id).ok()?;
    Some(unparse_component(&ce))
}

fn build_panel_component_expr(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    asset_path: &'static str,
    export_name: &str,
    args: Vec<Value>,
    panel_kind: &str,
) -> Option<MaterializedCE> {
    let module = match MeowMeowRunner::load_module_file(asset_path) {
        Ok(module) => module,
        Err(error) => {
            eprintln!("[InspectorSystemStopgapMmsAdapter] {panel_kind} module load error: {error}");
            return None;
        }
    };

    let panel_value = match MeowMeowRunner::call_mms_module_fn(
        &module,
        export_name,
        args,
        None,
        Some(world),
        Some(emit),
    ) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("[InspectorSystemStopgapMmsAdapter] {panel_kind} export call error: {error}");
            return None;
        }
    };

    let Value::ComponentExpr(panel_root) = panel_value else {
        eprintln!(
            "[InspectorSystemStopgapMmsAdapter] {panel_kind} export did not return a component tree"
        );
        return None;
    };

    Some(*panel_root)
}

fn panel_shell_ce(
    shell_name: &str,
    width_gu: f64,
    height_gu: f64,
    margin_left_gu: f64,
    panel_root: MaterializedCE,
) -> MaterializedCE {
    MaterializedCE {
        component_type: "T".to_string(),
        ctor_method: None,
        ctor_args: Vec::new(),
        calls: Vec::new(),
        named: vec![("name".to_string(), Value::String(shell_name.to_string()))],
        positionals: Vec::new(),
        children: vec![
            CeChild::Spawn(MaterializedCE {
                component_type: "Style".to_string(),
                ctor_method: None,
                ctor_args: Vec::new(),
                calls: vec![
                    ("display".to_string(), vec![Value::String("inline-block".to_string())]),
                    ("width".to_string(), vec![Value::Number(width_gu)]),
                    ("height".to_string(), vec![Value::Number(height_gu)]),
                    ("margin_left".to_string(), vec![Value::Number(margin_left_gu)]),
                ],
                named: Vec::new(),
                positionals: Vec::new(),
                children: Vec::new(),
            }),
            CeChild::Spawn(panel_root),
        ],
    }
}