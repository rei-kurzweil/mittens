use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::engine::ecs::component::{EditorInteractionMode, TransformComponent};
use crate::engine::ecs::component::{
    ColorComponent, DataComponent, DataValue, OpacityComponent, RenderableComponent,
    SelectableComponent, SelectionComponent, SelectionEntry, SelectionMode, SerializeComponent,
    StyleComponent,
};
use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::system::data_renderer_system::{
    DataRendererSystem, UiDetailItem, UiItem, UiItemKind,
};
use crate::engine::ecs::system::editor::context::EditorContextState;
use crate::engine::ecs::system::editor::grid_panel::{
    GRID_PANEL_ADD_BUTTON_SELECTOR, GRID_PANEL_DELETE_PAYLOAD_NAME, GRID_PANEL_ROOT_SELECTOR,
    GRID_PANEL_ROW_PAYLOAD_NAME, GRID_PANEL_ROW_SPEC, GRID_PANEL_TOGGLE_PAYLOAD_NAME,
    build_grid_panel_model, grid_panel_items,
};
use crate::engine::ecs::system::editor::inspector_panel::{
    INSPECTOR_DETAIL_SPEC, INSPECTOR_ITEM_PREFIX, INSPECTOR_PANEL_INSTANCE_ID_KEY,
    INSPECTOR_PANEL_PAYLOAD_NAME, INSPECTOR_ROW_SPEC, InspectorPanelDetailModel, InspectorPanelId,
    InspectorPanelModel, InspectorPanelRow, InspectorPanelRowKind, InspectorWorkspaceEvent,
    InspectorWorkspaceState, build_inspector_panel_models, build_inspector_panel_rows,
    clear_missing_inspector_targets, inspector_panel_instance_id_on_root,
    parse_inspector_item_index, reduce_inspector_workspace_state,
    resolve_selected_inspector_panel_payload,
};
use crate::engine::ecs::system::editor::panel_ui::{
    PanelUiRowSpec, spawn_block_container, spawn_panel_ui_row_tree,
};
use crate::engine::ecs::system::editor::settings_panel::{
    EDITOR_SETTINGS_PANEL_ROOT_SELECTOR, EDITOR_SETTINGS_SELECTION_SELECTOR,
    EditorSettingsOption,
};
use crate::engine::ecs::system::editor::workspace::EditorWorkspaceRuntime;
use crate::engine::ecs::system::editor::world_panel::{
    AuthoredWorldPanelSceneModel, ITEM_PREFIX, WORLD_PANEL_PAYLOAD_NAME,
    WORLD_PANEL_SELECTION_NAME, WorldPanelModel, WorldPanelRow, WorldPanelRowKind,
    build_world_panel_model, editor_scene_roots, mark_nearest_layout_dirty, parse_item_index,
    rebuild_world_panel_scene_model, register_editor_root, rerender_world_panel_content,
    rerender_world_panel_status, resolve_selected_world_panel_payload, sync_world_panel_selection,
    world_panel_item_label,
};
use crate::engine::ecs::system::editor_system::select_editor_target;
use crate::engine::ecs::system::{GridSystem, TransformSystem};
use crate::engine::ecs::system::panel_system::{
    EDITOR_RUNTIME_UI_ROOT_NAME, PANEL_LAYOUT_MOUNT_NAME, PANEL_LAYOUT_ROOT_NAME,
    PANEL_LAYOUT_SELECTION_NAME, PanelActionKind, PanelControlKind, PanelKind,
    PanelLayoutMountSpec, PanelShellSpec, PanelSlotKind, build_panel_shell_component_expr,
    decode_panel_action_payload, decorate_panel_root_ce, ensure_panel_layout_selection,
    find_named_root, is_descendant_or_self, panel_layout_root_id, panel_layout_selection_id,
    spawn_panel_instance, spawn_panel_layout_mount,
};
use crate::engine::ecs::system::selection_system::{
    apply_selection_set, resolve_semantic_target_from_payload,
};
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, SignalKind, World};
use crate::meow_meow::component_registry::{
    filtered_root_ids_for_roots, filtered_roots_to_ce_ast, spawn_tree,
};
use crate::meow_meow::object::{CeChild, MaterializedCE, Value};
use crate::meow_meow::runner::MeowMeowRunner;
use crate::utils::math::mat_to_quat;

const WORLD_PANEL_ROOT_SELECTOR: &str = "#world_panel_root";
const PAINT_PANEL_ROOT_SELECTOR: &str = "#paint_panel_root";
const EDITOR_SETTINGS_PANEL_WIDTH_GU: f64 = 16.0;
const EDITOR_SETTINGS_PANEL_TOTAL_HEIGHT_GU: f64 = 11.5;
const WORLD_PANEL_CONTENT_ROOT_SELECTOR: &str = "#world_panel_content_root";
const INSPECTOR_PANEL_SELECTION_NAME: &str = "inspector_panel_selection";
const PANEL_CONTENT_SLOT_SELECTOR: &str = "#content_slot";
const INSPECTOR_PANEL_ROOT_SELECTOR: &str = "#inspector_panel_root";
const INSPECTOR_PANEL_SIDEBAR_SLOT_SELECTOR: &str = "#sidebar_slot";
const INSPECTOR_PANEL_DETAIL_SLOT_SELECTOR: &str = "#detail_slot";
const INSPECTOR_PANEL_PIN_SLOT_SELECTOR: &str = "#pin_slot";
const INSPECTOR_PANEL_CONTENT_ROOT_SELECTOR: &str = "#inspector_panel_content_root";
const INSPECTOR_PANEL_DETAIL_ROOT_SELECTOR: &str = "#inspector_details_root";
const INSPECTOR_PANEL_SELECTION_SELECTOR: &str = "#inspector_panel_selection";
const INSPECTOR_PANEL_INSTANCE_PREFIX: &str = "inspector_panel_instance_";
const INSPECTOR_PANEL_INSTANCE_DATA_NAME: &str = "inspector_panel_instance_data";
const INSPECTOR_PANEL_PIN_BUTTON_NAME: &str = "pin_button";
const INSPECTOR_PANEL_PIN_BUTTON_SELECTOR: &str = "#pin_button";
const WORLD_PANEL_SELECTION_SELECTOR: &str = "#world_panel_selection";
// Removed: INSPECTOR_DETAIL_WORLD_PANEL_MOUNT_NAME, INSPECTOR_DETAIL_WORLD_LAYOUT_ROOT_NAME
const PANEL_STATUS_WRAP_SELECTOR: &str = "#save_status_wrap";
const PANEL_STATUS_VALUE_SELECTOR: &str = "#panel_status_value";
const PAINT_STATUS_WRAP_SELECTOR: &str = "#paint_status_wrap";
const PAINT_TOOL_SELECTION_SELECTOR: &str = "#paint_tool_selection";
const PANEL_PATH_INPUT_SELECTOR: &str = "#path_input";
const SAVE_BUTTON_SELECTOR: &str = "#save_button";
const LOAD_BUTTON_SELECTOR: &str = "#load_button";
const PANEL_LAYOUT_TEXT_SCALE: f64 = 0.08;
const WORLD_PANEL_WIDTH_GU: f64 = 29.5;
const WORLD_PANEL_TOTAL_HEIGHT_GU: f64 = 60.5;
const INSPECTOR_PANEL_WIDTH_GU: f64 = 44.0;
const INSPECTOR_PANEL_TOTAL_HEIGHT_GU: f64 = 60.5;
const ASSET_PANEL_WIDTH_GU: f64 = 39.0;
const ASSET_PANEL_TOTAL_HEIGHT_GU: f64 = 60.5;
const PAINT_PANEL_WIDTH_GU: f64 = 41.0;
const PAINT_PANEL_TOTAL_HEIGHT_GU: f64 = 32.0;
const GRID_PANEL_WIDTH_GU: f64 = 29.5;
const GRID_PANEL_TOTAL_HEIGHT_GU: f64 = 60.5;
const PANEL_LAYOUT_GAP_GU: f64 = 2.0;
const PANEL_ROOT_MARGIN_X_GU: f64 = 0.5;
const PANEL_ROOT_MARGIN_Y_GU: f64 = 0.5;
const PANEL_LAYOUT_AVAILABLE_WIDTH_GU: f64 = 200000.0;
#[cfg(test)]
static WORLD_PANEL_SCENE_PATH_OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);

#[derive(Debug)]
pub(crate) struct EditorInspectorSystemStopgapMmsAdapter {
    reconciler: EditorInspectorSystemStopgapMmsReconciler,
    workspace_runtime: EditorWorkspaceRuntime,
    editor_context_state: Option<Arc<Mutex<EditorContextState>>>,
    working_file_path: Arc<Mutex<PathBuf>>,
    world_panel_scene_model: Arc<Mutex<AuthoredWorldPanelSceneModel>>,
    inspector_workspace_state: Arc<Mutex<InspectorWorkspaceState>>,
    rendered_inspector_models: Arc<Mutex<Vec<InspectorPanelModel>>>,
    data_renderer: Arc<Mutex<DataRendererSystem>>,
}

impl Default for EditorInspectorSystemStopgapMmsAdapter {
    fn default() -> Self {
        Self {
            reconciler: EditorInspectorSystemStopgapMmsReconciler,
            workspace_runtime: EditorWorkspaceRuntime::default(),
            editor_context_state: None,
            working_file_path: Arc::new(Mutex::new(world_panel_scene_path())),
            world_panel_scene_model: Arc::new(Mutex::new(AuthoredWorldPanelSceneModel::default())),
            inspector_workspace_state: Arc::new(Mutex::new(InspectorWorkspaceState::default())),
            rendered_inspector_models: Arc::new(Mutex::new(Vec::new())),
            data_renderer: Arc::new(Mutex::new(DataRendererSystem::new())),
        }
    }
}

#[derive(Debug, Default)]
struct EditorInspectorSystemStopgapMmsReconciler;

impl EditorInspectorSystemStopgapMmsAdapter {
    pub fn setup_panels_for_editor(
        &mut self,
        rx: &mut RxWorld,
        world: &mut World,
        render_assets: &crate::engine::graphics::RenderAssets,
        emit: &mut dyn SignalEmitter,
        editor_root: ComponentId,
        world_panel_pos: (f32, f32, f32),
        inspector_panel_pos: (f32, f32, f32),
        editor_context_state: Arc<Mutex<EditorContextState>>,
        asset_system: &crate::engine::ecs::system::AssetSystem,
    ) {
        self.editor_context_state = Some(Arc::clone(&editor_context_state));
        let runtime_ui_root = self.workspace_runtime.get_or_create_runtime_ui_root(world);

        println!(
            "[InspectorSystem][debug] setup_panels_for_editor editor_root={editor_root:?} runtime_ui_root={runtime_ui_root:?} world_panel_pos={:?} inspector_panel_pos={:?}",
            world_panel_pos, inspector_panel_pos,
        );

        register_editor_root(self.workspace_runtime.installed_editor_roots(), editor_root);
        rebuild_world_panel_scene_model(
            &self.world_panel_scene_model,
            world,
            self.workspace_runtime.installed_editor_roots(),
        );

        let editor_context = self.editor_context();
        {
            let mut workspace = self
                .inspector_workspace_state
                .lock()
                .expect("inspector workspace mutex poisoned");
            workspace.ensure_default_panel(
                editor_root,
                editor_context
                    .selected_component
                    .or(editor_context.active_editor),
            );
        }
        let model = build_world_panel_model(
            world,
            &editor_context,
            &self
                .world_panel_scene_model
                .lock()
                .expect("world panel scene model mutex poisoned"),
        );
        let inspector_models = build_inspector_panel_models(
            world,
            &self
                .world_panel_scene_model
                .lock()
                .expect("world panel scene model mutex poisoned"),
            &self
                .inspector_workspace_state
                .lock()
                .expect("inspector workspace mutex poisoned"),
        );

        {
            let working_file_path = self
                .working_file_path
                .lock()
                .expect("working file path mutex poisoned");
            self.reconciler.reconcile_panel_layout(
                world,
                render_assets,
                emit,
                self.workspace_runtime.panel_layout_spawned_mut(),
                runtime_ui_root,
                world_panel_pos,
                inspector_panel_pos,
                &model,
                &inspector_models,
                &self.rendered_inspector_models,
                &working_file_path,
                asset_system,
                &mut *self
                    .data_renderer
                    .lock()
                    .expect("data renderer mutex poisoned"),
            );
        }

        self.refresh_world_panels(world, emit);

        self.install_shared_panel_handlers(rx, runtime_ui_root);
        self.install_editor_refresh_handlers(rx, editor_root);
    }

    fn install_shared_panel_handlers(&mut self, rx: &mut RxWorld, panel_query_root: ComponentId) {
        if self.workspace_runtime.panel_handler_installed() {
            return;
        }
        self.workspace_runtime.mark_panel_handler_installed();

        let editor_context_state = self
            .editor_context_state
            .as_ref()
            .expect("editor context state must be installed before panels")
            .clone();
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
        let click_editor_context_state = editor_context_state.clone();
        let click_world_panel_scene_model = Arc::clone(&self.world_panel_scene_model);
        let click_installed_editor_roots = Arc::clone(self.workspace_runtime.installed_editor_roots());
        let click_inspector_workspace_state = Arc::clone(&self.inspector_workspace_state);
        let click_rendered_inspector_models = Arc::clone(&self.rendered_inspector_models);
        let click_data_renderer = Arc::clone(&self.data_renderer);
        rx.add_handler_closure(
            SignalKind::Click,
            panel_query_root,
            move |world, emit, signal| {
                let Some(EventSignal::Click { renderable, .. }) = signal.event.as_ref() else {
                    return;
                };

                focus_panel_from_descendant_click(world, emit, panel_query_root, *renderable);
                handle_inspector_panel_workspace_click(
                    world,
                    emit,
                    panel_query_root,
                    *renderable,
                    &click_world_panel_scene_model,
                    &click_inspector_workspace_state,
                    &click_rendered_inspector_models,
                    &mut *click_data_renderer
                        .lock()
                        .expect("data renderer mutex poisoned"),
                );
                if handle_grid_panel_click(
                    world,
                    emit,
                    panel_query_root,
                    *renderable,
                    &click_editor_context_state,
                    &click_world_panel_scene_model,
                    &click_inspector_workspace_state,
                    &click_installed_editor_roots,
                    &click_rendered_inspector_models,
                    &mut *click_data_renderer
                        .lock()
                        .expect("data renderer mutex poisoned"),
                ) {
                    return;
                }

                let Some(panel_root) =
                    world.find_component(panel_query_root, WORLD_PANEL_ROOT_SELECTOR)
                else {
                    return;
                };
                if !is_descendant_or_self(world, panel_root, *renderable) {
                    return;
                }

                let Some(world_panel_root) =
                    world.find_component(panel_query_root, WORLD_PANEL_ROOT_SELECTOR)
                else {
                    return;
                };

                let Some(status_wrap) =
                    world.find_component(world_panel_root, PANEL_STATUS_WRAP_SELECTOR)
                else {
                    return;
                };

                let working_file_path = click_path_mutex
                    .lock()
                    .expect("working file path mutex poisoned");

                if let Some(status_text) =
                    handle_panel_button_click(world, emit, *renderable, &working_file_path)
                {
                    if panel_status_text(world, world_panel_root).as_deref()
                        != Some(status_text.as_str())
                    {
                        rerender_world_panel_status(
                            world,
                            emit,
                            world_panel_root,
                            status_wrap,
                            &status_text,
                        );
                    }

                    {
                        let mut dr = click_data_renderer
                            .lock()
                            .expect("data renderer mutex poisoned");
                        refresh_all_panel_models(
                            world,
                            emit,
                            panel_query_root,
                            &click_editor_context_state,
                            &click_world_panel_scene_model,
                            &click_inspector_workspace_state,
                            &click_installed_editor_roots,
                            &click_rendered_inspector_models,
                            true,
                            &mut *dr,
                        );
                    }
                    return;
                }

                let Some(action) = decode_panel_action_payload(
                    world,
                    *renderable,
                    WORLD_PANEL_PAYLOAD_NAME,
                    PanelKind::World,
                    PanelActionKind::Select,
                    None,
                    None,
                ) else {
                    return;
                };
                let Some(row_name) = action.item_key.as_deref() else {
                    return;
                };
                let Some(row_index) = parse_item_index(row_name) else {
                    return;
                };
                let Some(row_root) =
                    world.find_component(world_panel_root, &format!("#{row_name}"))
                else {
                    return;
                };
                let Some(selection_root) = world
                    .find_component(world_panel_root, &format!("#{WORLD_PANEL_SELECTION_NAME}"))
                else {
                    return;
                };
                crate::engine::ecs::system::selection_system::apply_selection_set(
                    world,
                    emit,
                    selection_root,
                    vec![SelectionEntry {
                        index: Some(row_index),
                        component: row_root,
                    }],
                    Some(row_root),
                );
                apply_world_panel_semantic_selection(
                    world,
                    emit,
                    panel_query_root,
                    &click_editor_context_state,
                    &click_world_panel_scene_model,
                    &click_inspector_workspace_state,
                    &click_rendered_inspector_models,
                    selection_root,
                    &mut *click_data_renderer
                        .lock()
                        .expect("data renderer mutex poisoned"),
                );
            },
        );

        let world_selection_editor_context_state = editor_context_state.clone();
        let world_selection_scene_model = Arc::clone(&self.world_panel_scene_model);
        let world_selection_inspector_workspace_state = Arc::clone(&self.inspector_workspace_state);
        let world_selection_rendered_inspector_models = Arc::clone(&self.rendered_inspector_models);
        let world_selection_data_renderer = Arc::clone(&self.data_renderer);
        rx.add_handler_closure(
            SignalKind::SelectionChanged,
            panel_query_root,
            move |world, emit, signal| {
                let Some(EventSignal::SelectionChanged {
                    selection_root,
                    selected_component: _,
                    selected_payload: _,
                    ..
                }) = signal.event.as_ref()
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
                let Some(EventSignal::SelectionChanged {
                    selection_root,
                    selected_component: _,
                    selected_payload: _,
                    ..
                }) = signal.event.as_ref()
                else {
                    return;
                };

                let is_world_panel_selection = world.component_label(*selection_root)
                    == Some(WORLD_PANEL_SELECTION_NAME)
                    || world.find_component(
                        panel_query_root,
                        &format!("#{WORLD_PANEL_SELECTION_NAME}"),
                    ) == Some(*selection_root);
                if !is_world_panel_selection {
                    return;
                }

                if world_panel_selection_matches_editor_context(
                    world,
                    &world_selection_editor_context_state,
                    *selection_root,
                ) {
                    return;
                }
                apply_world_panel_semantic_selection(
                    world,
                    emit,
                    panel_query_root,
                    &world_selection_editor_context_state,
                    &world_selection_scene_model,
                    &world_selection_inspector_workspace_state,
                    &world_selection_rendered_inspector_models,
                    *selection_root,
                    &mut *world_selection_data_renderer
                        .lock()
                        .expect("data renderer mutex poisoned"),
                );

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
                    world.find_component(panel_query_root, EDITOR_SETTINGS_SELECTION_SELECTOR)
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
                let Some(settings_panel_root) =
                    world.find_component(panel_query_root, EDITOR_SETTINGS_PANEL_ROOT_SELECTOR)
                else {
                    return;
                };

                emit.push_intent_now(
                    panel_layout_selection,
                    IntentValue::SelectionSet {
                        component_ids: vec![panel_layout_selection],
                        entries: vec![SelectionEntry {
                            index: None,
                            component: settings_panel_root,
                        }],
                        primary: Some(settings_panel_root),
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
                            component: asset_panel_root,
                        }],
                        primary: Some(asset_panel_root),
                    },
                );
            },
        );

        let layout_size_panel_query_root = panel_query_root;
        let original_mount_y: Arc<Mutex<Option<f32>>> = Arc::new(Mutex::new(None));
        let orig_mount_y = original_mount_y.clone();
        rx.add_handler_closure(
            SignalKind::LayoutRootSizeAvailable,
            layout_size_panel_query_root,
            move |world, emit, signal| {
                let Some(EventSignal::LayoutRootSizeAvailable {
                    layout_id,
                    width_wu,
                    height_wu,
                }) = signal.event.as_ref()
                else {
                    return;
                };

                if world.find_component(
                    layout_size_panel_query_root,
                    &format!("#{PANEL_LAYOUT_ROOT_NAME}"),
                ) != Some(*layout_id)
                {
                    return;
                }

                let Some(mount_root) = world.all_components().find(|&id| {
                    world
                        .component_label(id)
                        .is_some_and(|label| label == PANEL_LAYOUT_MOUNT_NAME)
                }) else {
                    return;
                };

                let Some(tc) =
                    world.get_component_by_id_as::<TransformComponent>(mount_root)
                else {
                    return;
                };

                let mut base = orig_mount_y.lock().expect("mount y mutex poisoned");
                if base.is_none() {
                    *base = Some(tc.transform.translation[1]);
                }
                let base_y = base.unwrap();

                println!(
                    "[LayoutRootSizeAvailable] layout_id={layout_id:?} width_wu={width_wu:.4} height_wu={height_wu:.4} mount_root={mount_root:?} base_y={base_y:.4} new_y={:.4}",
                    base_y + height_wu,
                );

                emit.push_intent_now(
                    mount_root,
                    IntentValue::UpdateTransform {
                        component_ids: vec![mount_root],
                        translation: [
                            tc.transform.translation[0],
                            -1.75 + height_wu, // base_y + height_wu,
                            tc.transform.translation[2],
                        ],
                        rotation_quat_xyzw: tc.transform.rotation,
                        scale: tc.transform.scale,
                    },
                );
            },
        );
    }

    fn install_editor_refresh_handlers(&mut self, rx: &mut RxWorld, editor_root: ComponentId) {
        let already_installed = self
            .workspace_runtime
            .refresh_handler_editor_roots()
            .lock()
            .expect("refresh handler editor roots mutex poisoned")
            .contains(&editor_root);
        if already_installed {
            return;
        }
        register_editor_root(
            self.workspace_runtime.refresh_handler_editor_roots(),
            editor_root,
        );

        let panel_query_root = self.workspace_runtime.runtime_ui_root_handle();
        let editor_context_state = self
            .editor_context_state
            .as_ref()
            .expect("editor context state must be installed before panels")
            .clone();
        let world_panel_scene_model = Arc::clone(&self.world_panel_scene_model);
        let inspector_workspace_state = Arc::clone(&self.inspector_workspace_state);
        let rendered_inspector_models = Arc::clone(&self.rendered_inspector_models);
        let selection_data_renderer = Arc::clone(&self.data_renderer);
        rx.add_handler_closure_named(
            SignalKind::SelectionChanged,
            editor_root,
            Some("editor_panel_refresh".to_string()),
            move |world, emit, signal| {
                let Some(EventSignal::SelectionChanged { selection_root, .. }) =
                    signal.event.as_ref()
                else {
                    return;
                };
                if *selection_root != editor_root {
                    return;
                }
                let Some(panel_query_root) = *panel_query_root
                    .lock()
                    .expect("runtime ui root mutex poisoned")
                else {
                    return;
                };
                sync_world_panel_selection(
                    world,
                    emit,
                    panel_query_root,
                    &editor_context_state,
                    &world_panel_scene_model,
                );
                sync_and_refresh_inspector_panels(
                    world,
                    emit,
                    panel_query_root,
                    &editor_context_state,
                    &world_panel_scene_model,
                    &inspector_workspace_state,
                    &rendered_inspector_models,
                    &mut *selection_data_renderer
                        .lock()
                        .expect("data renderer mutex poisoned"),
                );
            },
        );

        // Intentionally no ParentChanged-scoped full refresh here. Runtime systems such as
        // AvatarControl re-parent large authored subtrees during the first tick, and rebuilding
        // the cached world-panel model on every such mutation can wedge the first frame.
    }

    fn editor_context(&self) -> EditorContextState {
        self.editor_context_state
            .as_ref()
            .expect("editor context state must be installed before panels")
            .lock()
            .expect("editor context state mutex poisoned")
            .clone()
    }

    fn refresh_world_panels(&self, world: &mut World, emit: &mut dyn SignalEmitter) {
        let Some(panel_query_root) = self.workspace_runtime.current_runtime_ui_root() else {
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
        let Some(selection_root) =
            world.find_component(world_panel_root, WORLD_PANEL_SELECTION_SELECTOR)
        else {
            return;
        };

        let editor_context = self.editor_context();
        let model = build_world_panel_model(
            world,
            &editor_context,
            &self
                .world_panel_scene_model
                .lock()
                .expect("world panel scene model mutex poisoned"),
        );
        rerender_world_panel_content(
            world,
            emit,
            content_slot,
            selection_root,
            &model.rows,
            model.selected_index,
            &mut *self
                .data_renderer
                .lock()
                .expect("data renderer mutex poisoned"),
        );

        rerender_grid_panel_from_context(
            world,
            emit,
            panel_query_root,
            &editor_context,
            &mut *self
                .data_renderer
                .lock()
                .expect("data renderer mutex poisoned"),
        );
        sync_editor_settings_panel_selection(world, emit, panel_query_root, &editor_context);

        let inspector_models = build_inspector_panel_models(
            world,
            &self
                .world_panel_scene_model
                .lock()
                .expect("world panel scene model mutex poisoned"),
            &self
                .inspector_workspace_state
                .lock()
                .expect("inspector workspace mutex poisoned"),
        );
        let Some(bottom_row_root) = panel_layout_bottom_row_id(world, panel_query_root) else {
            return;
        };
        rerender_inspector_panels(
            world,
            emit,
            bottom_row_root,
            &inspector_models,
            &self.rendered_inspector_models,
            &mut *self
                .data_renderer
                .lock()
                .expect("data renderer mutex poisoned"),
        );
    }
}

fn refresh_all_panel_models(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    editor_context_state: &Arc<Mutex<EditorContextState>>,
    world_panel_scene_model: &Arc<Mutex<AuthoredWorldPanelSceneModel>>,
    inspector_workspace_state: &Arc<Mutex<InspectorWorkspaceState>>,
    installed_editor_roots: &Arc<Mutex<Vec<ComponentId>>>,
    rendered_inspector_models: &Arc<Mutex<Vec<InspectorPanelModel>>>,
    rebuild_world_panel: bool,
    data_renderer: &mut DataRendererSystem,
) {
    if rebuild_world_panel {
        rebuild_world_panel_scene_model(world_panel_scene_model, world, installed_editor_roots);
    }

    let editor_context = editor_context_state
        .lock()
        .expect("editor context state mutex poisoned")
        .clone();

    let Some(world_panel_root) = world.find_component(panel_query_root, WORLD_PANEL_ROOT_SELECTOR)
    else {
        return;
    };
    if let Some(content_slot) = world.find_component(world_panel_root, PANEL_CONTENT_SLOT_SELECTOR)
        && let Some(selection_root) =
            world.find_component(world_panel_root, WORLD_PANEL_SELECTION_SELECTOR)
    {
        let world_model = build_world_panel_model(
            world,
            &editor_context,
            &world_panel_scene_model
                .lock()
                .expect("world panel scene model mutex poisoned"),
        );
        rerender_world_panel_content(
            world,
            emit,
            content_slot,
            selection_root,
            &world_model.rows,
            world_model.selected_index,
            data_renderer,
        );
    }

    rerender_grid_panel_from_context(world, emit, panel_query_root, &editor_context, data_renderer);
    sync_editor_settings_panel_selection(world, emit, panel_query_root, &editor_context);

    sync_and_refresh_inspector_panels(
        world,
        emit,
        panel_query_root,
        editor_context_state,
        world_panel_scene_model,
        inspector_workspace_state,
        rendered_inspector_models,
        data_renderer,
    );
}

fn sync_and_refresh_inspector_panels(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    editor_context_state: &Arc<Mutex<EditorContextState>>,
    world_panel_scene_model: &Arc<Mutex<AuthoredWorldPanelSceneModel>>,
    inspector_workspace_state: &Arc<Mutex<InspectorWorkspaceState>>,
    rendered_inspector_models: &Arc<Mutex<Vec<InspectorPanelModel>>>,
    data_renderer: &mut DataRendererSystem,
) {
    let editor_context = editor_context_state
        .lock()
        .expect("editor context state mutex poisoned")
        .clone();
    println!(
        "[InspectorSystem][trace] rebuild inspector target={:?} active_editor={:?}",
        editor_context.selected_component, editor_context.active_editor
    );
    trace_suspicious_inspector_target(world, editor_context.selected_component);

    {
        let mut workspace = inspector_workspace_state
            .lock()
            .expect("inspector workspace mutex poisoned");
        sync_inspector_workspace_to_selection(world, &editor_context, &mut workspace);
    }

    let Some(bottom_row_root) = panel_layout_bottom_row_id(world, panel_query_root) else {
        return;
    };

    let inspector_models = build_inspector_panel_models(
        world,
        &world_panel_scene_model
            .lock()
            .expect("world panel scene model mutex poisoned"),
        &inspector_workspace_state
            .lock()
            .expect("inspector workspace mutex poisoned"),
    );
    rerender_inspector_panels(
        world,
        emit,
        bottom_row_root,
        &inspector_models,
        rendered_inspector_models,
        data_renderer,
    );
}

fn refresh_inspector_panels_from_workspace(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    world_panel_scene_model: &Arc<Mutex<AuthoredWorldPanelSceneModel>>,
    inspector_workspace_state: &Arc<Mutex<InspectorWorkspaceState>>,
    rendered_inspector_models: &Arc<Mutex<Vec<InspectorPanelModel>>>,
    data_renderer: &mut DataRendererSystem,
) {
    let Some(bottom_row_root) = panel_layout_bottom_row_id(world, panel_query_root) else {
        return;
    };

    let inspector_models = build_inspector_panel_models(
        world,
        &world_panel_scene_model
            .lock()
            .expect("world panel scene model mutex poisoned"),
        &inspector_workspace_state
            .lock()
            .expect("inspector workspace mutex poisoned"),
    );
    rerender_inspector_panels(
        world,
        emit,
        bottom_row_root,
        &inspector_models,
        rendered_inspector_models,
        data_renderer,
    );
}

fn apply_world_panel_semantic_selection(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    editor_context_state: &Arc<Mutex<EditorContextState>>,
    world_panel_scene_model: &Arc<Mutex<AuthoredWorldPanelSceneModel>>,
    inspector_workspace_state: &Arc<Mutex<InspectorWorkspaceState>>,
    rendered_inspector_models: &Arc<Mutex<Vec<InspectorPanelModel>>>,
    selection_root: ComponentId,
    data_renderer: &mut DataRendererSystem,
) {
    let Some((selected_component, selected_payload)) = world
        .get_component_by_id_as::<SelectionComponent>(selection_root)
        .map(|selection| (selection.selected_component, selection.selected_payload))
    else {
        return;
    };
    let Some(target_component) =
        resolve_semantic_target_from_payload(world, selected_payload, selected_component)
    else {
        return;
    };
    let active_editor = nearest_editor_ancestor(world, target_component);
    let gizmo_target = nearest_transform_ancestor(world, target_component);
    let used_editor_selection_path =
        active_editor
            .zip(gizmo_target)
            .map(|(editor_root, transform)| {
                select_editor_target(world, emit, editor_root, transform, true);
                transform
            });
    {
        let mut editor_context = editor_context_state
            .lock()
            .expect("editor context state mutex poisoned");
        editor_context.selected_component = Some(target_component);
        if active_editor.is_some() {
            editor_context.active_editor = active_editor;
        }
    }

    println!(
        "[InspectorSystem][trace] world_panel selection_root={selection_root:?} clicked_row={:?} payload={:?} authored_target={target_component:?} active_editor={active_editor:?} gizmo_target={gizmo_target:?} select_editor_target_ran={}",
        selected_component,
        selected_payload,
        used_editor_selection_path.is_some()
    );

    if let Some(world_panel_root) =
        world.find_component(panel_query_root, WORLD_PANEL_ROOT_SELECTOR)
        && let Some(status_wrap) =
            world.find_component(world_panel_root, PANEL_STATUS_WRAP_SELECTOR)
    {
        let status_text = format!(
            "selected {}",
            world_panel_item_label(world, target_component)
        );
        rerender_world_panel_status(world, emit, world_panel_root, status_wrap, &status_text);
    }

    sync_and_refresh_inspector_panels(
        world,
        emit,
        panel_query_root,
        editor_context_state,
        world_panel_scene_model,
        inspector_workspace_state,
        rendered_inspector_models,
        data_renderer,
    );
}

fn world_panel_selection_matches_editor_context(
    world: &World,
    editor_context_state: &Arc<Mutex<EditorContextState>>,
    selection_root: ComponentId,
) -> bool {
    let Some(selection) = world.get_component_by_id_as::<SelectionComponent>(selection_root) else {
        return false;
    };
    let Some(target_component) = resolve_semantic_target_from_payload(
        world,
        selection.selected_payload,
        selection.selected_component,
    ) else {
        return false;
    };
    let active_editor = nearest_editor_ancestor(world, target_component);
    let editor_context = editor_context_state
        .lock()
        .expect("editor context state mutex poisoned")
        .clone();
    editor_context.selected_component == Some(target_component)
        && (active_editor.is_none() || editor_context.active_editor == active_editor)
}

impl EditorInspectorSystemStopgapMmsReconciler {
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
        inspector_models: &[InspectorPanelModel],
        rendered_inspector_models: &Arc<Mutex<Vec<InspectorPanelModel>>>,
        working_file_path: &Path,
        asset_system: &crate::engine::ecs::system::AssetSystem,
        data_renderer: &mut DataRendererSystem,
    ) {
        let existing_world_panel =
            self.find_world_panel_node(world, panel_query_root, WORLD_PANEL_ROOT_SELECTOR);
        let existing_inspector_panel = self
            .find_inspector_panel_nodes(world, panel_query_root)
            .first()
            .copied();
        let existing_panel_mount = world.all_components().find(|&component_id| {
            world
                .component_label(component_id)
                .is_some_and(|label| label == PANEL_LAYOUT_MOUNT_NAME)
        });

        println!(
            "[InspectorSystem][debug] reconcile_panel_layout panel_query_root={panel_query_root:?} existing_world_panel={existing_world_panel:?} existing_inspector_panel={existing_inspector_panel:?} existing_panel_mount={existing_panel_mount:?}"
        );

        if *panel_layout_spawned {
            if existing_world_panel.is_none()
                && existing_inspector_panel.is_none()
                && existing_panel_mount.is_none()
            {
                println!(
                    "[InspectorSystem][debug] panel layout flag was stale for panel_query_root={panel_query_root:?}; respawning missing panel layout"
                );
                *panel_layout_spawned = false;
            } else {
                println!(
                    "[InspectorSystem][debug] panel layout already spawned for panel_query_root={panel_query_root:?}; skipping duplicate spawn"
                );
                return;
            }
        }

        if existing_world_panel.is_some() && existing_inspector_panel.is_some() {
            println!(
                "[InspectorSystem][debug] panel layout already present for panel_query_root={panel_query_root:?}; skipping spawn"
            );
            *panel_layout_spawned = true;
            return;
        }

        if existing_panel_mount.is_some() {
            println!(
                "[InspectorSystem][debug] pending panel layout mount already exists for panel_query_root={panel_query_root:?}; skipping duplicate spawn"
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
            inspector_models,
            rendered_inspector_models,
            working_file_path,
            asset_system,
            data_renderer,
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

    fn find_panel_layout_root(
        &self,
        world: &World,
        panel_query_root: ComponentId,
    ) -> Option<ComponentId> {
        world.find_component(panel_query_root, &format!("#{PANEL_LAYOUT_ROOT_NAME}"))
    }

    fn find_inspector_panel_nodes(
        &self,
        world: &World,
        panel_query_root: ComponentId,
    ) -> Vec<ComponentId> {
        let Some(layout_root) = self.find_panel_layout_root(world, panel_query_root) else {
            return Vec::new();
        };

        world
            .children_of(layout_root)
            .iter()
            .copied()
            .filter(|&child| {
                world
                    .component_label(child)
                    .is_some_and(|label| label == "inspector_panel_root")
            })
            .collect()
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
        inspector_models: &[InspectorPanelModel],
        rendered_inspector_models: &Arc<Mutex<Vec<InspectorPanelModel>>>,
        working_file_path: &Path,
        asset_system: &crate::engine::ecs::system::AssetSystem,
        data_renderer: &mut DataRendererSystem,
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
            PanelKind::World,
            "world panel",
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
            PanelKind::Assets,
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
            PanelKind::Paint,
            "paint panel",
        ) {
            Some(panel) => panel,
            None => return,
        };

        let grid_panel = match build_panel_component_expr(
            world,
            emit,
            grid_panel_asset_path(),
            "grid_panel",
            vec![
                Value::String("Grids".to_string()),
                Value::Array(Vec::new()),
                world_panel_title_color.clone(),
                world_panel_bg.clone(),
                world_panel_item_bg.clone(),
            ],
            PanelKind::Grid,
            "grid panel",
        ) {
            Some(panel) => panel,
            None => return,
        };

        let editor_settings_panel = match build_panel_component_expr(
            world,
            emit,
            editor_settings_panel_asset_path(),
            "editor_settings_panel",
            vec![
                Value::String("Editor".to_string()),
                world_panel_title_color.clone(),
                world_panel_bg.clone(),
            ],
            PanelKind::Inspector,
            "editor settings panel",
        ) {
            Some(panel) => panel,
            None => return,
        };

        let _ = inspector_panel_pos;
        let anchor_pos = world_panel_pos;

        let total_height_gu = WORLD_PANEL_TOTAL_HEIGHT_GU
            .max(INSPECTOR_PANEL_TOTAL_HEIGHT_GU)
            .max(ASSET_PANEL_TOTAL_HEIGHT_GU)
            .max(PAINT_PANEL_TOTAL_HEIGHT_GU)
            .max(GRID_PANEL_TOTAL_HEIGHT_GU)
            .max(EDITOR_SETTINGS_PANEL_TOTAL_HEIGHT_GU)
            * 2.0
            + PANEL_LAYOUT_GAP_GU
            + (PANEL_ROOT_MARGIN_Y_GU * 2.0);

        let world_panel = decorate_panel_root_ce(world_panel, PANEL_LAYOUT_GAP_GU);
        let paint_panel = decorate_panel_root_ce(paint_panel, PANEL_LAYOUT_GAP_GU);
        let asset_panel = decorate_panel_root_ce(asset_panel, PANEL_LAYOUT_GAP_GU);
        let grid_panel = decorate_panel_root_ce(grid_panel, PANEL_LAYOUT_GAP_GU);
        let editor_settings_panel =
            decorate_panel_root_ce(editor_settings_panel, PANEL_LAYOUT_GAP_GU);

        let (panel_mount_root, layout_root_id) = match spawn_panel_layout_mount(
            world,
            emit,
            PanelLayoutMountSpec {
                anchor_pos,
                total_height_gu,
                available_width_gu: PANEL_LAYOUT_AVAILABLE_WIDTH_GU,
                text_scale: PANEL_LAYOUT_TEXT_SCALE,
                mount_name: PANEL_LAYOUT_MOUNT_NAME.to_string(),
                layout_name: PANEL_LAYOUT_ROOT_NAME.to_string(),
                children: vec![
                    editor_settings_panel,
                    paint_panel,
                    grid_panel,
                    asset_panel,
                    world_panel,
                ],
            },
        ) {
            Ok(ids) => ids,
            Err(error) => {
                eprintln!("[InspectorSystemStopgapMmsAdapter] panel layout spawn error: {error}");
                return;
            }
        };

        // Add SelectionComponent to the LayoutRoot so we can select individual panels.
        let selection = ensure_panel_layout_selection(world, layout_root_id);
        world.init_component_tree(selection, emit);

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
        if let Some(panel_layout_selection) = panel_layout_selection_id(world, panel_mount_root) {
            if let Some(world_panel_root) =
                world.find_component(panel_mount_root, WORLD_PANEL_ROOT_SELECTOR)
            {
                emit.push_intent_now(
                    panel_layout_selection,
                    IntentValue::SelectionSet {
                        component_ids: vec![panel_layout_selection],
                        entries: vec![SelectionEntry {
                            index: Some(0),
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
                && let Some(selection_root) =
                    world.find_component(world_panel_root, WORLD_PANEL_SELECTION_SELECTOR)
            {
                rerender_world_panel_content(
                    world,
                    emit,
                    content_slot,
                    selection_root,
                    &model.rows,
                    model.selected_index,
                    data_renderer,
                );
            }
        }
        if let Some(layout_root) =
            world.find_component(panel_mount_root, &format!("#{PANEL_LAYOUT_ROOT_NAME}"))
        {
            rerender_inspector_panels(
                world,
                emit,
                layout_root,
                &inspector_models,
                rendered_inspector_models,
                data_renderer,
            );
        }

        let grid_context = EditorContextState::default();
        rerender_grid_panel_from_context(
            world,
            emit,
            panel_mount_root,
            &grid_context,
            data_renderer,
        );
        sync_editor_settings_panel_selection(world, emit, panel_mount_root, &grid_context);

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

    let serializable_roots = filtered_root_ids_for_roots(world, &editor_scene_roots(world));
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

    let removable_roots = filtered_root_ids_for_roots(world, &editor_scene_roots(world));
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
        if should_skip_loaded_root(component) {
            continue;
        }
        spawn_tree(component, None, world, emit)?;
        loaded_roots += 1;
    }
    Ok(loaded_roots)
}

fn should_skip_loaded_root(component: &MaterializedCE) -> bool {
    let Some(name) = materialized_ce_name(component) else {
        return false;
    };

    matches!(
        name,
        EDITOR_RUNTIME_UI_ROOT_NAME
            | PANEL_LAYOUT_MOUNT_NAME
            | PANEL_LAYOUT_ROOT_NAME
            | PANEL_LAYOUT_SELECTION_NAME
            | "world_panel_root"
            | "inspector_panel_root"
            | "assets_root"
            | "paint_panel_root"
            | "editor_settings_panel_root"
            | "world_panel_content_root"
            | "inspector_panel_content_root"
            | "panel_status_root"
            | "rows_mount"
    ) || name.starts_with(INSPECTOR_PANEL_INSTANCE_PREFIX)
}

fn panel_layout_bottom_row_id(world: &World, panel_query_root: ComponentId) -> Option<ComponentId> {
    panel_layout_root_id(world, panel_query_root)
}

fn materialized_ce_name(component: &MaterializedCE) -> Option<&str> {
    component.named.iter().find_map(|(key, value)| {
        if key != "name" {
            return None;
        }
        match value {
            Value::String(name) => Some(name.as_str()),
            _ => None,
        }
    })
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

fn rerender_inspector_panels(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    layout_root: ComponentId,
    models: &[InspectorPanelModel],
    rendered_models: &Arc<Mutex<Vec<InspectorPanelModel>>>,
    data_renderer: &mut DataRendererSystem,
) {
    let start = std::time::Instant::now();
    let existing_instance_roots = world
        .children_of(layout_root)
        .iter()
        .copied()
        .filter(|&child| inspector_panel_instance_id_on_root(world, child).is_some())
        .collect::<Vec<_>>();

    let existing_by_id = existing_instance_roots
        .iter()
        .filter_map(|&child| {
            inspector_panel_instance_id_on_root(world, child).map(|panel_id| (panel_id, child))
        })
        .collect::<std::collections::HashMap<_, _>>();

    let desired_ids = models
        .iter()
        .map(|model| model.panel_id)
        .collect::<Vec<_>>();
    for (&panel_id, &child) in &existing_by_id {
        if desired_ids.contains(&panel_id) {
            continue;
        }
        emit.push_intent_now(
            child,
            IntentValue::RemoveSubtree {
                component_ids: vec![child],
            },
        );
    }

    for (index, model) in models.iter().enumerate() {
        if let Some(&instance_root) = existing_by_id.get(&model.panel_id) {
            let previous_model = rendered_models
                .lock()
                .expect("rendered inspector models mutex poisoned")
                .iter()
                .find(|cached| cached.panel_id == model.panel_id)
                .cloned();
            update_inspector_panel_instance_tree(
                world,
                emit,
                instance_root,
                model,
                previous_model.as_ref(),
                data_renderer,
            );
            continue;
        }

        let instance_root =
            spawn_inspector_panel_instance_tree(world, emit, model, index, data_renderer);
        emit.push_intent_now(
            layout_root,
            IntentValue::Attach {
                parents: vec![layout_root],
                child: instance_root,
            },
        );
    }
    *rendered_models
        .lock()
        .expect("rendered inspector models mutex poisoned") = models.to_vec();
    mark_nearest_layout_dirty(world, layout_root);
    println!(
        "[InspectorPanel] rerender_inspector_panels took {:?}",
        start.elapsed()
    );
}

fn rerender_single_inspector_panel_sidebar(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    inspector_panel_root: ComponentId,
    panel_id: InspectorPanelId,
    sidebar_slot: ComponentId,
    selection_root: Option<ComponentId>,
    rows: &[InspectorPanelRow],
    data_renderer: &mut DataRendererSystem,
) {
    let start = std::time::Instant::now();
    println!(
        "[InspectorSystem][trace] rerender_single_inspector_panel_sidebar panel_id={} inspector_panel_root={inspector_panel_root:?} sidebar_slot={sidebar_slot:?} row_count={}",
        panel_id,
        rows.len()
    );

    // Transition cleanup: remove old content_root if present (pre-data-renderer subtrees).
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

    let items: Vec<UiItem> = rows
        .iter()
        .enumerate()
        .map(|(index, row)| UiItem {
            key: format!("{INSPECTOR_ITEM_PREFIX}{index}"),
            kind: match row.kind {
                InspectorPanelRowKind::Component => UiItemKind::Component,
                InspectorPanelRowKind::Info => UiItemKind::Info,
            },
            label: row.display_label.clone(),
            selected: row.selected,
            target_ref: row.target_component,
        })
        .collect();

    let Ok(container) =
        data_renderer.render_list(world, emit, sidebar_slot, &INSPECTOR_ROW_SPEC, &items)
    else {
        return;
    };

    if let Some(selection_root) = selection_root {
        if let Some((index, _)) = rows.iter().enumerate().find(|(_, row)| row.selected)
            && let Some(row_root) =
                world.find_component(container, &format!("#{INSPECTOR_ITEM_PREFIX}{index}"))
        {
            let selected_payload = resolve_selected_inspector_panel_payload(world, row_root);
            apply_selection_set(
                world,
                emit,
                selection_root,
                vec![SelectionEntry {
                    index: Some(index),
                    component: row_root,
                }],
                selected_payload.or(Some(row_root)),
            );
        } else {
            apply_selection_set(world, emit, selection_root, Vec::new(), None);
        }
    }
    println!(
        "[InspectorPanel] rerender_single_inspector_panel_sidebar took {:?}",
        start.elapsed()
    );
}

fn rerender_single_inspector_panel_detail(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    _inspector_panel_root: ComponentId,
    detail_slot: ComponentId,
    detail: &InspectorPanelDetailModel,
    data_renderer: &mut DataRendererSystem,
) {
    let start = std::time::Instant::now();
    if detail.id.is_empty() && detail.guid.is_empty() {
        data_renderer.clear_slot(world, emit, detail_slot);
        return;
    }

    let detail_item = UiDetailItem {
        name: detail.name.clone(),
        id: detail.id.clone(),
        guid: detail.guid.clone(),
    };

    if let Err(error) = data_renderer.render_detail(
        world,
        emit,
        detail_slot,
        &INSPECTOR_DETAIL_SPEC,
        &detail_item,
    ) {
        eprintln!("[InspectorSystemStopgapMmsAdapter] detail render error: {error}");
    }
    println!(
        "[InspectorPanel] rerender_single_inspector_panel_detail took {:?}",
        start.elapsed()
    );
}

fn update_inspector_panel_instance_tree(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    instance_root: ComponentId,
    model: &InspectorPanelModel,
    previous_model: Option<&InspectorPanelModel>,
    data_renderer: &mut DataRendererSystem,
) {
    let inspector_panel_root = instance_root;
    let Some(sidebar_slot) =
        world.find_component(inspector_panel_root, INSPECTOR_PANEL_SIDEBAR_SLOT_SELECTOR)
    else {
        return;
    };
    let Some(detail_slot) =
        world.find_component(inspector_panel_root, INSPECTOR_PANEL_DETAIL_SLOT_SELECTOR)
    else {
        return;
    };

    let title_changed = previous_model.is_none_or(|previous| previous.title != model.title);
    if title_changed
        && let Some(title_label) = world.find_component(inspector_panel_root, "#title_label")
    {
        emit.push_intent_now(
            title_label,
            IntentValue::SetText {
                component_ids: vec![title_label],
                text: model.title.clone(),
            },
        );
    }

    let pin_changed = previous_model.is_none_or(|previous| previous.pinned != model.pinned);
    if pin_changed {
        if let Some(pin_button) =
            world.find_component(inspector_panel_root, INSPECTOR_PANEL_PIN_BUTTON_SELECTOR)
        {
            set_inspector_pin_button_state(world, emit, pin_button, model.pinned);
        }
    }

    if previous_model.is_none_or(|previous| previous.rows != model.rows) {
        rerender_single_inspector_panel_sidebar(
            world,
            emit,
            inspector_panel_root,
            model.panel_id,
            sidebar_slot,
            world.find_component(inspector_panel_root, INSPECTOR_PANEL_SELECTION_SELECTOR),
            &model.rows,
            data_renderer,
        );
    }
    if previous_model.is_none_or(|previous| previous.detail != model.detail) {
        rerender_single_inspector_panel_detail(
            world,
            emit,
            inspector_panel_root,
            detail_slot,
            &model.detail,
            data_renderer,
        );
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

fn clicked_inspector_panel_instance_id(
    world: &World,
    node: ComponentId,
) -> Option<InspectorPanelId> {
    let mut current = Some(node);
    while let Some(component_id) = current {
        if let Some(id) = inspector_panel_instance_id_on_root(world, component_id) {
            return Some(id);
        }
        current = world.parent_of(component_id);
    }
    None
}

fn find_inspector_panel_instance_root(
    world: &World,
    panel_query_root: ComponentId,
    panel_id: InspectorPanelId,
) -> Option<ComponentId> {
    world
        .find_component(panel_query_root, &format!("#{PANEL_LAYOUT_ROOT_NAME}"))
        .and_then(|layout_root| {
            world
                .children_of(layout_root)
                .iter()
                .copied()
                .find(|&child| inspector_panel_instance_id_on_root(world, child) == Some(panel_id))
        })
}

fn sync_inspector_workspace_to_selection(
    world: &World,
    editor_context: &EditorContextState,
    workspace: &mut InspectorWorkspaceState,
) {
    let selected_target = editor_context
        .selected_component
        .or(editor_context.active_editor);
    let editor_root = selected_target
        .and_then(|component_id| nearest_editor_ancestor(world, component_id))
        .or(editor_context.active_editor);

    clear_missing_inspector_targets(workspace, |component_id| {
        world.get_component_record(component_id).is_some()
    });

    let Some(editor_root) = editor_root else {
        return;
    };

    let next_workspace = reduce_inspector_workspace_state(
        workspace,
        &InspectorWorkspaceEvent::SelectionChanged {
            editor_root,
            selected_target,
        },
    );
    *workspace = next_workspace;
    if let Some(active_index) = workspace.active_panel_index()
        && let Some(active_panel) = workspace.panels.get_mut(active_index)
    {
        active_panel.subtree_selection.focused_row = active_panel.inspected;
    }
}

fn handle_inspector_panel_workspace_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    renderable: ComponentId,
    world_panel_scene_model: &Arc<Mutex<AuthoredWorldPanelSceneModel>>,
    inspector_workspace_state: &Arc<Mutex<InspectorWorkspaceState>>,
    rendered_inspector_models: &Arc<Mutex<Vec<InspectorPanelModel>>>,
    data_renderer: &mut DataRendererSystem,
) {
    let Some(panel_id) = clicked_inspector_panel_instance_id(world, renderable) else {
        return;
    };

    let mut rerender_needed;
    {
        let mut workspace = inspector_workspace_state
            .lock()
            .expect("inspector workspace mutex poisoned");
        rerender_needed = workspace.active_panel != Some(panel_id);
        let focused_workspace = reduce_inspector_workspace_state(
            &workspace,
            &InspectorWorkspaceEvent::PanelFocused { panel_id },
        );
        *workspace = focused_workspace;

        if let Some(panel_root) =
            find_inspector_panel_instance_root(world, panel_query_root, panel_id)
            && let Some(pin_button) =
                world.find_component(panel_root, INSPECTOR_PANEL_PIN_BUTTON_SELECTOR)
            && is_descendant_or_self(world, pin_button, renderable)
        {
            let toggled_workspace = reduce_inspector_workspace_state(
                &workspace,
                &InspectorWorkspaceEvent::PanelPinToggled { panel_id },
            );
            *workspace = toggled_workspace;
            rerender_needed = true;
        }

        if let Some(action) = decode_panel_action_payload(
            world,
            renderable,
            INSPECTOR_PANEL_PAYLOAD_NAME,
            PanelKind::Inspector,
            PanelActionKind::Select,
            Some(panel_id),
            None,
        ) && let Some(row_name) = action.item_key.as_deref()
            && let Some(row_index) = parse_inspector_item_index(row_name)
        {
            let scene_model = world_panel_scene_model
                .lock()
                .expect("world panel scene model mutex poisoned")
                .clone();
            if let Some(panel) = workspace
                .panels
                .iter_mut()
                .find(|panel| panel.panel_id == panel_id)
                && let Some(inspected_root) = panel.inspected
            {
                let rows = build_inspector_panel_rows(world, &scene_model, panel, inspected_root);
                if let Some(target_component) =
                    rows.get(row_index).and_then(|row| row.target_component)
                    && panel.subtree_selection.focused_row != Some(target_component)
                {
                    panel.subtree_selection.focused_row = Some(target_component);
                    // Keep the existing sidebar subtree alive on row clicks. The
                    // SelectionComponent/Option styling already updates the visual
                    // highlight, and rebuilding the sidebar here churns the
                    // selection subtree while the detail pane is disabled.
                }
            }
        }
    }

    if rerender_needed {
        refresh_inspector_panels_from_workspace(
            world,
            emit,
            panel_query_root,
            world_panel_scene_model,
            inspector_workspace_state,
            rendered_inspector_models,
            data_renderer,
        );
    }
}

fn handle_grid_panel_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    renderable: ComponentId,
    editor_context_state: &Arc<Mutex<EditorContextState>>,
    world_panel_scene_model: &Arc<Mutex<AuthoredWorldPanelSceneModel>>,
    inspector_workspace_state: &Arc<Mutex<InspectorWorkspaceState>>,
    installed_editor_roots: &Arc<Mutex<Vec<ComponentId>>>,
    rendered_inspector_models: &Arc<Mutex<Vec<InspectorPanelModel>>>,
    data_renderer: &mut DataRendererSystem,
) -> bool {
    let Some(grid_panel_root) = world.find_component(panel_query_root, GRID_PANEL_ROOT_SELECTOR)
    else {
        return false;
    };
    if !is_descendant_or_self(world, grid_panel_root, renderable) {
        return false;
    }

    let editor_context = editor_context_state
        .lock()
        .expect("editor context state mutex poisoned")
        .clone();
    let editor_root = editor_context.active_editor.or_else(|| {
        installed_editor_roots
            .lock()
            .expect("installed editor roots mutex poisoned")
            .first()
            .copied()
    });
    let Some(editor_root) = editor_root else {
        return true;
    };

    if let Some(add_button) = world.find_component(grid_panel_root, GRID_PANEL_ADD_BUTTON_SELECTOR)
        && is_descendant_or_self(world, add_button, renderable)
    {
        let _owner_transform =
            spawn_default_grid_for_editor(world, emit, editor_root, &editor_context);
        let mut refreshed_context = editor_context.clone();
        refreshed_context.active_editor = Some(editor_root);
        rerender_grid_panel_from_context(
            world,
            emit,
            panel_query_root,
            &refreshed_context,
            data_renderer,
        );
        return true;
    }

    if let Some(action) = decode_panel_action_payload(
        world,
        renderable,
        GRID_PANEL_TOGGLE_PAYLOAD_NAME,
        PanelKind::Grid,
        PanelActionKind::Toggle,
        None,
        None,
    ) && let Some(owner_transform) = action.target_component
    {
        toggle_grid_visibility(world, owner_transform);
        rerender_grid_panel_from_context(world, emit, panel_query_root, &editor_context, data_renderer);
        return true;
    }

    if let Some(action) = decode_panel_action_payload(
        world,
        renderable,
        GRID_PANEL_DELETE_PAYLOAD_NAME,
        PanelKind::Grid,
        PanelActionKind::Delete,
        None,
        None,
    ) && let Some(owner_transform) = action.target_component
    {
        if editor_context.selected_component == Some(owner_transform)
            && let Some(editor) = world.get_component_by_id_as_mut::<crate::engine::ecs::component::EditorComponent>(editor_root)
        {
            editor.selected = Some(editor_root);
        }
        {
            let mut editor_context = editor_context_state
                .lock()
                .expect("editor context state mutex poisoned");
            editor_context.active_editor = Some(editor_root);
            if editor_context.selected_component == Some(owner_transform) {
                editor_context.selected_component = Some(editor_root);
            }
        }
        let _ = world.remove_component_subtree(owner_transform);
        refresh_all_panel_models(
            world,
            emit,
            panel_query_root,
            editor_context_state,
            world_panel_scene_model,
            inspector_workspace_state,
            installed_editor_roots,
            rendered_inspector_models,
            true,
            data_renderer,
        );
        return true;
    }

    if let Some(action) = decode_panel_action_payload(
        world,
        renderable,
        GRID_PANEL_ROW_PAYLOAD_NAME,
        PanelKind::Grid,
        PanelActionKind::Select,
        None,
        None,
    ) && let Some(owner_transform) = action.target_component
    {
        select_editor_target(world, emit, editor_root, owner_transform, true);
        {
            let mut editor_context = editor_context_state
                .lock()
                .expect("editor context state mutex poisoned");
            editor_context.active_editor = Some(editor_root);
            editor_context.selected_component = Some(owner_transform);
        }
        refresh_all_panel_models(
            world,
            emit,
            panel_query_root,
            editor_context_state,
            world_panel_scene_model,
            inspector_workspace_state,
            installed_editor_roots,
            rendered_inspector_models,
            false,
            data_renderer,
        );
        return true;
    }

    true
}

fn rerender_grid_panel_from_context(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    editor_context: &EditorContextState,
    data_renderer: &mut DataRendererSystem,
) {
    let Some(grid_panel_root) = world.find_component(panel_query_root, GRID_PANEL_ROOT_SELECTOR)
    else {
        return;
    };
    let Some(content_slot) = world.find_component(grid_panel_root, PANEL_CONTENT_SLOT_SELECTOR)
    else {
        return;
    };
    let Some(editor_root) = editor_context.active_editor else {
        data_renderer.clear_slot(world, emit, content_slot);
        return;
    };

    let grids = GridSystem::new();
    let model = build_grid_panel_model(world, &grids, editor_root);
    let items = grid_panel_items(&model);
    if let Err(error) =
        data_renderer.render_list(world, emit, content_slot, &GRID_PANEL_ROW_SPEC, &items)
    {
        eprintln!("[InspectorSystem] grid panel content render error: {error}");
        return;
    }
}

fn sync_editor_settings_panel_selection(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    editor_context: &EditorContextState,
) {
    let Some(selection_root) =
        world.find_component(panel_query_root, EDITOR_SETTINGS_SELECTION_SELECTOR)
    else {
        return;
    };

    let desired_option = match editor_context.interaction_mode {
        EditorInteractionMode::Select => EditorSettingsOption::Select,
        EditorInteractionMode::Cursor3d => EditorSettingsOption::Cursor3d,
    };
    let Some(panel_root) = world.find_component(panel_query_root, EDITOR_SETTINGS_PANEL_ROOT_SELECTOR)
    else {
        return;
    };
    let Some(row_root) = world.find_component(panel_root, &format!("#{}", desired_option.row_name()))
    else {
        return;
    };
    apply_selection_set(
        world,
        emit,
        selection_root,
        vec![SelectionEntry {
            index: Some(match desired_option {
                EditorSettingsOption::Select => 0,
                EditorSettingsOption::Cursor3d => 1,
            }),
            component: row_root,
        }],
        Some(row_root),
    );
}

fn rerender_world_panel_for_context(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    editor_context: &EditorContextState,
    world_panel_scene_model: &Arc<Mutex<AuthoredWorldPanelSceneModel>>,
    installed_editor_roots: &Arc<Mutex<Vec<ComponentId>>>,
    data_renderer: &mut DataRendererSystem,
) {
    rebuild_world_panel_scene_model(world_panel_scene_model, world, installed_editor_roots);

    let Some(world_panel_root) = world.find_component(panel_query_root, WORLD_PANEL_ROOT_SELECTOR)
    else {
        return;
    };
    let Some(content_slot) = world.find_component(world_panel_root, PANEL_CONTENT_SLOT_SELECTOR)
    else {
        return;
    };
    let Some(selection_root) =
        world.find_component(world_panel_root, WORLD_PANEL_SELECTION_SELECTOR)
    else {
        return;
    };

    let world_model = build_world_panel_model(
        world,
        editor_context,
        &world_panel_scene_model
            .lock()
            .expect("world panel scene model mutex poisoned"),
    );
    rerender_world_panel_content(
        world,
        emit,
        content_slot,
        selection_root,
        &world_model.rows,
        world_model.selected_index,
        data_renderer,
    );
}

fn spawn_default_grid_for_editor(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    editor_context: &EditorContextState,
) -> ComponentId {
    let index = GridSystem::new()
        .enumerate_grids_for_editor(world, editor_root)
        .len()
        + 1;
    let grid_component = crate::engine::ecs::component::GridComponent::default();
    let visual_scale_x = grid_component.size_x as f32 * grid_component.spacing;
    let visual_scale_z = grid_component.size_z as f32 * grid_component.spacing;
    let mut owner_transform_component = TransformComponent::new();
    let live_cursor_pose = editor_context.selected_component.and_then(|selected| {
        TransformSystem::world_model(world, selected).map(|world_model| {
            (
                [world_model[3][0], world_model[3][1], world_model[3][2]],
                mat_to_quat(world_model),
            )
        })
    });
    if let Some(translation) = live_cursor_pose
        .map(|(translation, _)| translation)
        .or(editor_context.cursor_translation)
    {
        owner_transform_component = owner_transform_component.with_position(
            translation[0],
            translation[1],
            translation[2],
        );
    }
    if let Some(rotation) = live_cursor_pose
        .map(|(_, rotation)| rotation)
        .or(editor_context.cursor_rotation)
    {
        owner_transform_component = owner_transform_component.with_rotation_quat(rotation);
    }
    let owner_transform = world.add_component_boxed_named(
        &format!("grid_{index}"),
        Box::new(owner_transform_component),
    );
    let grid = world.add_component_boxed_named(
        &format!("grid_{index}_component"),
        Box::new(grid_component),
    );
    let visual_root = world.add_component_boxed_named(
        "grid_visual",
        Box::new(TransformComponent::new()),
    );
    let visual_selectable =
        world.add_component_boxed_named("grid_visual_selectable", Box::new(SelectableComponent::off()));
    let visual_serialize =
        world.add_component_boxed_named("grid_visual_serialize", Box::new(SerializeComponent::off()));
    let visual_shape = world.add_component_boxed_named(
        "grid_visual_shape",
        Box::new(
            TransformComponent::new()
                .with_position(0.0, 0.005, 0.0)
                .with_scale(visual_scale_x, 0.0025, visual_scale_z),
        ),
    );
    let visual_renderable = world.add_component_boxed_named(
        "grid_visual_renderable",
        Box::new(RenderableComponent::from_cpu_mesh_handle(
            crate::engine::graphics::primitives::CpuMeshHandle::CUBE,
            crate::engine::graphics::primitives::MaterialHandle::GRID_MESH,
        )),
    );
    let visual_color = world.add_component_boxed_named(
        "grid_visual_color",
        Box::new(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0)),
    );
    let visual_opacity = world.add_component_boxed_named(
        "grid_visual_opacity",
        Box::new(OpacityComponent::new().with_opacity(1.0)),
    );
    let _ = world.add_child(editor_root, owner_transform);
    let _ = world.add_child(owner_transform, grid);
    let _ = world.add_child(owner_transform, visual_root);
    let _ = world.add_child(visual_root, visual_selectable);
    let _ = world.add_child(visual_root, visual_serialize);
    let _ = world.add_child(visual_root, visual_shape);
    let _ = world.add_child(visual_shape, visual_renderable);
    let _ = world.add_child(visual_renderable, visual_color);
    let _ = world.add_child(visual_renderable, visual_opacity);
    world.init_component_tree(owner_transform, emit);
    emit.push_intent_now(
        owner_transform,
        IntentValue::RegisterTransform {
            component_ids: vec![owner_transform, visual_root, visual_shape],
        },
    );
    emit.push_intent_now(
        visual_renderable,
        IntentValue::RegisterRenderable {
            component_ids: vec![visual_renderable],
        },
    );
    owner_transform
}

fn toggle_grid_visibility(world: &mut World, owner_transform: ComponentId) {
    let grids = GridSystem::new();
    let Some(grid_entry) = grids.grid_owned_by_transform(world, owner_transform) else {
        return;
    };
    if let Some(grid) =
        world.get_component_by_id_as_mut::<crate::engine::ecs::component::GridComponent>(
            grid_entry.grid_component,
        )
    {
        grid.enabled = !grid.enabled;
    }
}

fn trace_suspicious_inspector_target(world: &World, target: Option<ComponentId>) {
    let Some(target) = target else {
        return;
    };
    let Some(name) = world
        .component_label(target)
        .or_else(|| world.component_name(target))
    else {
        return;
    };
    if [
        "item_",
        "_text",
        "_option",
        "selection_highlight",
        "editor_runtime_ui_root",
        "editor_gizmo_anchor",
        "editor_transform_gizmo",
    ]
    .iter()
    .any(|pattern| name.contains(pattern))
    {
        println!(
            "[InspectorSystem][trace] suspicious inspector target target={target:?} name={name:?}"
        );
    }
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

    if let Some(panel_id) = clicked_inspector_panel_instance_id(world, renderable)
        && let Some(panel_root) =
            find_inspector_panel_instance_root(world, panel_query_root, panel_id)
    {
        emit.push_intent_now(
            panel_layout_selection,
            IntentValue::SelectionSet {
                component_ids: vec![panel_layout_selection],
                entries: vec![SelectionEntry {
                    index: None,
                    component: panel_root,
                }],
                primary: Some(panel_root),
            },
        );
        return;
    }

    for selector in [
        WORLD_PANEL_ROOT_SELECTOR,
        "#assets_root",
        PAINT_PANEL_ROOT_SELECTOR,
        GRID_PANEL_ROOT_SELECTOR,
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
                    component: panel_root,
                }],
                primary: Some(panel_root),
            },
        );
        return;
    }
}

fn nearest_editor_ancestor(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut current = Some(start);
    while let Some(component_id) = current {
        if world
            .get_component_by_id_as::<crate::engine::ecs::component::EditorComponent>(component_id)
            .is_some()
        {
            return Some(component_id);
        }
        current = world.parent_of(component_id);
    }
    None
}

fn nearest_transform_ancestor(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut current = Some(start);
    while let Some(component_id) = current {
        if world
            .get_component_by_id_as::<TransformComponent>(component_id)
            .is_some()
        {
            return Some(component_id);
        }
        current = world.parent_of(component_id);
    }
    None
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

fn grid_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/panels.mms")
}

fn editor_settings_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/panels.mms")
}

fn inspector_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/panels.mms")
}

fn build_panel_component_expr(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    asset_path: &'static str,
    export_name: &str,
    args: Vec<Value>,
    panel_kind: PanelKind,
    panel_kind_label: &str,
) -> Option<MaterializedCE> {
        build_panel_shell_component_expr(
            world,
            emit,
            &PanelShellSpec {
                panel_kind,
            asset_path: asset_path.to_string(),
                export_name: export_name.to_string(),
                args,
                root_selector: String::new(),
                slot_selectors: HashMap::new(),
                control_selectors: HashMap::new(),
            },
        )
    .map_err(|error| {
        eprintln!("[InspectorSystemStopgapMmsAdapter] {panel_kind_label} render error: {error}");
    })
    .ok()
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

fn spawn_inspector_panel_instance_tree(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    model: &InspectorPanelModel,
    _index: usize,
    data_renderer: &mut DataRendererSystem,
) -> ComponentId {
    let title_color = if model.active {
        Value::Array(vec![
            Value::Number(0.96),
            Value::Number(1.0),
            Value::Number(0.98),
            Value::Number(1.0),
        ])
    } else {
        Value::Array(vec![
            Value::Number(0.84),
            Value::Number(0.90),
            Value::Number(0.86),
            Value::Number(1.0),
        ])
    };
    let panel_bg = if model.active {
        Value::Array(vec![
            Value::Number(0.18),
            Value::Number(0.78),
            Value::Number(0.22),
            Value::Number(0.95),
        ])
    } else {
        Value::Array(vec![
            Value::Number(0.20),
            Value::Number(0.52),
            Value::Number(0.24),
            Value::Number(0.90),
        ])
    };
    let item_bg = if model.pinned {
        Value::Array(vec![
            Value::Number(0.96),
            Value::Number(0.94),
            Value::Number(0.86),
            Value::Number(0.86),
        ])
    } else {
        Value::Array(vec![
            Value::Number(0.92),
            Value::Number(0.92),
            Value::Number(0.92),
            Value::Number(0.80),
        ])
    };

    let shell_spec = PanelShellSpec {
        panel_kind: PanelKind::Inspector,
        asset_path: inspector_panel_asset_path().to_string(),
        export_name: "inspector_panel".to_string(),
        args: vec![
            Value::String(model.title.clone()),
            Value::Array(Vec::new()),
            title_color,
            panel_bg,
            item_bg,
        ],
        root_selector: INSPECTOR_PANEL_ROOT_SELECTOR.to_string(),
        slot_selectors: HashMap::from([
            (
                PanelSlotKind::Sidebar,
                INSPECTOR_PANEL_SIDEBAR_SLOT_SELECTOR.to_string(),
            ),
            (
                PanelSlotKind::Detail,
                INSPECTOR_PANEL_DETAIL_SLOT_SELECTOR.to_string(),
            ),
            (
                PanelSlotKind::Toolbar,
                INSPECTOR_PANEL_PIN_SLOT_SELECTOR.to_string(),
            ),
        ]),
        control_selectors: HashMap::from([
            (
                PanelControlKind::Selection,
                INSPECTOR_PANEL_SELECTION_SELECTOR.to_string(),
            ),
            (
                PanelControlKind::TitleLabel,
                "#title_label".to_string(),
            ),
        ]),
    };
    let spawned = match spawn_panel_instance(
        world,
        emit,
        &shell_spec,
        Some(model.panel_id),
        PANEL_LAYOUT_GAP_GU,
    ) {
        Ok(spawned) => spawned,
        Err(error) => {
            eprintln!("[InspectorSystemStopgapMmsAdapter] inspector instance spawn error: {error}");
            return spawn_inspector_panel_instance_fallback_root(world, model.panel_id);
        }
    };
    attach_inspector_panel_instance_id(world, spawned.mount_root, model.panel_id);
    let instance = spawned.instance;
    let inspector_panel_root = instance.root;
    let Some(sidebar_slot) = instance.slots.get(&PanelSlotKind::Sidebar).copied() else {
        return inspector_panel_root;
    };
    let Some(detail_slot) = instance.slots.get(&PanelSlotKind::Detail).copied() else {
        return inspector_panel_root;
    };
    let selection_root = instance
        .controls
        .get(&PanelControlKind::Selection)
        .copied();
    rerender_single_inspector_panel_sidebar(
        world,
        emit,
        inspector_panel_root,
        model.panel_id,
        sidebar_slot,
        selection_root,
        &model.rows,
        data_renderer,
    );
    rerender_single_inspector_panel_detail(
        world,
        emit,
        inspector_panel_root,
        detail_slot,
        &model.detail,
        data_renderer,
    );
    inspector_panel_root
}

fn attach_inspector_panel_instance_id(
    world: &mut World,
    instance_root: ComponentId,
    panel_id: InspectorPanelId,
) {
    let data = world.add_component_boxed_named(
        INSPECTOR_PANEL_INSTANCE_DATA_NAME,
        Box::new(
            DataComponent::new()
                .with_entry(
                    INSPECTOR_PANEL_INSTANCE_ID_KEY,
                    DataValue::Integer(panel_id as i64),
                )
                .with_entry(
                    "instance_name",
                    DataValue::Text(format!("{INSPECTOR_PANEL_INSTANCE_PREFIX}{panel_id}")),
                ),
        ),
    );
    let _ = world.add_child(instance_root, data);
}

fn spawn_inspector_panel_instance_fallback_root(
    world: &mut World,
    panel_id: InspectorPanelId,
) -> ComponentId {
    let root = world
        .add_component_boxed_named("inspector_panel_root", Box::new(TransformComponent::new()));
    attach_inspector_panel_instance_id(world, root, panel_id);
    root
}

fn set_inspector_pin_button_state(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    pin_button: ComponentId,
    pinned: bool,
) {
    if let Some(text) = world.find_component(pin_button, "#pin_button_text") {
        emit.push_intent_now(
            text,
            IntentValue::SetText {
                component_ids: vec![text],
                text: if pinned { "Unpin" } else { "Pin" }.to_string(),
            },
        );
    }

    if let Some(style_id) = world.children_of(pin_button).iter().copied().find(|&child| {
        world
            .get_component_by_id_as::<StyleComponent>(child)
            .is_some()
    }) && let Some(style) = world.get_component_by_id_as_mut::<StyleComponent>(style_id)
    {
        style.background_color = Some(if pinned {
            [0.95, 0.82, 0.18, 1.0]
        } else {
            [0.10, 0.55, 0.18, 1.0]
        });
        style.color = Some(if pinned {
            [0.10, 0.12, 0.06, 1.0]
        } else {
            [0.75, 1.00, 0.45, 1.0]
        });
    }
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
    if let Some(selection_component) =
        world.get_component_by_id_as_mut::<SelectionComponent>(selection)
    {
        selection_component.payload_selector = Some(format!("[name='{WORLD_PANEL_PAYLOAD_NAME}']"));
    }
    let _ = world.add_child(rows_mount, selection);

    for (index, row) in rows.iter().enumerate() {
        let row_root = spawn_world_panel_row_tree(
            world,
            &format!("{ITEM_PREFIX}{index}"),
            row,
            index,
            selected_index == Some(index as i64),
        );
        let _ = world.add_child(rows_mount, row_root);
    }

    if let Some(index) = selected_index.and_then(|index| usize::try_from(index).ok())
        && let Some(_) = rows.get(index)
        && let Some(row_root) =
            world.find_component(content_root, &format!("#{ITEM_PREFIX}{index}"))
    {
        let selected_payload = resolve_selected_world_panel_payload(world, row_root);
        let Some(selection) = world.get_component_by_id_as_mut::<SelectionComponent>(selection)
        else {
            return content_root;
        };
        selection.select_entry(SelectionEntry {
            index: Some(index),
            component: row_root,
        });
        selection.selected_payload = selected_payload;
    }

    content_root
}

fn spawn_world_panel_row_tree(
    world: &mut World,
    row_name: &str,
    row: &WorldPanelRow,
    _row_index: usize,
    selected: bool,
) -> ComponentId {
    match row.kind {
        WorldPanelRowKind::Spacer => spawn_panel_ui_row_tree(
            world,
            PanelUiRowSpec {
                row_name,
                payload_name: WORLD_PANEL_PAYLOAD_NAME,
                target_component: None,
                label: "",
                row_kind_label: "Spacer",
                interactive: false,
                background_rgba: [0.0, 0.0, 0.0, 0.0],
                text_rgba: [0.0, 0.0, 0.0, 0.0],
                font_size_gu: None,
                spacer_height_gu: Some(0.8),
            },
        ),
        WorldPanelRowKind::EditorRoot | WorldPanelRowKind::Info | WorldPanelRowKind::Component => {
            let (background_rgba, text_rgba, interactive, row_kind_label) = match row.kind {
                WorldPanelRowKind::EditorRoot => (
                    [0.30, 0.84, 0.38, 0.98],
                    [0.03, 0.08, 0.04, 1.0],
                    true,
                    "EditorRoot",
                ),
                WorldPanelRowKind::Info => {
                    ([0.85, 0.85, 0.85, 1.0], [0.0, 0.0, 0.0, 1.0], false, "Info")
                }
                WorldPanelRowKind::Component if selected => (
                    [1.00, 0.88, 0.20, 0.96],
                    [0.06, 0.09, 0.08, 1.0],
                    true,
                    "Component",
                ),
                WorldPanelRowKind::Component => (
                    [0.92, 0.97, 0.92, 1.0],
                    [0.06, 0.09, 0.08, 1.0],
                    true,
                    "Component",
                ),
                WorldPanelRowKind::Spacer => unreachable!(),
            };
            spawn_panel_ui_row_tree(
                world,
                PanelUiRowSpec {
                    row_name,
                    payload_name: WORLD_PANEL_PAYLOAD_NAME,
                    target_component: row.target_component,
                    label: &row.display_label,
                    row_kind_label,
                    interactive,
                    background_rgba,
                    text_rgba,
                    font_size_gu: None,
                    spacer_height_gu: None,
                },
            )
        }
    }
}
