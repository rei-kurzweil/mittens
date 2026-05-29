use std::sync::{Arc, Mutex};

use crate::engine::ecs::component::{
    ColorComponent, Display, EdgeInsets, Overflow, RaycastableComponent, SizeDimension,
    StyleComponent, TextComponent,
};
use crate::engine::ecs::component::TransformComponent;
use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, SignalKind, World};
use crate::meow_meow::component_registry::spawn_tree_uninitialized;
use crate::meow_meow::object::{CeChild, MaterializedCE, Value};
use crate::meow_meow::runner::MeowMeowRunner;

const PANEL_LAYOUT_MOUNT_NAME: &str = "editor_panel_layout_mount";
const PANEL_LAYOUT_ROOT_NAME: &str = "editor_panel_layout_root";
const EDITOR_RUNTIME_UI_ROOT_NAME: &str = "editor_runtime_ui_root";
const WORLD_PANEL_SHELL_NAME: &str = "editor_world_panel_shell";
const INSPECTOR_PANEL_SHELL_NAME: &str = "editor_inspector_panel_shell";
const WORLD_PANEL_ROOT_SELECTOR: &str = "#world_panel_root";
const WORLD_PANEL_CONTENT_ROOT_SELECTOR: &str = "#world_panel_content_root";
const PANEL_CONTENT_SLOT_SELECTOR: &str = "#content_slot";
const INSPECTOR_PANEL_ROOT_SELECTOR: &str = "#inspector_panel_root";
const INSPECTOR_PANEL_CONTENT_ROOT_SELECTOR: &str = "#inspector_panel_content_root";
const PANEL_STATUS_ROOT_SELECTOR: &str = "#panel_status_root";
const PANEL_STATUS_WRAP_SELECTOR: &str = "#save_status_wrap";
const PANEL_STATUS_VALUE_SELECTOR: &str = "#panel_status_value";
const SAVE_BUTTON_SELECTOR: &str = "#save_button";
const LOAD_BUTTON_SELECTOR: &str = "#load_button";
const ITEM_PREFIX: &str = "item_";
const INSPECTOR_ITEM_PREFIX: &str = "inspector_item_";
const PANEL_LAYOUT_TEXT_SCALE: f64 = 0.08;
const WORLD_PANEL_WIDTH_GU: f64 = 29.5;
const WORLD_PANEL_TOTAL_HEIGHT_GU: f64 = 60.5;
const INSPECTOR_PANEL_WIDTH_GU: f64 = 22.0;
const INSPECTOR_PANEL_TOTAL_HEIGHT_GU: f64 = 57.5;
const PANEL_LAYOUT_GAP_GU: f64 = 2.0;
#[derive(Debug, Clone, PartialEq, Eq)]
struct WorldPanelModel {
    title: String,
    rows: Vec<WorldPanelRow>,
    selected_index: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorldPanelRow {
    target_component: Option<ComponentId>,
    label: String,
    display_label: String,
    kind: WorldPanelRowKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorldPanelRowKind {
    Spacer,
    EditorHeader,
    Info,
    Component,
}

#[derive(Debug)]
pub(crate) struct InspectorSystemStopgapMmsAdapter {
    reconciler: InspectorSystemStopgapMmsReconciler,
    panel_handler_installed: bool,
    panel_layout_spawned: bool,
    selected_component: Arc<Mutex<Option<ComponentId>>>,
    runtime_ui_root: Arc<Mutex<Option<ComponentId>>>,
}

impl Default for InspectorSystemStopgapMmsAdapter {
    fn default() -> Self {
        Self {
            reconciler: InspectorSystemStopgapMmsReconciler,
            panel_handler_installed: false,
            panel_layout_spawned: false,
            selected_component: Arc::new(Mutex::new(None)),
            runtime_ui_root: Arc::new(Mutex::new(None)),
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
        let runtime_ui_root = self.get_or_create_runtime_ui_root(world);

        println!(
            "[InspectorSystem][debug] setup_panels_for_editor editor_root={editor_root:?} runtime_ui_root={runtime_ui_root:?} world_panel_pos={:?} inspector_panel_pos={:?}",
            world_panel_pos,
            inspector_panel_pos,
        );

        let model = build_world_panel_model(world, None);
        let inspector_model = build_inspector_panel_model(world, None);

        self.reconciler
            .reconcile_panel_layout(
            world,
            emit,
            &mut self.panel_layout_spawned,
            runtime_ui_root,
            world_panel_pos,
            inspector_panel_pos,
            &model,
            &inspector_model,
        );

        self.refresh_world_panels(world, emit);

        self.install_shared_panel_handlers(rx, runtime_ui_root);
    }

    fn install_shared_panel_handlers(&mut self, rx: &mut RxWorld, panel_query_root: ComponentId) {
        if self.panel_handler_installed {
            return;
        }
        self.panel_handler_installed = true;

        let selected_component = Arc::clone(&self.selected_component);

        rx.add_handler_closure(SignalKind::Click, panel_query_root, move |world, emit, signal| {
            let Some(EventSignal::Click { renderable, .. }) = signal.event.as_ref() else {
                return;
            };

            let Some(panel_root) = world.find_component(panel_query_root, WORLD_PANEL_ROOT_SELECTOR) else {
                return;
            };
            if !is_descendant_or_self(world, panel_root, *renderable) {
                return;
            }

            let Some(world_panel_root) = world.find_component(panel_query_root, WORLD_PANEL_ROOT_SELECTOR) else {
                return;
            };

            let Some(status_wrap) = world.find_component(world_panel_root, PANEL_STATUS_WRAP_SELECTOR) else {
                return;
            };

            let Some(content_slot) = world.find_component(world_panel_root, PANEL_CONTENT_SLOT_SELECTOR) else {
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

            let visible_rows = build_world_panel_rows(world);
            let Some(row) = visible_rows.get(row_index).cloned() else {
                return;
            };
            let Some(target_component) = row.target_component else {
                return;
            };

            println!(
                "[InspectorSystem][debug] world panel click target_component={target_component:?} name={:?}",
                world.component_label(target_component).filter(|label| !label.is_empty())
            );

            let previous_selected = *selected_component
                .lock()
                .expect("selected component mutex poisoned");
            {
                let mut selected = selected_component.lock().expect("selected component mutex poisoned");
                *selected = Some(target_component);
            }

            let previous_index = previous_selected.and_then(|selected| {
                visible_rows
                    .iter()
                    .position(|visible_row| visible_row.target_component == Some(selected))
            });

            if let Some(index) = previous_index {
                update_world_panel_row_selection(world, emit, panel_query_root, index, false);
            }
            update_world_panel_row_selection(world, emit, panel_query_root, row_index, true);

            if let Some(inspector_panel_root) = world.find_component(panel_query_root, INSPECTOR_PANEL_ROOT_SELECTOR) {
                if let Some(inspector_content_slot) = world.find_component(inspector_panel_root, PANEL_CONTENT_SLOT_SELECTOR) {
                    let inspector_model = build_inspector_panel_model(world, Some(target_component));
                    rerender_inspector_panel_content(
                        world,
                        emit,
                        panel_query_root,
                        inspector_content_slot,
                        &inspector_model.rows,
                    );
                }
            }

            let _ = content_slot;
            return;
        });
    }

    fn get_or_create_runtime_ui_root(&self, world: &mut World) -> ComponentId {
        {
            let runtime_ui_root = self.runtime_ui_root.lock().expect("runtime ui root mutex poisoned");
            if let Some(root) = *runtime_ui_root {
                return root;
            }
        }

        let runtime_ui_root = world.add_component_boxed_named(
            EDITOR_RUNTIME_UI_ROOT_NAME,
            Box::new(TransformComponent::new()),
        );

        *self.runtime_ui_root
            .lock()
            .expect("runtime ui root mutex poisoned") = Some(runtime_ui_root);

        println!(
            "[InspectorSystem][debug] created runtime ui root runtime_ui_root={runtime_ui_root:?}"
        );

        runtime_ui_root
    }

    fn refresh_world_panels(&self, world: &mut World, emit: &mut dyn SignalEmitter) {
        let Some(panel_query_root) = *self
            .runtime_ui_root
            .lock()
            .expect("runtime ui root mutex poisoned")
        else {
            return;
        };

        let Some(world_panel_root) = world.find_component(panel_query_root, WORLD_PANEL_ROOT_SELECTOR) else {
            return;
        };
        let Some(content_slot) = world.find_component(world_panel_root, PANEL_CONTENT_SLOT_SELECTOR) else {
            return;
        };

        let selected_component = *self
            .selected_component
            .lock()
            .expect("selected component mutex poisoned");
        let model = build_world_panel_model(world, selected_component);
        rerender_world_panel_content(
            world,
            emit,
            panel_query_root,
            content_slot,
            &model.rows,
            model.selected_index,
        );

        let Some(inspector_panel_root) = world.find_component(panel_query_root, INSPECTOR_PANEL_ROOT_SELECTOR) else {
            return;
        };
        let Some(inspector_content_slot) = world.find_component(inspector_panel_root, PANEL_CONTENT_SLOT_SELECTOR) else {
            return;
        };

        let inspector_model = build_inspector_panel_model(world, selected_component);
        rerender_inspector_panel_content(
            world,
            emit,
            panel_query_root,
            inspector_content_slot,
            &inspector_model.rows,
        );

    }
}

impl InspectorSystemStopgapMmsReconciler {
    fn reconcile_panel_layout(
        &self,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
        panel_layout_spawned: &mut bool,
        panel_query_root: ComponentId,
        world_panel_pos: (f32, f32, f32),
        inspector_panel_pos: (f32, f32, f32),
        model: &WorldPanelModel,
        inspector_model: &InspectorPanelModel,
    ) {
        let existing_world_panel = self.find_world_panel_node(world, panel_query_root, WORLD_PANEL_ROOT_SELECTOR);
        let existing_inspector_panel = self.find_world_panel_node(world, panel_query_root, INSPECTOR_PANEL_ROOT_SELECTOR);

        println!(
            "[InspectorSystem][debug] reconcile_panel_layout panel_query_root={panel_query_root:?} existing_world_panel={existing_world_panel:?} existing_inspector_panel={existing_inspector_panel:?}"
        );

        if *panel_layout_spawned {
            println!(
                "[InspectorSystem][debug] panel layout already spawned for panel_query_root={panel_query_root:?}; skipping duplicate spawn"
            );
            return;
        }

        if existing_world_panel.is_some() && existing_inspector_panel.is_some() {
            println!(
                "[InspectorSystem][debug] panel layout already present for panel_query_root={panel_query_root:?}; skipping spawn"
            );
            *panel_layout_spawned = true;
            return;
        }

        *panel_layout_spawned = true;

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
                Value::Array(Vec::new()),
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
                Value::Array(Vec::new()),
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

        if let Some(world_panel_root) = world.find_component(panel_mount_root, WORLD_PANEL_ROOT_SELECTOR) {
            if let Some(content_slot) = world.find_component(world_panel_root, PANEL_CONTENT_SLOT_SELECTOR) {
            rerender_world_panel_content(
                world,
                emit,
                panel_mount_root,
                content_slot,
                &model.rows,
                model.selected_index,
            );
            }
        }

        println!(
            "[InspectorSystem][debug] queued attach panel_mount_root={panel_mount_root:?} -> panel_query_root={panel_query_root:?}"
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InspectorPanelModel {
    title: String,
    rows: Vec<InspectorPanelRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InspectorPanelRow {
    display_label: String,
    kind: InspectorPanelRowKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InspectorPanelRowKind {
    Info,
    Component,
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
    selected_component: Option<ComponentId>,
) -> WorldPanelModel {
    let rows = build_world_panel_rows(world);
    let selected_index = selected_component.and_then(|selected| {
        rows
            .iter()
            .position(|row| row.target_component == Some(selected))
            .map(|index| index as i64)
    });

    WorldPanelModel {
        title: "World".to_string(),
        rows,
        selected_index,
    }
}

fn build_inspector_panel_model(world: &World, selected_component: Option<ComponentId>) -> InspectorPanelModel {
    let rows = selected_component
        .map(|component_id| build_inspector_panel_rows(world, component_id))
        .unwrap_or_else(|| {
            vec![InspectorPanelRow {
                display_label: "<nothing selected>".to_string(),
                kind: InspectorPanelRowKind::Info,
            }]
        });

    InspectorPanelModel {
        title: "Inspector".to_string(),
        rows,
    }
}

fn build_inspector_panel_rows(world: &World, root: ComponentId) -> Vec<InspectorPanelRow> {
    let mut rows = Vec::new();
    push_inspector_panel_rows(world, root, 0, &mut rows);
    rows
}

fn push_inspector_panel_rows(
    world: &World,
    component_id: ComponentId,
    depth: usize,
    out: &mut Vec<InspectorPanelRow>,
) {
    out.push(InspectorPanelRow {
        display_label: format!("{}{}", "  ".repeat(depth), world_panel_item_label(world, component_id)),
        kind: InspectorPanelRowKind::Component,
    });

    for &child in world.children_of(component_id) {
        push_inspector_panel_rows(world, child, depth + 1, out);
    }
}

fn build_world_panel_rows(world: &World) -> Vec<WorldPanelRow> {
    let mut editor_roots: Vec<ComponentId> = world
        .all_components()
        .filter(|&component_id| world.get_component_by_id_as::<crate::engine::ecs::component::EditorComponent>(component_id).is_some())
        .collect();
    editor_roots.sort_by_key(|component_id| component_id_short(*component_id));

    let mut out = Vec::new();
    for (editor_index, editor_root) in editor_roots.into_iter().enumerate() {
        if editor_index > 0 {
            out.push(WorldPanelRow {
                target_component: None,
                label: String::new(),
                display_label: String::new(),
                kind: WorldPanelRowKind::Spacer,
            });
        }

        let header_label = editor_chunk_label(world, editor_root);
        out.push(WorldPanelRow {
            target_component: None,
            label: header_label.clone(),
            display_label: header_label,
            kind: WorldPanelRowKind::EditorHeader,
        });

        for &child in world.children_of(editor_root) {
            push_editor_world_panel_rows(world, child, 0, &mut out);
        }
    }

    if out.is_empty() {
        out.push(WorldPanelRow {
            target_component: None,
            label: "<empty>".to_string(),
            display_label: "<empty>".to_string(),
            kind: WorldPanelRowKind::Info,
        });
    }

    out
}

fn push_editor_world_panel_rows(
    world: &World,
    component_id: ComponentId,
    depth: usize,
    out: &mut Vec<WorldPanelRow>,
) {
    let label = world_panel_item_label(world, component_id);
    out.push(WorldPanelRow {
        target_component: Some(component_id),
        display_label: format!("{}{}", "  ".repeat(depth), label),
        label,
        kind: WorldPanelRowKind::Component,
    });

    for &child in world.children_of(component_id) {
        push_editor_world_panel_rows(world, child, depth + 1, out);
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
    if let Some(existing_status_root) = world.find_component(panel_query_root, PANEL_STATUS_ROOT_SELECTOR) {
        emit.push_intent_now(
            existing_status_root,
            IntentValue::RemoveSubtree {
                component_ids: vec![existing_status_root],
            },
        );
    }

    let spawned_status_root = match MeowMeowRunner::spawn_mms_module_component_uninitialized_from_file(
        world_panel_status_asset_path(),
        "world_panel_status",
        vec![Value::String(label.to_string())],
        world,
        emit,
    ) {
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
    rows: &[WorldPanelRow],
    selected_index: Option<i64>,
) {
    let Some(world_panel_root) = world.find_component(panel_query_root, WORLD_PANEL_ROOT_SELECTOR) else {
        return;
    };

    if let Some(existing_content_root) = world.find_component(world_panel_root, WORLD_PANEL_CONTENT_ROOT_SELECTOR) {
        emit.push_intent_now(
            existing_content_root,
            IntentValue::RemoveSubtree {
                component_ids: vec![existing_content_root],
            },
        );
    }

    let spawned_content_root = spawn_world_panel_content_tree(world, emit, rows, selected_index);
    let _ = world.add_child(content_slot, spawned_content_root);
    world.init_component_tree(spawned_content_root, emit);
}

fn update_world_panel_row_selection(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    row_index: usize,
    selected: bool,
) {
    let Some(world_panel_root) = world.find_component(panel_query_root, WORLD_PANEL_ROOT_SELECTOR) else {
        return;
    };
    let Some(row_id) = world.find_component(world_panel_root, &format!("#{ITEM_PREFIX}{row_index}")) else {
        return;
    };

    let (background_rgba, text_rgba) = if selected {
        ([1.00, 0.88, 0.20, 0.96], [0.06, 0.09, 0.08, 1.0])
    } else {
        ([0.92, 0.97, 0.92, 1.0], [0.06, 0.09, 0.08, 1.0])
    };

    if let Some(bg_id) = world.find_component(row_id, "#__bg") {
        if let Some(bg_color_id) = world.find_component(bg_id, "Color") {
            emit.push_intent_now(
                bg_color_id,
                IntentValue::SetColor {
                    component_ids: vec![bg_color_id],
                    rgba: background_rgba,
                },
            );
        }
    }

    if let Some(text_id) = world.find_component(row_id, "Text") {
        if let Some(text_color_id) = world.find_component(text_id, "Color") {
            emit.push_intent_now(
                text_color_id,
                IntentValue::SetColor {
                    component_ids: vec![text_color_id],
                    rgba: text_rgba,
                },
            );
        }
    }
}

fn rerender_inspector_panel_content(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    content_slot: ComponentId,
    rows: &[InspectorPanelRow],
) {
    let Some(inspector_panel_root) = world.find_component(panel_query_root, INSPECTOR_PANEL_ROOT_SELECTOR) else {
        return;
    };

    if let Some(existing_content_root) = world.find_component(inspector_panel_root, INSPECTOR_PANEL_CONTENT_ROOT_SELECTOR) {
        emit.push_intent_now(
            existing_content_root,
            IntentValue::RemoveSubtree {
                component_ids: vec![existing_content_root],
            },
        );
    }

    let spawned_content_root = spawn_inspector_panel_content_tree(world, emit, rows);
    let _ = world.add_child(content_slot, spawned_content_root);
    world.init_component_tree(spawned_content_root, emit);
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

fn editor_chunk_label(world: &World, editor_root: ComponentId) -> String {
    if let Some(label) = world.component_label(editor_root) {
        if !label.is_empty() {
            return format!("Editor#{label}");
        }
    }

    format!("Editor {{ id={} }}", component_id_short(editor_root))
}

fn component_id_short(component_id: ComponentId) -> String {
    format!("{:?}", component_id)
        .trim_start_matches("ComponentId(")
        .trim_end_matches(')')
        .to_string()
}

fn world_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/world_panel.mms")
}

fn world_panel_status_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/world_panel_status.mms")
}

fn inspector_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/inspector_panel.mms")
}

fn build_panel_component_expr(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    asset_path: &'static str,
    export_name: &str,
    args: Vec<Value>,
    panel_kind: &str,
) -> Option<MaterializedCE> {
    match MeowMeowRunner::materialize_mms_module_component_from_file(
        asset_path,
        export_name,
        args,
        Some(world),
        Some(emit),
    ) {
        Ok(panel_root) => Some(panel_root),
        Err(error) => {
            eprintln!("[InspectorSystemStopgapMmsAdapter] {panel_kind} render error: {error}");
            None
        }
    }
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

fn spawn_world_panel_content_tree(
    world: &mut World,
    _emit: &mut dyn SignalEmitter,
    rows: &[WorldPanelRow],
    selected_index: Option<i64>,
) -> ComponentId {
    let content_root = spawn_block_container(
        world,
        WORLD_PANEL_CONTENT_ROOT_SELECTOR.trim_start_matches('#'),
    );
    let rows_mount = spawn_block_container(world, "rows_mount");
    let _ = world.add_child(content_root, rows_mount);

    for (index, row) in rows.iter().enumerate() {
        let row_root = spawn_world_panel_row_tree(
            world,
            &format!("{ITEM_PREFIX}{index}"),
            row,
            selected_index == Some(index as i64),
        );
        let _ = world.add_child(rows_mount, row_root);
    }

    content_root
}

fn spawn_world_panel_row_tree(
    world: &mut World,
    row_name: &str,
    row: &WorldPanelRow,
    selected: bool,
) -> ComponentId {
    match row.kind {
        WorldPanelRowKind::Spacer => {
            let row_root = world.add_component_boxed_named(
                row_name,
                Box::new(TransformComponent::new()),
            );
            let style = world.add_component_boxed_named(
                format!("{row_name}_style"),
                Box::new({
                    let mut style = StyleComponent::new();
                    style.display = Some(Display::Block);
                    style.width = SizeDimension::Percent(100.0);
                    style.height = SizeDimension::GlyphUnits(0.8);
                    style.overflow = Overflow::Visible;
                    style
                }),
            );
            let _ = world.add_child(row_root, style);
            row_root
        }
        WorldPanelRowKind::EditorHeader | WorldPanelRowKind::Info | WorldPanelRowKind::Component => {
            let (background_rgba, text_rgba, interactive) = match row.kind {
                WorldPanelRowKind::EditorHeader => ([0.18, 0.78, 0.22, 0.95], [0.0, 0.0, 0.0, 1.0], false),
                WorldPanelRowKind::Info => ([0.85, 0.85, 0.85, 1.0], [0.0, 0.0, 0.0, 1.0], false),
                WorldPanelRowKind::Component if selected => ([1.00, 0.88, 0.20, 0.96], [0.06, 0.09, 0.08, 1.0], true),
                WorldPanelRowKind::Component => ([0.92, 0.97, 0.92, 1.0], [0.06, 0.09, 0.08, 1.0], true),
                WorldPanelRowKind::Spacer => unreachable!(),
            };

            let row_root = world.add_component_boxed_named(
                row_name,
                Box::new(TransformComponent::new()),
            );

            if interactive {
                let raycastable = world.add_component_boxed_named(
                    format!("{row_name}_raycastable"),
                    Box::new(RaycastableComponent::click_only()),
                );
                let _ = world.add_child(row_root, raycastable);
            }

            let style = world.add_component_boxed_named(
                format!("{row_name}_style"),
                Box::new({
                    let mut style = StyleComponent::new();
                    style.display = Some(Display::Block);
                    style.width = SizeDimension::Percent(100.0);
                    style.margin = EdgeInsets::axes(0.25, 0.20);
                    style.padding = EdgeInsets::axes(0.55, 0.45);
                    style.font_size = SizeDimension::GlyphUnits(1.0);
                    style.background_color = Some(background_rgba);
                    style.color = Some(text_rgba);
                    style.overflow = Overflow::Visible;
                    style
                }),
            );
            let _ = world.add_child(row_root, style);

            let text_root = world.add_component_boxed_named(
                format!("{row_name}_text_root"),
                Box::new(TransformComponent::new().with_position(0.0, 0.0, 0.02)),
            );
            let text = world.add_component_boxed_named(
                format!("{row_name}_text"),
                Box::new(TextComponent::new(row.display_label.clone())),
            );
            let text_color = world.add_component_boxed_named(
                format!("{row_name}_text_color"),
                Box::new(ColorComponent::rgba(
                    text_rgba[0],
                    text_rgba[1],
                    text_rgba[2],
                    text_rgba[3],
                )),
            );

            let _ = world.add_child(row_root, text_root);
            let _ = world.add_child(text_root, text);
            let _ = world.add_child(text, text_color);

            row_root
        }
    }
}

fn spawn_inspector_panel_content_tree(
    world: &mut World,
    _emit: &mut dyn SignalEmitter,
    rows: &[InspectorPanelRow],
) -> ComponentId {
    let content_root = spawn_block_container(
        world,
        INSPECTOR_PANEL_CONTENT_ROOT_SELECTOR.trim_start_matches('#'),
    );
    let rows_mount = spawn_block_container(world, "rows_mount");
    let _ = world.add_child(content_root, rows_mount);

    for (index, row) in rows.iter().enumerate() {
        let row_root = spawn_inspector_panel_row_tree(
            world,
            &format!("{INSPECTOR_ITEM_PREFIX}{index}"),
            row,
        );
        let _ = world.add_child(rows_mount, row_root);
    }

    content_root
}

fn spawn_inspector_panel_row_tree(
    world: &mut World,
    row_name: &str,
    row: &InspectorPanelRow,
) -> ComponentId {
    let (background_rgba, text_rgba) = match row.kind {
        InspectorPanelRowKind::Info => ([0.85, 0.85, 0.85, 1.0], [0.0, 0.0, 0.0, 1.0]),
        InspectorPanelRowKind::Component => ([0.92, 0.97, 0.92, 1.0], [0.06, 0.09, 0.08, 1.0]),
    };

    let row_root = world.add_component_boxed_named(row_name, Box::new(TransformComponent::new()));
    let style = world.add_component_boxed_named(
        format!("{row_name}_style"),
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::Block);
            style.width = SizeDimension::Percent(100.0);
            style.margin = EdgeInsets::axes(0.25, 0.20);
            style.padding = EdgeInsets::axes(0.55, 0.45);
            style.font_size = SizeDimension::GlyphUnits(1.0);
            style.background_color = Some(background_rgba);
            style.color = Some(text_rgba);
            style.overflow = Overflow::Visible;
            style
        }),
    );
    let _ = world.add_child(row_root, style);

    let text_root = world.add_component_boxed_named(
        format!("{row_name}_text_root"),
        Box::new(TransformComponent::new().with_position(0.0, 0.0, 0.02)),
    );
    let text = world.add_component_boxed_named(
        format!("{row_name}_text"),
        Box::new(TextComponent::new(row.display_label.clone())),
    );
    let text_color = world.add_component_boxed_named(
        format!("{row_name}_text_color"),
        Box::new(ColorComponent::rgba(
            text_rgba[0],
            text_rgba[1],
            text_rgba[2],
            text_rgba[3],
        )),
    );

    let _ = world.add_child(row_root, text_root);
    let _ = world.add_child(text_root, text);
    let _ = world.add_child(text, text_color);

    row_root
}

fn spawn_block_container(world: &mut World, name: &str) -> ComponentId {
    let root = world.add_component_boxed_named(name, Box::new(TransformComponent::new()));
    let style = world.add_component_boxed_named(
        format!("{name}_style"),
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::Block);
            style.width = SizeDimension::Percent(100.0);
            style.overflow = Overflow::Visible;
            style
        }),
    );
    let _ = world.add_child(root, style);
    root
}