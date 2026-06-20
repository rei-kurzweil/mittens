use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::engine::ecs::component::{
    DataComponent, DataValue, EditorComponent, SelectionComponent, SelectionEntry, SelectionMode,
    StyleComponent,
};
use crate::engine::ecs::component::{EditorInteractionMode, TransformComponent};
use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::system::data_renderer_system::{
    DataRendererSystem, UiDetailItem, UiItem, UiItemKind,
};
use crate::engine::ecs::system::editor::context::{
    EditorContextState, apply_editor_root_selection, apply_semantic_target_selection,
};
use crate::engine::ecs::system::editor::grid_panel::{
    GRID_PANEL_ADD_BUTTON_SELECTOR, GRID_PANEL_DELETE_PAYLOAD_NAME, GRID_PANEL_ENABLED_PAYLOAD_NAME,
    GRID_PANEL_ROOT_SELECTOR, GRID_PANEL_ROW_PAYLOAD_NAME, GRID_PANEL_ROW_SPEC,
    GRID_PANEL_VISIBILITY_PAYLOAD_NAME, build_grid_panel_model, grid_panel_items,
};
use crate::engine::ecs::system::editor::inspector_panel::{
    INSPECTOR_DETAIL_SPEC, INSPECTOR_ITEM_PREFIX, INSPECTOR_PANEL_INSTANCE_ID_KEY,
    INSPECTOR_PANEL_PAYLOAD_NAME, INSPECTOR_ROW_SPEC, InspectorPanelDetailModel, InspectorPanelId,
    InspectorPanelModel, InspectorPanelRow, InspectorPanelRowKind, InspectorWorkspaceEvent,
    InspectorWorkspaceState, build_inspector_panel_models, clear_missing_inspector_targets,
    inspector_panel_instance_id_on_root, reduce_inspector_workspace_state,
};
use crate::engine::ecs::system::editor::panel_ui::{
    PanelUiRowSpec, spawn_block_container, spawn_panel_ui_row_tree,
    spawn_panel_ui_section_header_tree,
};
use crate::engine::ecs::system::editor::settings_panel::{
    EDITOR_SETTINGS_ARMATURE_CHECKMARK_SLOT_NAME, EDITOR_SETTINGS_ARMATURE_ROW_NAME,
    EDITOR_SETTINGS_PANEL_ROOT_SELECTOR, EDITOR_SETTINGS_PAYLOAD_NAME,
    EDITOR_SETTINGS_SELECTION_SELECTOR, EditorSettingsOption,
};
use crate::engine::ecs::system::editor::workspace::EditorWorkspaceRuntime;
use crate::engine::ecs::system::editor::world_panel::{
    AuthoredWorldPanelSceneModel, ITEM_PREFIX, WORLD_PANEL_PAYLOAD_NAME,
    WORLD_PANEL_SELECTION_NAME, WorldPanelModel, WorldPanelRow, WorldPanelRowKind,
    build_world_panel_model, effective_editor_roots, editor_scene_roots, mark_nearest_layout_dirty, parse_item_index,
    rebuild_world_panel_scene_model, register_editor_root, rerender_world_panel_content,
    rerender_world_panel_status, sync_world_panel_selection, world_panel_item_label,
};
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
use crate::engine::ecs::system::grid_system::GridSpawnSpec;
use crate::engine::ecs::system::GridSystem;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, SignalKind, World};
use crate::meow_meow::component_registry::{
    filtered_root_ids_for_roots, filtered_roots_to_ce_ast, spawn_tree,
};
use crate::meow_meow::object::{CeChild, MaterializedCE, Value};
use crate::meow_meow::runner::MeowMeowRunner;
const WORLD_PANEL_ROOT_SELECTOR: &str = "#world_panel_root";
const PAINT_PANEL_ROOT_SELECTOR: &str = "#paint_panel_root";
const EDITOR_WORKSPACE_GRIDS_CHANGED: &str = "EditorWorkspaceGridsChanged";
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
const POSE_PANEL_TOTAL_HEIGHT_GU: f64 = 60.5;
const PANEL_LAYOUT_GAP_GU: f64 = 2.0;
const PANEL_ROOT_MARGIN_X_GU: f64 = 0.5;
const PANEL_ROOT_MARGIN_Y_GU: f64 = 0.5;
const PANEL_LAYOUT_AVAILABLE_WIDTH_GU: f64 = 200000.0;
const DISABLE_INSPECTOR_MOUNT_WRITES: bool = false;
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

fn editor_memory_marker(label: &str) {
    let _ = label;
}

fn editor_memory_marker_with_panel(label: &str, panel_export: &str) {
    editor_memory_marker(&format!("{label} export={panel_export}"));
}

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
        editor_memory_marker("editor setup_panels_for_editor:start");
        self.editor_context_state = Some(Arc::clone(&editor_context_state));
        let runtime_ui_root = self.workspace_runtime.get_or_create_runtime_ui_root(world);
        editor_memory_marker("editor setup_panels_for_editor:after runtime ui root");

        println!(
            "[InspectorSystem][debug] setup_panels_for_editor editor_root={editor_root:?} runtime_ui_root={runtime_ui_root:?} world_panel_pos={:?} inspector_panel_pos={:?}",
            world_panel_pos, inspector_panel_pos,
        );

        register_editor_root(self.workspace_runtime.installed_editor_roots(), editor_root);
        editor_memory_marker("editor setup_panels_for_editor:after register_editor_root");
        GridSystem::new().ensure_default_grid(world, emit, editor_root);
        editor_memory_marker("editor setup_panels_for_editor:after ensure_default_grid");
        rebuild_world_panel_scene_model(
            &self.world_panel_scene_model,
            world,
            self.workspace_runtime.installed_editor_roots(),
        );
        editor_memory_marker("editor setup_panels_for_editor:after rebuild_world_panel_scene_model");

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
        editor_memory_marker("editor setup_panels_for_editor:after ensure_default_panel");
        let model = build_world_panel_model(
            world,
            &editor_context,
            &self
                .world_panel_scene_model
                .lock()
                .expect("world panel scene model mutex poisoned"),
        );
        editor_memory_marker("editor setup_panels_for_editor:after build_world_panel_model");
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
        editor_memory_marker("editor setup_panels_for_editor:after build_inspector_panel_models");

        {
            let working_file_path = self
                .working_file_path
                .lock()
                .expect("working file path mutex poisoned");
            editor_memory_marker("editor setup_panels_for_editor:before reconcile_panel_layout");
            self.reconciler.reconcile_panel_layout(
                world,
                render_assets,
                emit,
                self.workspace_runtime.panel_layout_spawned_mut(),
                runtime_ui_root,
                editor_root,
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
        editor_memory_marker("editor setup_panels_for_editor:after reconcile_panel_layout");

        editor_memory_marker("editor setup_panels_for_editor:before refresh_world_panels");
        self.refresh_world_panels(world, emit);
        editor_memory_marker("editor setup_panels_for_editor:after refresh_world_panels");

        self.install_shared_panel_handlers(rx, runtime_ui_root);
        editor_memory_marker("editor setup_panels_for_editor:after install_shared_panel_handlers");
        self.install_editor_refresh_handlers(rx, editor_root);
        editor_memory_marker("editor setup_panels_for_editor:end");
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
        let click_installed_editor_roots =
            Arc::clone(self.workspace_runtime.installed_editor_roots());
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
                if handle_editor_settings_panel_click(
                    world,
                    emit,
                    panel_query_root,
                    *renderable,
                    &click_editor_context_state,
                    &click_installed_editor_roots,
                ) {
                    return;
                }
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

                if handle_pose_panel_click(
                    world,
                    emit,
                    panel_query_root,
                    *renderable,
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
                let payload_child = world.children_of(row_root).iter().copied().find(|&child| {
                    world
                        .get_component_by_id_as::<DataComponent>(child)
                        .is_some_and(|data| data.get_component("target_component").is_some())
                });
                println!(
                    "[WorldPanel][trace] click row_name={row_name} row_root={row_root:?} row_index={row_index} payload_child={payload_child:?} selection_root={selection_root:?}"
                );
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

                println!(
                    "✨🫠🐈 [1/5] [InspectorPanel][SelectionChanged] sidebar selection_root={selection_root:?} panel_query_root={panel_query_root:?} inspector_panel_root={inspector_panel_root:?}",
                );

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

        let pose_data_renderer = Arc::clone(&self.data_renderer);
        rx.add_handler_closure(
            SignalKind::DataEvent,
            panel_query_root,
            move |world, emit, signal| {
                let Some(EventSignal::DataEvent { name, .. }) = signal.event.as_ref() else {
                    return;
                };

                if name == "pose_captured" {
                    rerender_pose_panel(
                        world,
                        emit,
                        panel_query_root,
                        &mut *pose_data_renderer
                            .lock()
                            .expect("data renderer mutex poisoned"),
                    );
                }
            },
        );

        let grid_data_renderer = Arc::clone(&self.data_renderer);
        rx.add_handler_closure(
            SignalKind::DataEvent,
            panel_query_root,
            move |world, emit, signal| {
                let Some(EventSignal::DataEvent { name, .. }) = signal.event.as_ref() else {
                    return;
                };

                if name == EDITOR_WORKSPACE_GRIDS_CHANGED {
                    let editor_context = EditorContextState::default();
                    rerender_grid_panel_from_context(
                        world,
                        emit,
                        panel_query_root,
                        &editor_context,
                        &mut *grid_data_renderer
                            .lock()
                            .expect("data renderer mutex poisoned"),
                    );
                }
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
        editor_memory_marker("editor refresh_world_panels:start");
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
        editor_memory_marker("editor refresh_world_panels:after build_world_panel_model");
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
        editor_memory_marker("editor refresh_world_panels:after rerender_world_panel_content");

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
        editor_memory_marker("editor refresh_world_panels:after rerender_grid_panel");

        rerender_pose_panel(
            world,
            emit,
            panel_query_root,
            &mut *self
                .data_renderer
                .lock()
                .expect("data renderer mutex poisoned"),
        );
        editor_memory_marker("editor refresh_world_panels:after rerender_pose_panel");

        sync_editor_settings_panel_selection(world, emit, panel_query_root, &editor_context);
        editor_memory_marker("editor refresh_world_panels:after sync_editor_settings_panel_selection");

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
        editor_memory_marker("editor refresh_world_panels:after build_inspector_panel_models");
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
        editor_memory_marker("editor refresh_world_panels:end");
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

    rerender_grid_panel_from_context(
        world,
        emit,
        panel_query_root,
        &editor_context,
        data_renderer,
    );

    rerender_pose_panel(world, emit, panel_query_root, data_renderer);

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
    let total_start = std::time::Instant::now();
    if DISABLE_INSPECTOR_MOUNT_WRITES {
        let _ = world;
        let _ = emit;
        let _ = panel_query_root;
        let _ = editor_context_state;
        let _ = world_panel_scene_model;
        let _ = inspector_workspace_state;
        let _ = rendered_inspector_models;
        let _ = data_renderer;
        return;
    }

    let editor_context = editor_context_state
        .lock()
        .expect("editor context state mutex poisoned")
        .clone();
    println!(
        "[InspectorSystem][trace] rebuild inspector target={:?} active_editor={:?}",
        editor_context.selected_component, editor_context.active_editor
    );
    println!(
        "✨🫠🐈 [2/5] [InspectorPanel][Refresh] selected_component={:?} active_editor={:?} panel_query_root={panel_query_root:?}",
        editor_context.selected_component, editor_context.active_editor,
    );
    trace_suspicious_inspector_target(world, editor_context.selected_component);

    {
        let sync_start = std::time::Instant::now();
        let mut workspace = inspector_workspace_state
            .lock()
            .expect("inspector workspace mutex poisoned");
        sync_inspector_workspace_to_selection(world, &editor_context, &mut workspace);
        println!(
            "[InspectorPanel][perf] sync_inspector_workspace_to_selection took {:?}",
            sync_start.elapsed()
        );
    }

    let Some(bottom_row_root) = panel_layout_bottom_row_id(world, panel_query_root) else {
        return;
    };

    let build_models_start = std::time::Instant::now();
    let inspector_models = build_inspector_panel_models(
        world,
        &world_panel_scene_model
            .lock()
            .expect("world panel scene model mutex poisoned"),
        &inspector_workspace_state
            .lock()
            .expect("inspector workspace mutex poisoned"),
    );
    println!(
        "[InspectorPanel][perf] build_inspector_panel_models took {:?} count={}",
        build_models_start.elapsed(),
        inspector_models.len()
    );
    let rerender_start = std::time::Instant::now();
    rerender_inspector_panels(
        world,
        emit,
        bottom_row_root,
        &inspector_models,
        rendered_inspector_models,
        data_renderer,
    );
    println!(
        "[InspectorPanel][perf] sync_and_refresh_inspector_panels rerender took {:?}",
        rerender_start.elapsed()
    );
    println!(
        "[InspectorPanel][perf] sync_and_refresh_inspector_panels total took {:?}",
        total_start.elapsed()
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
    let selection_result = apply_semantic_target_selection(
        world,
        emit,
        editor_context_state,
        target_component,
        true,
    );
    let active_editor = selection_result.active_editor;
    let is_editor_root_target = active_editor == Some(target_component);
    let gizmo_target = selection_result.gizmo_target;

    println!(
        "[InspectorSystem][trace] world_panel selection_root={selection_root:?} clicked_row={:?} payload={:?} authored_target={target_component:?} active_editor={active_editor:?} is_editor_root_target={is_editor_root_target} gizmo_target={gizmo_target:?} select_editor_target_ran={}",
        selected_component,
        selected_payload,
        selection_result.used_editor_selection_path
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
        editor_root: ComponentId,
        world_panel_pos: (f32, f32, f32),
        inspector_panel_pos: (f32, f32, f32),
        model: &WorldPanelModel,
        inspector_models: &[InspectorPanelModel],
        rendered_inspector_models: &Arc<Mutex<Vec<InspectorPanelModel>>>,
        working_file_path: &Path,
        asset_system: &crate::engine::ecs::system::AssetSystem,
        data_renderer: &mut DataRendererSystem,
    ) {
        editor_memory_marker("editor reconcile_panel_layout:start");
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
                editor_memory_marker("editor reconcile_panel_layout:skip already spawned");
                return;
            }
        }

        if existing_world_panel.is_some() && existing_inspector_panel.is_some() {
            println!(
                "[InspectorSystem][debug] panel layout already present for panel_query_root={panel_query_root:?}; skipping spawn"
            );
            *panel_layout_spawned = true;
            editor_memory_marker("editor reconcile_panel_layout:skip already present");
            return;
        }

        if existing_panel_mount.is_some() {
            println!(
                "[InspectorSystem][debug] pending panel layout mount already exists for panel_query_root={panel_query_root:?}; skipping duplicate spawn"
            );
            *panel_layout_spawned = true;
            editor_memory_marker("editor reconcile_panel_layout:skip pending mount");
            return;
        }

        *panel_layout_spawned = true;

        self.spawn_panel_layout(
            world,
            render_assets,
            emit,
            panel_query_root,
            editor_root,
            world_panel_pos,
            inspector_panel_pos,
            model,
            inspector_models,
            rendered_inspector_models,
            working_file_path,
            asset_system,
            data_renderer,
        );
        editor_memory_marker("editor reconcile_panel_layout:end");
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
        editor_root: ComponentId,
        world_panel_pos: (f32, f32, f32),
        inspector_panel_pos: (f32, f32, f32),
        model: &WorldPanelModel,
        inspector_models: &[InspectorPanelModel],
        rendered_inspector_models: &Arc<Mutex<Vec<InspectorPanelModel>>>,
        working_file_path: &Path,
        asset_system: &crate::engine::ecs::system::AssetSystem,
        data_renderer: &mut DataRendererSystem,
    ) {
        editor_memory_marker("editor spawn_panel_layout:start");
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
        editor_memory_marker("editor spawn_panel_layout:after world_panel expr");

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
        editor_memory_marker("editor spawn_panel_layout:after asset_panel expr");

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
        editor_memory_marker("editor spawn_panel_layout:after paint_panel expr");

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
        editor_memory_marker("editor spawn_panel_layout:after grid_panel expr");

        let pose_panel = match build_panel_component_expr(
            world,
            emit,
            pose_panel_asset_path(),
            "pose_capture_panel",
            vec![
                Value::String("Poses".to_string()),
                world_panel_title_color.clone(),
                world_panel_bg.clone(),
            ],
            PanelKind::Pose,
            "pose capture panel",
        ) {
            Some(panel) => panel,
            None => return,
        };
        editor_memory_marker("editor spawn_panel_layout:after pose_panel expr");

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
        editor_memory_marker("editor spawn_panel_layout:after editor_settings_panel expr");

        let _ = inspector_panel_pos;
        let anchor_pos = world_panel_pos;

        let total_height_gu = WORLD_PANEL_TOTAL_HEIGHT_GU
            .max(INSPECTOR_PANEL_TOTAL_HEIGHT_GU)
            .max(ASSET_PANEL_TOTAL_HEIGHT_GU)
            .max(PAINT_PANEL_TOTAL_HEIGHT_GU)
            .max(GRID_PANEL_TOTAL_HEIGHT_GU)
            .max(POSE_PANEL_TOTAL_HEIGHT_GU)
            .max(EDITOR_SETTINGS_PANEL_TOTAL_HEIGHT_GU)
            * 2.0
            + PANEL_LAYOUT_GAP_GU
            + (PANEL_ROOT_MARGIN_Y_GU * 2.0);

        let world_panel = decorate_panel_root_ce(world_panel, PANEL_LAYOUT_GAP_GU);
        let paint_panel = decorate_panel_root_ce(paint_panel, PANEL_LAYOUT_GAP_GU);
        let asset_panel = decorate_panel_root_ce(asset_panel, PANEL_LAYOUT_GAP_GU);
        let grid_panel = decorate_panel_root_ce(grid_panel, PANEL_LAYOUT_GAP_GU);
        let pose_panel = decorate_panel_root_ce(pose_panel, PANEL_LAYOUT_GAP_GU);
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
                    pose_panel,
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
        editor_memory_marker("editor spawn_panel_layout:after spawn_panel_layout_mount");

        // Add SelectionComponent to the LayoutRoot so we can select individual panels.
        let selection = ensure_panel_layout_selection(world, layout_root_id);
        world.init_component_tree(selection, emit);
        editor_memory_marker("editor spawn_panel_layout:after ensure_panel_layout_selection");

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
                        editor_memory_marker("editor spawn_panel_layout:before asset panel population");

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
                        editor_memory_marker("editor spawn_panel_layout:after asset panel population");
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
        let _ = inspector_models;
        let _ = rendered_inspector_models;

        let grid_context = EditorContextState {
            active_editor: Some(editor_root),
            ..EditorContextState::default()
        };
        rerender_grid_panel_from_context(
            world,
            emit,
            panel_mount_root,
            &grid_context,
            data_renderer,
        );
        editor_memory_marker("editor spawn_panel_layout:after rerender_grid_panel");
        sync_editor_settings_panel_selection(world, emit, panel_mount_root, &grid_context);
        editor_memory_marker("editor spawn_panel_layout:after sync_editor_settings_panel_selection");

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
        editor_memory_marker("editor spawn_panel_layout:end");
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
            apply_selection_set(
                world,
                emit,
                selection_root,
                vec![SelectionEntry {
                    index: Some(index),
                    component: row_root,
                }],
                Some(row_root),
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

    println!(
        "✨🫠🐈 [5/5] [InspectorPanel][DetailRender] detail_slot={detail_slot:?} detail={detail:?}",
    );
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
    println!(
        "✨🫠🐈 [4/5] [InspectorPanel][WorkspaceSync] editor_root={editor_root:?} selected_target={selected_target:?} active_panel={:?} before={:?} after={:?}",
        workspace.active_panel, workspace, next_workspace,
    );
    *workspace = next_workspace;
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
        ) && let Some(target_component) = action.target_component
        {
            println!(
                "✨🫠🐈 [3/5] [InspectorPanel][SidebarSelect] panel_id={} renderable={renderable:?} target_component={target_component:?} rerender_needed_pre={rerender_needed}",
                panel_id,
            );
            let next_workspace = reduce_inspector_workspace_state(
                &workspace,
                &InspectorWorkspaceEvent::SidebarRowFocused {
                    panel_id,
                    component: target_component,
                },
            );
            rerender_needed |= next_workspace != *workspace;
            *workspace = next_workspace;
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
        let _owner_transform = GridSystem::new().spawn_grid_for_editor(
            world,
            emit,
            editor_root,
            GridSpawnSpec::from_cursor_pose(
                editor_context.cursor_translation,
                editor_context.cursor_rotation,
                false,
            ),
        );
        emit.push_event(
            panel_query_root,
            EventSignal::DataEvent {
                name: EDITOR_WORKSPACE_GRIDS_CHANGED.to_string(),
                payload: Some(editor_root),
            },
        );
        rerender_grid_panel_from_context(
            world,
            emit,
            panel_query_root,
            &EditorContextState {
                active_editor: Some(editor_root),
                ..editor_context.clone()
            },
            data_renderer,
        );
        return true;
    }

    if let Some(action) = decode_panel_action_payload(
        world,
        renderable,
        GRID_PANEL_VISIBILITY_PAYLOAD_NAME,
        PanelKind::Grid,
        PanelActionKind::Toggle,
        None,
        None,
    ) && let Some(owner_transform) = action.target_component
    {
        let _ = GridSystem::new().toggle_grid_hidden(world, emit, owner_transform);
        emit.push_event(
            panel_query_root,
            EventSignal::DataEvent {
                name: EDITOR_WORKSPACE_GRIDS_CHANGED.to_string(),
                payload: Some(owner_transform),
            },
        );
        rerender_grid_panel_from_context(
            world,
            emit,
            panel_query_root,
            &editor_context,
            data_renderer,
        );
        return true;
    }

    if let Some(action) = decode_panel_action_payload(
        world,
        renderable,
        GRID_PANEL_ENABLED_PAYLOAD_NAME,
        PanelKind::Grid,
        PanelActionKind::Toggle,
        None,
        None,
    ) && let Some(owner_transform) = action.target_component
    {
        let _ = GridSystem::new().toggle_grid_enabled(world, emit, owner_transform);
        emit.push_event(
            panel_query_root,
            EventSignal::DataEvent {
                name: EDITOR_WORKSPACE_GRIDS_CHANGED.to_string(),
                payload: Some(owner_transform),
            },
        );
        rerender_grid_panel_from_context(
            world,
            emit,
            panel_query_root,
            &editor_context,
            data_renderer,
        );
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
        if editor_context.selected_component == Some(owner_transform) {
            apply_editor_root_selection(world, editor_context_state, editor_root);
        }
        let _ = GridSystem::new().delete_grid(world, emit, owner_transform);
        emit.push_event(
            panel_query_root,
            EventSignal::DataEvent {
                name: EDITOR_WORKSPACE_GRIDS_CHANGED.to_string(),
                payload: Some(editor_root),
            },
        );
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
        let _ = editor_root;
        let _selection_result = apply_semantic_target_selection(
            world,
            emit,
            editor_context_state,
            owner_transform,
            true,
        );
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

fn handle_editor_settings_panel_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    renderable: ComponentId,
    editor_context_state: &Arc<Mutex<EditorContextState>>,
    installed_editor_roots: &Arc<Mutex<Vec<ComponentId>>>,
) -> bool {
    let Some(settings_panel_root) =
        world.find_component(panel_query_root, EDITOR_SETTINGS_PANEL_ROOT_SELECTOR)
    else {
        return false;
    };
    if !is_descendant_or_self(world, settings_panel_root, renderable) {
        return false;
    }

    let mut current = Some(renderable);
    while let Some(component_id) = current {
        let Some(payload_id) = world.children_of(component_id).iter().copied().find(|&child| {
            world.component_label(child) == Some(EDITOR_SETTINGS_PAYLOAD_NAME)
        }) else {
            current = world.parent_of(component_id);
            continue;
        };

        let Some(payload) = world.get_component_by_id_as::<DataComponent>(payload_id) else {
            return true;
        };
        let row_kind = data_text(payload, "row_kind").unwrap_or_default();
        if row_kind != "GLTFArmatureVisibility" {
            return false;
        }

        let visible = !editor_context_state
            .lock()
            .expect("editor context state mutex poisoned")
            .armature_visible;

        {
            let mut editor_context = editor_context_state
                .lock()
                .expect("editor context state mutex poisoned");
            editor_context.armature_visible = visible;
        }

        let editor_roots = effective_editor_roots(world, installed_editor_roots);
        for editor_root in editor_roots {
            let gltf_components = find_gltf_components_under(world, editor_root);
            for gltf_component in gltf_components {
                emit.push_intent_now(
                    gltf_component,
                    IntentValue::GLTFArmatureVisible {
                        component_ids: vec![gltf_component],
                        visible,
                    },
                );
            }
        }

        let editor_context = editor_context_state
            .lock()
            .expect("editor context state mutex poisoned")
            .clone();
        sync_editor_settings_panel_selection(world, emit, panel_query_root, &editor_context);
        return true;
    }

    true
}

fn find_gltf_components_under(world: &World, root: ComponentId) -> Vec<ComponentId> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(component_id) = stack.pop() {
        if world
            .get_component_by_id_as::<crate::engine::ecs::component::GLTFComponent>(component_id)
            .is_some()
        {
            out.push(component_id);
        }
        for &child in world.children_of(component_id) {
            stack.push(child);
        }
    }
    out
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
    let Some(editor_root) = resolve_grid_panel_editor_root(world, editor_context) else {
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

fn resolve_grid_panel_editor_root(
    world: &World,
    editor_context: &EditorContextState,
) -> Option<ComponentId> {
    if editor_context.active_editor.is_some() {
        return editor_context.active_editor;
    }

    let grids = GridSystem::new();
    for component_id in world.all_components() {
        if world
            .get_component_by_id_as::<EditorComponent>(component_id)
            .is_some()
            && !grids
                .enumerate_grids_for_editor(world, component_id)
                .is_empty()
        {
            return Some(component_id);
        }
    }

    world.all_components().find(|&component_id| {
        world
            .get_component_by_id_as::<EditorComponent>(component_id)
            .is_some()
    })
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
        EditorInteractionMode::SelectAndCursor => EditorSettingsOption::SelectAndCursor,
    };
    let Some(panel_root) =
        world.find_component(panel_query_root, EDITOR_SETTINGS_PANEL_ROOT_SELECTOR)
    else {
        return;
    };
    let Some(row_root) =
        world.find_component(panel_root, &format!("#{}", desired_option.row_name()))
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
                EditorSettingsOption::SelectAndCursor => 2,
            }),
            component: row_root,
        }],
        Some(row_root),
    );

    sync_editor_settings_armature_checkmark(world, emit, panel_query_root, editor_context);
}

fn sync_editor_settings_armature_checkmark(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    editor_context: &EditorContextState,
) {
    let Some(settings_panel_root) =
        world.find_component(panel_query_root, EDITOR_SETTINGS_PANEL_ROOT_SELECTOR)
    else {
        return;
    };
    let Some(armature_row_root) =
        world.find_component(settings_panel_root, &format!("#{EDITOR_SETTINGS_ARMATURE_ROW_NAME}"))
    else {
        return;
    };
    let Some(checkmark_slot) = world.find_component(
        armature_row_root,
        &format!("#{EDITOR_SETTINGS_ARMATURE_CHECKMARK_SLOT_NAME}"),
    ) else {
        return;
    };

    let existing_children = world.children_of(checkmark_slot).to_vec();
    for child in existing_children {
        let _ = world.remove_component_subtree(child);
    }

    if !editor_context.armature_visible {
        return;
    }

    let Some(checkmark) = build_panel_component_expr(
        world,
        emit,
        icons_asset_path(),
        "checkmark_icon",
        vec![],
        PanelKind::Inspector,
        "editor settings checkmark",
    ) else {
        return;
    };
    let Ok(root) = spawn_tree(&checkmark, Some(checkmark_slot), world, emit) else {
        return;
    };
    let _ = root;
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

pub fn rerender_pose_panel(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_mount_root: ComponentId,
    data_renderer: &mut DataRendererSystem,
) {
    use crate::engine::ecs::system::editor::pose_panel::*;

    let Some(panel_root) = world.find_component(panel_mount_root, POSE_PANEL_ROOT_SELECTOR) else {
        return;
    };

    let Some(content_slot) = world.find_component(panel_root, "#content_area") else {
        return;
    };

    // Clear content
    let children = world.children_of(content_slot).to_vec();
    for ch in children {
        let _ = world.remove_component_subtree(ch);
    }

    let model = build_pose_panel_model(world);

    for section in model.sections {
        let header =
            spawn_panel_ui_section_header_tree(world, "pose_section_header", &section.label);
        let _ = world.add_child(content_slot, header);

        for row in section.poses {
            let row_spec = PanelUiRowSpec {
                row_name: "pose_row",
                payload_name: POSE_PANEL_PAYLOAD_NAME,
                target_component: Some(row.pose),
                label: &row.label,
                row_kind_label: "PoseRow",
                interactive: true,
                background_rgba: [0.92, 0.97, 0.92, 1.0],
                text_rgba: [0.0, 0.0, 0.0, 1.0],
                font_size_gu: None,
                spacer_height_gu: None,
            };
            let row_node = spawn_panel_ui_row_tree(world, row_spec);

            // Add extra payload for target
            if let Some(payload_id) = world
                .find_component(row_node, &format!("[name='{POSE_PANEL_PAYLOAD_NAME}']"))
            {
                if let Some(data) = world.get_component_by_id_as_mut::<DataComponent>(payload_id) {
                    data.insert("pose_target", DataValue::Component(row.target));
                }
            }

            let _ = world.add_child(content_slot, row_node);
        }

    }

    world.init_component_tree(content_slot, emit);
}

pub fn handle_pose_panel_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    clicked_node: ComponentId,
    data_renderer: &mut DataRendererSystem,
) -> bool {
    use crate::engine::ecs::system::editor::pose_panel::*;

    let Some(panel_root) = world.find_component(panel_query_root, POSE_PANEL_ROOT_SELECTOR) else {
        return false;
    };

    if !is_descendant_or_self(world, panel_root, clicked_node) {
        return false;
    }

    if let Some(capture_button) = world.find_component(panel_root, POSE_PANEL_CAPTURE_BUTTON_SELECTOR)
        && is_descendant_or_self(world, capture_button, clicked_node)
    {
        emit.push_intent_now(
            panel_root,
            IntentValue::PoseCapture {
                target: panel_root,
                pose_name: None,
            },
        );
        return true;
    }

    // Search up for a payload
    let mut current = Some(clicked_node);
    while let Some(curr_id) = current {
        if let Some(payload_id) = world
            .children_of(curr_id)
            .iter()
            .find(|&&child| world.component_label(child) == Some(POSE_PANEL_PAYLOAD_NAME))
        {
            if let Some(data) = world.get_component_by_id_as::<DataComponent>(*payload_id) {
                let row_kind = data_text(data, "row_kind").unwrap_or_default();
                match row_kind.as_str() {
                    "PoseRow" => {
                        let pose_id = data.get_component("target_component");
                        let target_id = data.get_component("pose_target");
                        if let (Some(pose), Some(target)) = (pose_id, target_id) {
                            emit.push_intent_now(target, IntentValue::PoseApply { target, pose });
                            return true;
                        }
                    }
                    "PoseAdd" => {
                        let target_id = data.get_component("target_component");
                        if let Some(target) = target_id {
                            emit.push_intent_now(target, IntentValue::PoseCapture {
                                target,
                                pose_name: None,
                            });
                            // Delay rerender slightly to allow system to process capture
                            return true;
                        }
                    }
                    _ => {}
                }
            }
        }
        current = world.parent_of(curr_id);
    }

    false
}

fn world_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/panels.mms")
}

fn icons_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/icons.mms")
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

fn pose_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/panels.mms")
}

fn data_text(data: &DataComponent, key: &str) -> Option<String> {
    match data.get(key) {
        Some(DataValue::Text(value)) => Some(value.clone()),
        _ => None,
    }
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
    editor_memory_marker_with_panel(
        "editor build_panel_component_expr:start",
        export_name,
    );
    let result = build_panel_shell_component_expr(
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
    });
    editor_memory_marker_with_panel(
        "editor build_panel_component_expr:after materialize",
        export_name,
    );
    result.ok()
}

fn build_placeholder_panel_component_expr(title_name: &'static str, title: &str) -> MaterializedCE {
    MaterializedCE {
        component_type: "T".to_string(),
        component_property_assignment_only: false,
        ctor_method: None,
        ctor_args: Vec::new(),
        calls: Vec::new(),
        named: vec![("name".to_string(), Value::String(title_name.to_string()))],
        positionals: Vec::new(),
        children: vec![CeChild::Spawn(MaterializedCE {
            component_type: "T".to_string(),
            component_property_assignment_only: false,
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
                component_property_assignment_only: false,
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
            (PanelControlKind::TitleLabel, "#title_label".to_string()),
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
    let selection_root = instance.controls.get(&PanelControlKind::Selection).copied();
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

    if let Some(style_id) = world
        .children_of(pin_button)
        .iter()
        .copied()
        .find(|&child| {
            world
                .get_component_by_id_as::<StyleComponent>(child)
                .is_some()
        })
        && let Some(style) = world.get_component_by_id_as_mut::<StyleComponent>(style_id)
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
        let Some(selection) = world.get_component_by_id_as_mut::<SelectionComponent>(selection)
        else {
            return content_root;
        };
        selection.select_entry(SelectionEntry {
            index: Some(index),
            component: row_root,
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::command_queue::CommandQueue;
    use crate::engine::ecs::component::{
        DataComponent, DataValue, EditorComponent, GLTFComponent, SelectionComponent,
        TransformComponent,
    };
    use crate::engine::ecs::system::SystemWorld;
    use crate::engine::graphics::{RenderAssets, VisualWorld};

    #[test]
    fn world_panel_editor_root_target_does_not_attach_gizmo() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let panel_query_root =
            world.add_component_boxed_named("panel_root", Box::new(TransformComponent::new()));
        let selection_root = world.add_component_boxed_named(
            WORLD_PANEL_SELECTION_NAME,
            Box::new(SelectionComponent::new()),
        );
        let row_root =
            world.add_component_boxed_named("item_0", Box::new(TransformComponent::new()));
        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let payload = world.add_component_boxed_named(
            WORLD_PANEL_PAYLOAD_NAME,
            Box::new(
                DataComponent::new()
                    .with_entry("target_component", DataValue::Component(editor_root)),
            ),
        );

        let _ = world.add_child(panel_query_root, selection_root);
        let _ = world.add_child(panel_query_root, row_root);
        let _ = world.add_child(row_root, payload);

        if let Some(selection) =
            world.get_component_by_id_as_mut::<SelectionComponent>(selection_root)
        {
            selection.selected_component = Some(row_root);
            selection.selected_payload = Some(payload);
        }

        let editor_context_state = Arc::new(Mutex::new(EditorContextState::default()));
        let world_panel_scene_model = Arc::new(Mutex::new(AuthoredWorldPanelSceneModel::default()));
        let inspector_workspace_state = Arc::new(Mutex::new(InspectorWorkspaceState::default()));
        let rendered_inspector_models = Arc::new(Mutex::new(Vec::new()));
        let mut data_renderer = DataRendererSystem::new();

        apply_world_panel_semantic_selection(
            &mut world,
            &mut emit,
            panel_query_root,
            &editor_context_state,
            &world_panel_scene_model,
            &inspector_workspace_state,
            &rendered_inspector_models,
            selection_root,
            &mut data_renderer,
        );

        assert!(
            world
                .find_component(editor_root, "#editor_transform_gizmo")
                .is_none(),
            "editor-root semantic selection should not spawn or attach gizmo",
        );
        assert_eq!(
            world
                .get_component_by_id_as::<EditorComponent>(editor_root)
                .and_then(|editor| editor.selected),
            None,
        );
        assert_eq!(
            editor_context_state
                .lock()
                .expect("editor context mutex poisoned")
                .selected_component,
            Some(editor_root),
        );
    }

    #[test]
    fn armature_settings_click_toggles_state_renders_checkmark_and_fans_out_to_all_editors() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let render_assets = RenderAssets::new();
        let mut systems = SystemWorld::default();

        let panel_query_root =
            world.add_component_boxed_named("panel_root", Box::new(TransformComponent::new()));
        let settings_panel_root = world.add_component_boxed_named(
            "editor_settings_panel_root",
            Box::new(TransformComponent::new()),
        );
        let armature_row = world.add_component_boxed_named(
            EDITOR_SETTINGS_ARMATURE_ROW_NAME,
            Box::new(TransformComponent::new()),
        );
        let checkmark_slot = world.add_component_boxed_named(
            EDITOR_SETTINGS_ARMATURE_CHECKMARK_SLOT_NAME,
            Box::new(TransformComponent::new()),
        );
        let payload = world.add_component_boxed_named(
            EDITOR_SETTINGS_PAYLOAD_NAME,
            Box::new(
                DataComponent::new()
                    .with_entry("row_kind", DataValue::Text("GLTFArmatureVisibility".into()))
                    .with_entry("visible", DataValue::Bool(false)),
            ),
        );
        let _ = world.add_child(panel_query_root, settings_panel_root);
        let _ = world.add_child(settings_panel_root, armature_row);
        let _ = world.add_child(armature_row, checkmark_slot);
        let _ = world.add_child(armature_row, payload);

        let editor_a =
            world.add_component_boxed_named("editor_a", Box::new(EditorComponent::new()));
        let editor_b =
            world.add_component_boxed_named("editor_b", Box::new(EditorComponent::new()));
        let gltf_a = world.add_component(GLTFComponent::new("a.glb"));
        let gltf_b = world.add_component(GLTFComponent::new("b.glb"));
        let _ = world.add_child(editor_a, gltf_a);
        let _ = world.add_child(editor_b, gltf_b);

        let editor_context_state = Arc::new(Mutex::new(EditorContextState::default()));
        let installed_editor_roots = Arc::new(Mutex::new(vec![editor_a, editor_b]));

        assert!(handle_editor_settings_panel_click(
            &mut world,
            &mut emit,
            panel_query_root,
            armature_row,
            &editor_context_state,
            &installed_editor_roots,
        ));

        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);
        let editor_context = editor_context_state
            .lock()
            .expect("editor context state mutex poisoned")
            .clone();
        assert!(editor_context.armature_visible);
        assert!(
            world
                .get_component_by_id_as::<GLTFComponent>(gltf_a)
                .expect("gltf_a")
                .armature_visible
        );
        assert!(
            world
                .get_component_by_id_as::<GLTFComponent>(gltf_b)
                .expect("gltf_b")
                .armature_visible
        );

        sync_editor_settings_armature_checkmark(
            &mut world,
            &mut emit,
            panel_query_root,
            &editor_context,
        );
        assert!(
            !world.children_of(checkmark_slot).is_empty(),
            "expected checkmark subtree to be rendered into slot"
        );
    }
}
