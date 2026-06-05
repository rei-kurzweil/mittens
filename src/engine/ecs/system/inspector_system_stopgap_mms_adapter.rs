use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::engine::ecs::component::TransformComponent;
use crate::engine::ecs::component::{
    ColorComponent, Display, EdgeInsets, LayoutComponent, OptionComponent, Overflow,
    RaycastableComponent, SelectionComponent, SelectionEntry, SelectionMode, SerializeComponent,
    SizeDimension, StyleComponent, TextComponent,
};
use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, SignalKind, World};
use crate::meow_meow::component_registry::{
    filtered_root_ids_for_roots, filtered_roots_to_ce_ast, filtered_world_root_ids, spawn_tree,
};
use crate::meow_meow::object::{CeChild, MaterializedCE, Value};
use crate::meow_meow::runner::MeowMeowRunner;

const PANEL_LAYOUT_MOUNT_NAME: &str = "editor_panel_layout_mount";
const PANEL_LAYOUT_ROOT_NAME: &str = "editor_panel_layout_root";
const PANEL_LAYOUT_SELECTION_NAME: &str = "editor_panel_layout_selection";
const EDITOR_RUNTIME_UI_ROOT_NAME: &str = "editor_runtime_ui_root";
const WORLD_PANEL_ROOT_SELECTOR: &str = "#world_panel_root";
const PAINT_PANEL_ROOT_SELECTOR: &str = "#paint_panel_root";
const WORLD_PANEL_CONTENT_ROOT_SELECTOR: &str = "#world_panel_content_root";
const WORLD_PANEL_SELECTION_NAME: &str = "world_panel_selection";
const INSPECTOR_PANEL_SELECTION_NAME: &str = "inspector_panel_selection";
const PANEL_CONTENT_SLOT_SELECTOR: &str = "#content_slot";
const INSPECTOR_PANEL_ROOT_SELECTOR: &str = "#inspector_panel_root";
const INSPECTOR_PANEL_CONTENT_ROOT_SELECTOR: &str = "#inspector_panel_content_root";
const INSPECTOR_PANEL_SELECTION_SELECTOR: &str = "#inspector_panel_selection";
const PANEL_STATUS_ROOT_SELECTOR: &str = "#panel_status_root";
const PANEL_STATUS_WRAP_SELECTOR: &str = "#save_status_wrap";
const PANEL_STATUS_VALUE_SELECTOR: &str = "#panel_status_value";
const PAINT_STATUS_WRAP_SELECTOR: &str = "#paint_status_wrap";
const PAINT_TOOL_SELECTION_SELECTOR: &str = "#paint_tool_selection";
const PANEL_PATH_INPUT_SELECTOR: &str = "#path_input";
const SAVE_BUTTON_SELECTOR: &str = "#save_button";
const LOAD_BUTTON_SELECTOR: &str = "#load_button";
const ITEM_PREFIX: &str = "item_";
const INSPECTOR_ITEM_PREFIX: &str = "inspector_item_";
const PANEL_LAYOUT_TEXT_SCALE: f64 = 0.08;
const WORLD_PANEL_WIDTH_GU: f64 = 29.5;
const WORLD_PANEL_TOTAL_HEIGHT_GU: f64 = 60.5;
const INSPECTOR_PANEL_WIDTH_GU: f64 = 22.0;
const INSPECTOR_PANEL_TOTAL_HEIGHT_GU: f64 = 60.5;
const ASSET_PANEL_WIDTH_GU: f64 = 30.0;
const ASSET_PANEL_TOTAL_HEIGHT_GU: f64 = 60.5;
const PAINT_PANEL_WIDTH_GU: f64 = 41.0;
const PAINT_PANEL_TOTAL_HEIGHT_GU: f64 = 32.0;
const PANEL_LAYOUT_GAP_GU: f64 = 2.0;
const PANEL_ROOT_MARGIN_X_GU: f64 = 0.5;
const PANEL_ROOT_MARGIN_Y_GU: f64 = 0.5;
const PANEL_LAYOUT_WIDTH_BUDGET_MULTIPLIER: f64 = 10.0;

#[cfg(test)]
static WORLD_PANEL_SCENE_PATH_OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);
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
    working_file_path: Arc<Mutex<PathBuf>>,
}

impl Default for InspectorSystemStopgapMmsAdapter {
    fn default() -> Self {
        Self {
            reconciler: InspectorSystemStopgapMmsReconciler,
            panel_handler_installed: false,
            panel_layout_spawned: false,
            selected_component: Arc::new(Mutex::new(None)),
            runtime_ui_root: Arc::new(Mutex::new(None)),
            working_file_path: Arc::new(Mutex::new(world_panel_scene_path())),
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
        render_assets: &crate::engine::graphics::RenderAssets,
        emit: &mut dyn SignalEmitter,
        editor_root: ComponentId,
        world_panel_pos: (f32, f32, f32),
        inspector_panel_pos: (f32, f32, f32),
        asset_system: &crate::engine::ecs::system::AssetSystem,
    ) {
        let runtime_ui_root = self.get_or_create_runtime_ui_root(world);

        println!(
            "[InspectorSystem][debug] setup_panels_for_editor editor_root={editor_root:?} runtime_ui_root={runtime_ui_root:?} world_panel_pos={:?} inspector_panel_pos={:?}",
            world_panel_pos, inspector_panel_pos,
        );

        let model = build_world_panel_model(world, None);
        let inspector_model = build_inspector_panel_model(world, None);

        {
            let working_file_path = self
                .working_file_path
                .lock()
                .expect("working file path mutex poisoned");
            self.reconciler.reconcile_panel_layout(
                world,
                render_assets,
                emit,
                &mut self.panel_layout_spawned,
                runtime_ui_root,
                world_panel_pos,
                inspector_panel_pos,
                &model,
                &inspector_model,
                &working_file_path,
                asset_system,
            );
        }

        self.refresh_world_panels(world, emit);

        self.install_shared_panel_handlers(rx, runtime_ui_root);
    }

    fn install_shared_panel_handlers(&mut self, rx: &mut RxWorld, panel_query_root: ComponentId) {
        if self.panel_handler_installed {
            return;
        }
        self.panel_handler_installed = true;

        let selected_component = Arc::clone(&self.selected_component);
        let working_file_path_mutex = Arc::clone(&self.working_file_path);

        let input_changed_path_mutex = Arc::clone(&working_file_path_mutex);
        rx.add_handler_closure(
            SignalKind::TextInputChanged,
            panel_query_root,
            move |world, _emit, signal| {
                let Some(EventSignal::TextInputChanged {
                    component_id, text, ..
                }) = signal.event.as_ref()
                else {
                    return;
                };

                if let Some(target) =
                    world.find_component(panel_query_root, PANEL_PATH_INPUT_SELECTOR)
                {
                    if target == *component_id {
                        let mut path = input_changed_path_mutex
                            .lock()
                            .expect("working file path mutex poisoned");
                        *path = PathBuf::from(text);
                    }
                }
            },
        );

        let click_path_mutex = Arc::clone(&working_file_path_mutex);
        rx.add_handler_closure(SignalKind::Click, panel_query_root, move |world, emit, signal| {
            let Some(EventSignal::Click { renderable, .. }) = signal.event.as_ref() else {
                return;
            };

            focus_panel_from_descendant_click(world, emit, panel_query_root, *renderable);

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

            let working_file_path = click_path_mutex
                .lock()
                .expect("working file path mutex poisoned");

            if let Some(status_text) = handle_panel_button_click(world, emit, *renderable, &working_file_path) {
                if panel_status_text(world, world_panel_root).as_deref() != Some(status_text.as_str()) {
                    rerender_world_panel_status(world, emit, world_panel_root, status_wrap, &status_text);
                }

                let mut selected = selected_component.lock().expect("selected component mutex poisoned");
                if selected.is_some_and(|component_id| world.get_component_record(component_id).is_none()) {
                    *selected = None;
                }

                let world_model = build_world_panel_model(world, *selected);
                rerender_world_panel_content(
                    world,
                    emit,
                    panel_query_root,
                    content_slot,
                    &world_model.rows,
                    world_model.selected_index,
                );

                if let Some(inspector_panel_root) = world.find_component(panel_query_root, INSPECTOR_PANEL_ROOT_SELECTOR) {
                    if let Some(inspector_content_slot) = world.find_component(inspector_panel_root, PANEL_CONTENT_SLOT_SELECTOR) {
                        let inspector_model = build_inspector_panel_model(world, *selected);
                        rerender_inspector_panel_content(
                            world,
                            emit,
                            panel_query_root,
                            inspector_content_slot,
                            &inspector_model.rows,
                        );
                    }
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

            {
                let mut selected = selected_component.lock().expect("selected component mutex poisoned");
                *selected = Some(target_component);
            }

            if let Some(target_label) = world.component_label(target_component) {
                let status_text = format!("selected {target_label}");
                rerender_world_panel_status(world, emit, world_panel_root, status_wrap, &status_text);
            }

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

        rx.add_handler_closure(
            SignalKind::SelectionChanged,
            panel_query_root,
            move |world, emit, signal| {
                let Some(EventSignal::SelectionChanged { selection_root, .. }) =
                    signal.event.as_ref()
                else {
                    return;
                };

                let Some(expected_selection_root) =
                    world.find_component(panel_query_root, PAINT_TOOL_SELECTION_SELECTOR)
                else {
                    return;
                };
                if *selection_root != expected_selection_root {
                    return;
                }

                let _ = emit;
            },
        );

        rx.add_handler_closure(
            SignalKind::SelectionChanged,
            panel_query_root,
            move |world, emit, signal| {
                let Some(EventSignal::SelectionChanged { selection_root, .. }) =
                    signal.event.as_ref()
                else {
                    return;
                };

                let Some(expected_selection_root) = world
                    .find_component(panel_query_root, &format!("#{WORLD_PANEL_SELECTION_NAME}"))
                else {
                    return;
                };
                if *selection_root != expected_selection_root {
                    return;
                }

                let Some(panel_layout_selection) = world
                    .find_component(panel_query_root, &format!("#{PANEL_LAYOUT_SELECTION_NAME}"))
                else {
                    return;
                };
                let Some(world_panel_root) =
                    world.find_component(panel_query_root, WORLD_PANEL_ROOT_SELECTOR)
                else {
                    return;
                };

                emit.push_intent_now(
                    panel_layout_selection,
                    IntentValue::SelectionSet {
                        component_ids: vec![panel_layout_selection],
                        entries: vec![SelectionEntry {
                            index: None,
                            item: Some(
                                WORLD_PANEL_ROOT_SELECTOR
                                    .trim_start_matches('#')
                                    .to_string(),
                            ),
                            component: world_panel_root,
                        }],
                        primary: Some(world_panel_root),
                    },
                );
            },
        );

        rx.add_handler_closure(
            SignalKind::SelectionChanged,
            panel_query_root,
            move |world, emit, signal| {
                let Some(EventSignal::SelectionChanged { selection_root, .. }) =
                    signal.event.as_ref()
                else {
                    return;
                };

                let Some(expected_selection_root) =
                    world.find_component(panel_query_root, INSPECTOR_PANEL_SELECTION_SELECTOR)
                else {
                    return;
                };
                if *selection_root != expected_selection_root {
                    return;
                }

                let Some(panel_layout_selection) = world
                    .find_component(panel_query_root, &format!("#{PANEL_LAYOUT_SELECTION_NAME}"))
                else {
                    return;
                };
                let Some(inspector_panel_root) =
                    world.find_component(panel_query_root, INSPECTOR_PANEL_ROOT_SELECTOR)
                else {
                    return;
                };

                emit.push_intent_now(
                    panel_layout_selection,
                    IntentValue::SelectionSet {
                        component_ids: vec![panel_layout_selection],
                        entries: vec![SelectionEntry {
                            index: None,
                            item: Some(
                                INSPECTOR_PANEL_ROOT_SELECTOR
                                    .trim_start_matches('#')
                                    .to_string(),
                            ),
                            component: inspector_panel_root,
                        }],
                        primary: Some(inspector_panel_root),
                    },
                );
            },
        );

        rx.add_handler_closure(
            SignalKind::SelectionChanged,
            panel_query_root,
            move |world, emit, signal| {
                let Some(EventSignal::SelectionChanged { selection_root, .. }) =
                    signal.event.as_ref()
                else {
                    return;
                };

                let Some(expected_selection_root) =
                    world.find_component(panel_query_root, PAINT_TOOL_SELECTION_SELECTOR)
                else {
                    return;
                };
                if *selection_root != expected_selection_root {
                    return;
                }

                let Some(panel_layout_selection) = world
                    .find_component(panel_query_root, &format!("#{PANEL_LAYOUT_SELECTION_NAME}"))
                else {
                    return;
                };
                let Some(paint_panel_root) =
                    world.find_component(panel_query_root, PAINT_PANEL_ROOT_SELECTOR)
                else {
                    return;
                };

                emit.push_intent_now(
                    panel_layout_selection,
                    IntentValue::SelectionSet {
                        component_ids: vec![panel_layout_selection],
                        entries: vec![SelectionEntry {
                            index: None,
                            item: Some(
                                PAINT_PANEL_ROOT_SELECTOR
                                    .trim_start_matches('#')
                                    .to_string(),
                            ),
                            component: paint_panel_root,
                        }],
                        primary: Some(paint_panel_root),
                    },
                );
            },
        );

        rx.add_handler_closure(
            SignalKind::SelectionChanged,
            panel_query_root,
            move |world, emit, signal| {
                let Some(EventSignal::SelectionChanged { selection_root, .. }) =
                    signal.event.as_ref()
                else {
                    return;
                };

                let Some(expected_selection_root) =
                    world.find_component(panel_query_root, "#assets_selection")
                else {
                    return;
                };
                if *selection_root != expected_selection_root {
                    return;
                }

                let Some(panel_layout_selection) = world
                    .find_component(panel_query_root, &format!("#{PANEL_LAYOUT_SELECTION_NAME}"))
                else {
                    return;
                };
                let Some(asset_panel_root) = world.find_component(panel_query_root, "#assets_root")
                else {
                    return;
                };

                emit.push_intent_now(
                    panel_layout_selection,
                    IntentValue::SelectionSet {
                        component_ids: vec![panel_layout_selection],
                        entries: vec![SelectionEntry {
                            index: None,
                            item: Some("assets_root".to_string()),
                            component: asset_panel_root,
                        }],
                        primary: Some(asset_panel_root),
                    },
                );
            },
        );
    }

    fn get_or_create_runtime_ui_root(&self, world: &mut World) -> ComponentId {
        {
            let runtime_ui_root = self
                .runtime_ui_root
                .lock()
                .expect("runtime ui root mutex poisoned");
            if let Some(root) = *runtime_ui_root {
                return root;
            }
        }

        let runtime_ui_root = world.add_component_boxed_named(
            EDITOR_RUNTIME_UI_ROOT_NAME,
            Box::new(TransformComponent::new()),
        );
        let runtime_ui_serialize = world.add_component_boxed_named(
            "editor_runtime_ui_serialize",
            Box::new(SerializeComponent::off()),
        );
        let _ = world.add_child(runtime_ui_root, runtime_ui_serialize);

        *self
            .runtime_ui_root
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

        let Some(world_panel_root) =
            world.find_component(panel_query_root, WORLD_PANEL_ROOT_SELECTOR)
        else {
            return;
        };
        let Some(content_slot) =
            world.find_component(world_panel_root, PANEL_CONTENT_SLOT_SELECTOR)
        else {
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

        let Some(inspector_panel_root) =
            world.find_component(panel_query_root, INSPECTOR_PANEL_ROOT_SELECTOR)
        else {
            return;
        };
        let Some(inspector_content_slot) =
            world.find_component(inspector_panel_root, PANEL_CONTENT_SLOT_SELECTOR)
        else {
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
        render_assets: &crate::engine::graphics::RenderAssets,
        emit: &mut dyn SignalEmitter,
        panel_layout_spawned: &mut bool,
        panel_query_root: ComponentId,
        world_panel_pos: (f32, f32, f32),
        inspector_panel_pos: (f32, f32, f32),
        model: &WorldPanelModel,
        inspector_model: &InspectorPanelModel,
        working_file_path: &Path,
        asset_system: &crate::engine::ecs::system::AssetSystem,
    ) {
        let existing_world_panel =
            self.find_world_panel_node(world, panel_query_root, WORLD_PANEL_ROOT_SELECTOR);
        let existing_inspector_panel =
            self.find_world_panel_node(world, panel_query_root, INSPECTOR_PANEL_ROOT_SELECTOR);

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
            render_assets,
            emit,
            panel_query_root,
            world_panel_pos,
            inspector_panel_pos,
            model,
            inspector_model,
            working_file_path,
            asset_system,
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
        render_assets: &crate::engine::graphics::RenderAssets,
        emit: &mut dyn SignalEmitter,
        panel_query_root: ComponentId,
        world_panel_pos: (f32, f32, f32),
        inspector_panel_pos: (f32, f32, f32),
        model: &WorldPanelModel,
        inspector_model: &InspectorPanelModel,
        working_file_path: &Path,
        asset_system: &crate::engine::ecs::system::AssetSystem,
    ) {
        println!(
            "[InspectorSystem][debug] spawn_panel_layout panel_query_root={panel_query_root:?} world_panel_pos={:?} inspector_panel_pos={:?}",
            world_panel_pos, inspector_panel_pos,
        );

        let world_panel_title_color = Value::Array(vec![
            Value::Number(0.90),
            Value::Number(1.00),
            Value::Number(0.92),
            Value::Number(1.0),
        ]);
        let world_panel_bg = Value::Array(vec![
            Value::Number(0.18),
            Value::Number(0.78),
            Value::Number(0.22),
            Value::Number(0.95),
        ]);
        let world_panel_item_bg = Value::Array(vec![
            Value::Number(0.92),
            Value::Number(0.97),
            Value::Number(0.92),
            Value::Number(1.0),
        ]);

        let inspector_panel_title_color = Value::Array(vec![
            Value::Number(0.90),
            Value::Number(1.00),
            Value::Number(0.92),
            Value::Number(1.0),
        ]);
        let inspector_panel_bg = Value::Array(vec![
            Value::Number(0.18),
            Value::Number(0.78),
            Value::Number(0.22),
            Value::Number(0.95),
        ]);
        let inspector_panel_item_bg = Value::Array(vec![
            Value::Number(0.92),
            Value::Number(0.92),
            Value::Number(0.92),
            Value::Number(0.80),
        ]);

        let asset_panel_title_color = world_panel_title_color.clone();
        let asset_panel_bg = world_panel_bg.clone();
        let asset_panel_item_bg = world_panel_item_bg.clone();

        let paint_panel_title_color = world_panel_title_color.clone();
        let paint_panel_bg = world_panel_bg.clone();
        let paint_panel_item_bg = world_panel_item_bg.clone();

        let working_file_path_str = working_file_path.to_string_lossy().to_string();

        let world_panel = match build_panel_component_expr(
            world,
            emit,
            world_panel_asset_path(),
            "world_panel",
            vec![
                Value::String(model.title.clone()),
                Value::Array(Vec::new()),
                world_panel_title_color.clone(),
                world_panel_bg.clone(),
                world_panel_item_bg.clone(),
                Value::String(working_file_path_str),
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
                inspector_panel_title_color.clone(),
                inspector_panel_bg.clone(),
                inspector_panel_item_bg.clone(),
            ],
            "inspector panel",
        ) {
            Some(panel) => panel,
            None => return,
        };

        let asset_items_val = Value::Array(Vec::new());

        let asset_panel = match build_panel_component_expr(
            world,
            emit,
            asset_panel_asset_path(),
            "asset_panel",
            vec![
                Value::String("Assets".to_string()),
                asset_items_val,
                asset_panel_title_color.clone(),
                asset_panel_bg.clone(),
                asset_panel_item_bg.clone(),
            ],
            "asset panel",
        ) {
            Some(panel) => panel,
            None => return,
        };

        let paint_panel = match build_panel_component_expr(
            world,
            emit,
            paint_panel_asset_path(),
            "paint_panel",
            vec![
                Value::String("Paint".to_string()),
                paint_panel_title_color.clone(),
                paint_panel_bg.clone(),
                paint_panel_item_bg.clone(),
            ],
            "paint panel",
        ) {
            Some(panel) => panel,
            None => return,
        };

        let _ = inspector_panel_pos;
        let anchor_pos = world_panel_pos;

        let panel_strip_width_gu = WORLD_PANEL_WIDTH_GU
            + PANEL_LAYOUT_GAP_GU
            + INSPECTOR_PANEL_WIDTH_GU
            + PANEL_LAYOUT_GAP_GU
            + ASSET_PANEL_WIDTH_GU
            + PANEL_LAYOUT_GAP_GU
            + PAINT_PANEL_WIDTH_GU
            + (PANEL_ROOT_MARGIN_X_GU * 2.0 * 4.0);
        let total_width_gu = panel_strip_width_gu * PANEL_LAYOUT_WIDTH_BUDGET_MULTIPLIER;
        let total_height_gu = WORLD_PANEL_TOTAL_HEIGHT_GU
            .max(INSPECTOR_PANEL_TOTAL_HEIGHT_GU)
            .max(ASSET_PANEL_TOTAL_HEIGHT_GU)
            .max(PAINT_PANEL_TOTAL_HEIGHT_GU)
            + (PANEL_ROOT_MARGIN_Y_GU * 2.0);

        let world_panel = decorate_panel_root_ce(world_panel, 0.0);
        let inspector_panel = decorate_panel_root_ce(inspector_panel, PANEL_LAYOUT_GAP_GU);
        let asset_panel = decorate_panel_root_ce(asset_panel, PANEL_LAYOUT_GAP_GU);
        let paint_panel = decorate_panel_root_ce(paint_panel, PANEL_LAYOUT_GAP_GU);

        let shared_layout_root = MaterializedCE {
            component_type: "LayoutRoot".to_string(),
            ctor_method: None,
            ctor_args: Vec::new(),
            calls: vec![
                (
                    "available_width".to_string(),
                    vec![Value::Number(total_width_gu)],
                ),
                (
                    "available_height".to_string(),
                    vec![Value::Number(total_height_gu)],
                ),
                (
                    "unit_scale".to_string(),
                    vec![Value::Number(PANEL_LAYOUT_TEXT_SCALE)],
                ),
            ],
            named: vec![(
                "name".to_string(),
                Value::String(PANEL_LAYOUT_ROOT_NAME.to_string()),
            )],
            positionals: Vec::new(),
            children: vec![
                CeChild::Spawn(world_panel),
                CeChild::Spawn(inspector_panel),
                CeChild::Spawn(asset_panel),
                CeChild::Spawn(paint_panel),
            ],
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

        let panel_mount_root = match spawn_tree(&mount_ce, None, world, emit) {
            Ok(component_id) => component_id,
            Err(error) => {
                eprintln!("[InspectorSystemStopgapMmsAdapter] panel layout spawn error: {error}");
                return;
            }
        };

        // Add SelectionComponent to the LayoutRoot so we can select individual panels.
        if let Some(layout_root_id) =
            world.find_component(panel_mount_root, &format!("#{PANEL_LAYOUT_ROOT_NAME}"))
        {
            use crate::engine::ecs::component::SelectionComponent;
            let selection = world.add_component_boxed_named(
                PANEL_LAYOUT_SELECTION_NAME,
                Box::new(SelectionComponent::new()),
            );
            emit.push_intent_now(
                layout_root_id,
                IntentValue::Attach {
                    parents: vec![layout_root_id],
                    child: selection,
                },
            );
            world.init_component_tree(selection, emit);
        }

        if let Some(_world_panel_root) =
            world.find_component(panel_mount_root, WORLD_PANEL_ROOT_SELECTOR)
        {}
        if let Some(_inspector_panel_root) =
            world.find_component(panel_mount_root, INSPECTOR_PANEL_ROOT_SELECTOR)
        {}
        if let Some(paint_tool_selection) =
            world.find_component(panel_mount_root, "#paint_tool_selection")
        {
            if let Some(selection) =
                world.get_component_by_id_as_mut::<SelectionComponent>(paint_tool_selection)
            {
                selection.mode = SelectionMode::Single;
                selection.clear();
            }
            if let Some(free_draw_item) = world
                .find_all_components(panel_mount_root, "[name='paint_panel_item']")
                .into_iter()
                .next()
            {
                emit.push_intent_now(
                    paint_tool_selection,
                    IntentValue::SelectionSet {
                        component_ids: vec![paint_tool_selection],
                        entries: vec![SelectionEntry {
                            index: Some(0),
                            item: Some("Free Draw".to_string()),
                            component: free_draw_item,
                        }],
                        primary: Some(free_draw_item),
                    },
                );
            }
        }
        if let Some(inspector_panel_selection) =
            world.find_component(panel_mount_root, INSPECTOR_PANEL_SELECTION_SELECTOR)
        {
            if let Some(selection) =
                world.get_component_by_id_as_mut::<SelectionComponent>(inspector_panel_selection)
            {
                selection.mode = SelectionMode::Single;
                selection.clear();
            }
        }
        if let Some(asset_panel_root) = world.find_component(panel_mount_root, "#assets_root") {
            if let Some(_content_slot) =
                world.find_component(asset_panel_root, PANEL_CONTENT_SLOT_SELECTOR)
            {
                if let Some(selection_root) =
                    world.find_component(asset_panel_root, "#assets_content_area")
                {
                    let items_already_there = world.children_of(selection_root).len();
                    if items_already_there <= 2 {
                        // Only style/Selection markers are there
                        println!(
                            "[InspectorSystem][debug] populating asset panel with {} items into selection_root={:?}",
                            asset_system.items.len(),
                            selection_root
                        );

                        let mut last_module_id = None;
                        for (index, item) in asset_system.items.iter().enumerate() {
                            if last_module_id != Some(item.module_id) {
                                last_module_id = Some(item.module_id);
                                if let Some(module_name) =
                                    asset_system.get_module_name(item.module_id)
                                {
                                    match asset_system.build_asset_module_header(
                                        world,
                                        emit,
                                        &module_name,
                                    ) {
                                        Ok(header_root) => {
                                            world.init_component_tree(header_root, emit);
                                            emit.push_intent_now(
                                                header_root,
                                                IntentValue::Attach {
                                                    parents: vec![selection_root],
                                                    child: header_root,
                                                },
                                            );
                                        }
                                        Err(e) => {
                                            eprintln!(
                                                "[InspectorSystem][error] failed to build asset header for {}: {}",
                                                module_name, e
                                            );
                                        }
                                    }
                                }
                            }

                            match asset_system.build_asset_item_shell(
                                world,
                                render_assets,
                                emit,
                                item,
                                index,
                            ) {
                                Ok(item_root) => {
                                    println!(
                                        "[InspectorSystem][debug] attaching asset item title={:?} export={:?} root={:?} to selection_root={:?}",
                                        item.title, item.export_name, item_root, selection_root
                                    );
                                    world.init_component_tree(item_root, emit);
                                    emit.push_intent_now(
                                        item_root,
                                        IntentValue::Attach {
                                            parents: vec![selection_root],
                                            child: item_root,
                                        },
                                    );
                                }
                                Err(e) => {
                                    eprintln!(
                                        "[InspectorSystem][error] failed to build asset item {}: {}",
                                        item.export_name, e
                                    );
                                }
                            }
                        }
                        mark_nearest_layout_dirty(world, selection_root);
                    }
                }
            }
        }
        if let Some(_paint_panel_root) = world.find_component(panel_mount_root, "#paint_panel_root")
        {
        }

        if let Some(panel_layout_selection) =
            world.find_component(panel_mount_root, &format!("#{PANEL_LAYOUT_SELECTION_NAME}"))
        {
            if let Some(world_panel_root) =
                world.find_component(panel_mount_root, WORLD_PANEL_ROOT_SELECTOR)
            {
                emit.push_intent_now(
                    panel_layout_selection,
                    IntentValue::SelectionSet {
                        component_ids: vec![panel_layout_selection],
                        entries: vec![SelectionEntry {
                            index: Some(0),
                            item: Some("world_panel_root".to_string()),
                            component: world_panel_root,
                        }],
                        primary: Some(world_panel_root),
                    },
                );
            }
        }

        println!(
            "[InspectorSystem][debug] spawned panel mount root={panel_mount_root:?} name={} anchor_pos={:?}",
            PANEL_LAYOUT_MOUNT_NAME, anchor_pos,
        );

        emit.push_intent_now(
            panel_mount_root,
            IntentValue::Attach {
                parents: vec![panel_query_root],
                child: panel_mount_root,
            },
        );

        if let Some(world_panel_root) =
            world.find_component(panel_mount_root, WORLD_PANEL_ROOT_SELECTOR)
        {
            if let Some(content_slot) =
                world.find_component(world_panel_root, PANEL_CONTENT_SLOT_SELECTOR)
            {
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
        emit.push_intent_now(
            panel_mount_root,
            IntentValue::Attach {
                parents: vec![panel_query_root],
                child: panel_mount_root,
            },
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

fn handle_panel_button_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    renderable: ComponentId,
    working_file_path: &Path,
) -> Option<String> {
    let runtime_ui_root = find_named_root(world, EDITOR_RUNTIME_UI_ROOT_NAME)?;

    let save_button = world.find_component(runtime_ui_root, SAVE_BUTTON_SELECTOR);
    if save_button.is_some_and(|button| is_descendant_or_self(world, button, renderable)) {
        return Some(
            match save_world_panel_scene_to_path(world, working_file_path) {
                Ok(saved_roots) => format!(
                    "saved {saved_roots} roots to {}",
                    working_file_path.display()
                ),
                Err(error) => format!("save failed: {error}"),
            },
        );
    }

    let load_button = world.find_component(runtime_ui_root, LOAD_BUTTON_SELECTOR);
    if load_button.is_some_and(|button| is_descendant_or_self(world, button, renderable)) {
        return Some(
            match load_world_panel_scene_from_path(world, emit, working_file_path) {
                Ok(loaded_roots) => format!(
                    "loaded {loaded_roots} roots from {}",
                    working_file_path.display()
                ),
                Err(error) => format!("load failed: {error}"),
            },
        );
    }

    None
}

fn find_named_root(world: &World, name: &str) -> Option<ComponentId> {
    world.all_components().find(|&component_id| {
        world.parent_of(component_id).is_none()
            && world
                .component_label(component_id)
                .is_some_and(|label| label == name)
    })
}

fn world_panel_scene_path() -> PathBuf {
    #[cfg(test)]
    {
        if let Some(path) = WORLD_PANEL_SCENE_PATH_OVERRIDE
            .lock()
            .expect("world panel scene path override mutex poisoned")
            .clone()
        {
            return path;
        }
    }

    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/data/world.mms"
    ))
}

#[cfg(test)]
pub(crate) fn set_world_panel_scene_path_for_tests(path: Option<PathBuf>) {
    *WORLD_PANEL_SCENE_PATH_OVERRIDE
        .lock()
        .expect("world panel scene path override mutex poisoned") = path;
}

fn save_world_panel_scene_to_path(world: &World, path: &Path) -> Result<usize, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }

    let world_roots: Vec<ComponentId> = world
        .all_components()
        .filter(|&cid| world.parent_of(cid).is_none())
        .collect();
    let serializable_roots = filtered_root_ids_for_roots(world, &world_roots);
    if serializable_roots.is_empty() {
        return Err("no serializable roots found".to_string());
    }

    let mut out = String::new();
    let total = serializable_roots.len();
    println!(
        "[WorldPanel] serializing {} root components to {}",
        total,
        path.display()
    );
    for (index, &root_id) in serializable_roots.iter().enumerate() {
        println!(
            "[WorldPanel] serializing root components [{}/{}]: {}",
            index + 1,
            total,
            world_panel_root_label(world, root_id)
        );
        let mut components = filtered_roots_to_ce_ast(world, &[root_id])?;
        let component = components
            .pop()
            .ok_or_else(|| format!("missing serialized root for {:?}", root_id))?;
        out.push_str(&crate::meow_meow::unparser::unparse_component(&component));
        out.push_str("\n\n");
    }
    println!(
        "[WorldPanel] finished serializing {} root components",
        total
    );

    std::fs::write(path, out)
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;

    Ok(total)
}

fn world_panel_root_label(world: &World, component_id: ComponentId) -> String {
    if let Some(label) = world.component_label(component_id) {
        if !label.is_empty() {
            return format!("#{label}");
        }
    }

    if let Some(name) = world.component_name(component_id) {
        return format!("{}({:?})", name, component_id);
    }

    format!("{:?}", component_id)
}

fn load_world_panel_scene_from_path(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    path: &Path,
) -> Result<usize, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;

    let removable_roots = filtered_world_root_ids(world);
    for root in removable_roots {
        world
            .remove_component_subtree(root)
            .map_err(|error| format!("remove_component_subtree failed: {error}"))?;
    }

    let Some(path_str) = path.to_str() else {
        return Err(format!("non-utf8 path: {}", path.display()));
    };
    let module = MeowMeowRunner::load_module_source(&source, Some(path_str))?;
    let mut loaded_roots = 0;
    for component in &module.sequence {
        spawn_tree(component, None, world, emit)?;
        loaded_roots += 1;
    }
    Ok(loaded_roots)
}

fn build_world_panel_model(
    world: &World,
    selected_component: Option<ComponentId>,
) -> WorldPanelModel {
    let rows = build_world_panel_rows(world);
    let selected_index = selected_component.and_then(|selected| {
        rows.iter()
            .position(|row| row.target_component == Some(selected))
            .map(|index| index as i64)
    });

    WorldPanelModel {
        title: "World".to_string(),
        rows,
        selected_index,
    }
}

fn build_inspector_panel_model(
    world: &World,
    selected_component: Option<ComponentId>,
) -> InspectorPanelModel {
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
        display_label: format!(
            "{}{}",
            "  ".repeat(depth),
            world_panel_item_label(world, component_id)
        ),
        kind: InspectorPanelRowKind::Component,
    });

    for &child in world.children_of(component_id) {
        push_inspector_panel_rows(world, child, depth + 1, out);
    }
}

fn build_world_panel_rows(world: &World) -> Vec<WorldPanelRow> {
    let mut editor_roots: Vec<ComponentId> = world
        .all_components()
        .filter(|&component_id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::EditorComponent>(
                    component_id,
                )
                .is_some()
        })
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

fn panel_status_text(world: &World, panel_root: ComponentId) -> Option<String> {
    world
        .find_component(panel_root, PANEL_STATUS_VALUE_SELECTOR)
        .and_then(|status_id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(status_id)
                .map(|text| text.text.clone())
        })
}

fn rerender_world_panel_status(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    world_panel_root: ComponentId,
    status_wrap: ComponentId,
    label: &str,
) {
    rerender_panel_status(world, emit, world_panel_root, status_wrap, label);
}

fn rerender_panel_status(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_root: ComponentId,
    status_wrap: ComponentId,
    label: &str,
) {
    if let Some(existing_status_root) = world.find_component(panel_root, PANEL_STATUS_ROOT_SELECTOR)
    {
        emit.push_intent_now(
            existing_status_root,
            IntentValue::RemoveSubtree {
                component_ids: vec![existing_status_root],
            },
        );
    }

    let spawned_status_root =
        match MeowMeowRunner::spawn_mms_module_component_uninitialized_from_file(
            world_panel_status_asset_path(),
            "world_panel_status",
            vec![Value::String(label.to_string())],
            world,
            emit,
        ) {
            Ok(component_id) => component_id,
            Err(error) => {
                eprintln!("[InspectorSystemStopgapMmsAdapter] panel status spawn error: {error}");
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
    mark_nearest_layout_dirty(world, status_wrap);
}

fn rerender_world_panel_content(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    content_slot: ComponentId,
    rows: &[WorldPanelRow],
    selected_index: Option<i64>,
) {
    let Some(world_panel_root) = world.find_component(panel_query_root, WORLD_PANEL_ROOT_SELECTOR)
    else {
        return;
    };

    if let Some(existing_content_root) =
        world.find_component(world_panel_root, WORLD_PANEL_CONTENT_ROOT_SELECTOR)
    {
        emit.push_intent_now(
            existing_content_root,
            IntentValue::RemoveSubtree {
                component_ids: vec![existing_content_root],
            },
        );
    }

    let spawned_content_root = spawn_world_panel_content_tree(world, emit, rows, selected_index);
    emit.push_intent_now(
        spawned_content_root,
        IntentValue::Attach {
            parents: vec![content_slot],
            child: spawned_content_root,
        },
    );
    mark_nearest_layout_dirty(world, content_slot);
}

fn rerender_inspector_panel_content(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    content_slot: ComponentId,
    rows: &[InspectorPanelRow],
) {
    let Some(inspector_panel_root) =
        world.find_component(panel_query_root, INSPECTOR_PANEL_ROOT_SELECTOR)
    else {
        return;
    };

    if let Some(existing_content_root) =
        world.find_component(inspector_panel_root, INSPECTOR_PANEL_CONTENT_ROOT_SELECTOR)
    {
        emit.push_intent_now(
            existing_content_root,
            IntentValue::RemoveSubtree {
                component_ids: vec![existing_content_root],
            },
        );
    }

    let spawned_content_root = spawn_inspector_panel_content_tree(world, emit, rows);
    emit.push_intent_now(
        spawned_content_root,
        IntentValue::Attach {
            parents: vec![content_slot],
            child: spawned_content_root,
        },
    );
    mark_nearest_layout_dirty(world, content_slot);
}

fn mark_nearest_layout_dirty(world: &mut World, start: ComponentId) {
    let mut current = Some(start);
    while let Some(component_id) = current {
        if let Some(layout) = world.get_component_by_id_as_mut::<LayoutComponent>(component_id) {
            layout.mark_dirty();
            return;
        }
        current = world.parent_of(component_id);
    }
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

fn focus_panel_from_descendant_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    renderable: ComponentId,
) {
    let Some(panel_layout_selection) =
        world.find_component(panel_query_root, &format!("#{PANEL_LAYOUT_SELECTION_NAME}"))
    else {
        return;
    };

    for (selector, item_name) in [
        (WORLD_PANEL_ROOT_SELECTOR, "world_panel_root"),
        (INSPECTOR_PANEL_ROOT_SELECTOR, "inspector_panel_root"),
        ("#assets_root", "assets_root"),
        (PAINT_PANEL_ROOT_SELECTOR, "paint_panel_root"),
    ] {
        let Some(panel_root) = world.find_component(panel_query_root, selector) else {
            continue;
        };
        if !is_descendant_or_self(world, panel_root, renderable) {
            continue;
        }
        emit.push_intent_now(
            panel_layout_selection,
            IntentValue::SelectionSet {
                component_ids: vec![panel_layout_selection],
                entries: vec![SelectionEntry {
                    index: None,
                    item: Some(item_name.to_string()),
                    component: panel_root,
                }],
                primary: Some(panel_root),
            },
        );
        return;
    }
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
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/panels.mms")
}

fn world_panel_status_asset_path() -> &'static str {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/components/panel_items.mms"
    )
}

fn asset_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/panels.mms")
}

fn paint_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/panels.mms")
}

fn inspector_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/panels.mms")
}

fn build_placeholder_panel_component_expr(title_name: &'static str, title: &str) -> MaterializedCE {
    MaterializedCE {
        component_type: "T".to_string(),
        ctor_method: None,
        ctor_args: Vec::new(),
        calls: Vec::new(),
        named: vec![("name".to_string(), Value::String(title_name.to_string()))],
        positionals: Vec::new(),
        children: vec![CeChild::Spawn(MaterializedCE {
            component_type: "T".to_string(),
            ctor_method: None,
            ctor_args: Vec::new(),
            calls: Vec::new(),
            named: vec![(
                "name".to_string(),
                Value::String(format!("{title_name}_title")),
            )],
            positionals: Vec::new(),
            children: vec![CeChild::Spawn(MaterializedCE {
                component_type: "Text".to_string(),
                ctor_method: None,
                ctor_args: Vec::new(),
                calls: Vec::new(),
                named: vec![(
                    "name".to_string(),
                    Value::String(format!("{title_name}_label")),
                )],
                positionals: vec![Value::String(title.to_string())],
                children: Vec::new(),
            })],
        })],
    }
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

fn decorate_panel_root_ce(mut panel_root: MaterializedCE, margin_left_gu: f64) -> MaterializedCE {
    panel_root.children.insert(
        0,
        CeChild::Spawn(MaterializedCE {
            component_type: "Option".to_string(),
            ctor_method: None,
            ctor_args: Vec::new(),
            calls: Vec::new(),
            named: Vec::new(),
            positionals: Vec::new(),
            children: Vec::new(),
        }),
    );
    panel_root.children.insert(
        1,
        CeChild::Spawn(MaterializedCE {
            component_type: "Raycastable".to_string(),
            ctor_method: Some("enabled".to_string()),
            ctor_args: Vec::new(),
            calls: Vec::new(),
            named: Vec::new(),
            positionals: Vec::new(),
            children: Vec::new(),
        }),
    );

    if let Some(CeChild::Spawn(style_ce)) = panel_root.children.iter_mut().find(|child| {
        matches!(
            child,
            CeChild::Spawn(MaterializedCE {
                component_type,
                ..
            }) if component_type == "Style"
        )
    }) {
        style_ce.calls.push((
            "display".to_string(),
            vec![Value::String("inline-block".to_string())],
        ));
        style_ce.calls.push((
            "margin_left".to_string(),
            vec![Value::Number(margin_left_gu)],
        ));
    }

    panel_root
}

fn debug_style_details(world: &World, root: ComponentId, selector: &str, label: &str) {
    let node = match world.find_component(root, selector) {
        Some(node) => node,
        None => {
            println!("[InspectorSystem][trace] {} {} missing", label, selector);
            return;
        }
    };

    let style_children: Vec<_> = world
        .children_of(node)
        .iter()
        .filter(|&&child| {
            world
                .get_component_by_id_as::<StyleComponent>(child)
                .is_some()
        })
        .copied()
        .collect();

    println!(
        "[InspectorSystem][trace] {} {} child_count={}",
        label,
        selector,
        world.children_of(node).len()
    );
    for &child in world.children_of(node) {
        println!(
            "[InspectorSystem][trace] {} {} child={:?} type={:?} label={:?}",
            label,
            selector,
            child,
            world.component_name(child),
            world.component_label(child),
        );
    }

    if style_children.is_empty() {
        println!(
            "[InspectorSystem][trace] {} {} has no Style child",
            label, selector
        );
        return;
    }

    for style_id in style_children {
        if let Some(style) = world.get_component_by_id_as::<StyleComponent>(style_id) {
            println!(
                "[InspectorSystem][trace] {} {} style={:?} display={:?} background_color={:?} height={:?} width={:?} font_size={:?}",
                label,
                selector,
                style_id,
                style.display,
                style.background_color,
                style.height,
                style.width,
                style.font_size,
            );
        }
    }
}

fn debug_panel_root(world: &World, root: ComponentId, kind: &str) {
    let title_label = world.find_component(root, "#title_label").is_some();
    let text_count = world.find_all_components(root, "Text").len();
    let style_count = world.find_all_components(root, "Style").len();
    let content_slot = world.find_component(root, "#content_slot").is_some();
    let rows_mount = world.find_component(root, "#rows_mount").is_some();
    let assets_content = world.find_component(root, "#assets_content_area").is_some();
    let paint_title_bar = world
        .find_component(root, "paint_panel_title_bar")
        .is_some();
    println!(
        "[InspectorSystem][trace] {} root={:?} title_label={} content_slot={} rows_mount={} assets_content={} paint_title_bar={} text_count={} style_count={}",
        kind,
        root,
        title_label,
        content_slot,
        rows_mount,
        assets_content,
        paint_title_bar,
        text_count,
        style_count,
    );
    debug_style_details(world, root, "#content_slot", kind);
    debug_style_details(world, root, "title_bar", kind);
    debug_style_details(world, root, "#title_label", kind);
    debug_style_details(world, root, "#assets_content_area", kind);
    debug_style_details(world, root, "paint_panel_title_bar", kind);
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
    let selection = world.add_component_boxed_named(
        WORLD_PANEL_SELECTION_NAME,
        Box::new(SelectionComponent::new()),
    );
    let _ = world.add_child(rows_mount, selection);

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
            let row_root =
                world.add_component_boxed_named(row_name, Box::new(TransformComponent::new()));
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
        WorldPanelRowKind::EditorHeader
        | WorldPanelRowKind::Info
        | WorldPanelRowKind::Component => {
            let (background_rgba, text_rgba, interactive) = match row.kind {
                WorldPanelRowKind::EditorHeader => {
                    ([0.18, 0.78, 0.22, 0.95], [0.0, 0.0, 0.0, 1.0], false)
                }
                WorldPanelRowKind::Info => ([0.85, 0.85, 0.85, 1.0], [0.0, 0.0, 0.0, 1.0], false),
                WorldPanelRowKind::Component if selected => {
                    ([1.00, 0.88, 0.20, 0.96], [0.06, 0.09, 0.08, 1.0], true)
                }
                WorldPanelRowKind::Component => {
                    ([0.92, 0.97, 0.92, 1.0], [0.06, 0.09, 0.08, 1.0], true)
                }
                WorldPanelRowKind::Spacer => unreachable!(),
            };

            let row_root =
                world.add_component_boxed_named(row_name, Box::new(TransformComponent::new()));

            if interactive {
                let option = world.add_component_boxed_named(
                    format!("{row_name}_option"),
                    Box::new(OptionComponent::new()),
                );
                let _ = world.add_child(row_root, option);
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
                    style.background_color = Some(background_rgba);
                    style.background_z = Some(0.001);
                    style.color = Some(text_rgba);
                    style.overflow = Overflow::Visible;
                    style
                }),
            );
            let _ = world.add_child(row_root, style);

            let text_root = world.add_component_boxed_named(
                format!("{row_name}_text_root"),
                Box::new(TransformComponent::new().with_position(0.0, 0.0, 0.005)),
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
    let selection = world.add_component_boxed_named(
        INSPECTOR_PANEL_SELECTION_NAME,
        Box::new(SelectionComponent::new()),
    );
    let _ = world.add_child(rows_mount, selection);

    for (index, row) in rows.iter().enumerate() {
        let row_root =
            spawn_inspector_panel_row_tree(world, &format!("{INSPECTOR_ITEM_PREFIX}{index}"), row);
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
    if matches!(row.kind, InspectorPanelRowKind::Component) {
        let option = world.add_component_boxed_named(
            format!("{row_name}_option"),
            Box::new(OptionComponent::new()),
        );
        let _ = world.add_child(row_root, option);
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
            style.background_color = Some(background_rgba);
            style.background_z = Some(0.001);
            style.color = Some(text_rgba);
            style.overflow = Overflow::Visible;
            style
        }),
    );
    let _ = world.add_child(row_root, style);

    let text_root = world.add_component_boxed_named(
        format!("{row_name}_text_root"),
        Box::new(TransformComponent::new().with_position(0.0, 0.0, 0.005)),
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
