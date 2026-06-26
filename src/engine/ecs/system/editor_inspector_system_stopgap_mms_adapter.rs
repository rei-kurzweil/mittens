use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::engine::ecs::component::{
    SelectionComponent, SelectionEntry, SelectionMode, TransformComponent,
};
use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::system::GridSystem;
use crate::engine::ecs::system::data_renderer_system::DataRendererSystem;
use crate::engine::ecs::system::editor::context::EditorContextState;
use crate::engine::ecs::system::editor::grid_panel::{
    EDITOR_WORKSPACE_GRIDS_CHANGED, GRID_PANEL_ROOT_SELECTOR, GRID_PANEL_SELECTION_SELECTOR,
    GridPanelClickOutcome, handle_grid_panel_click, rerender_grid_panel_from_context,
};
use crate::engine::ecs::system::editor::inspector_panel::{
    INSPECTOR_DETAIL_SPEC, INSPECTOR_ITEM_PREFIX, INSPECTOR_PANEL_INSTANCE_ID_KEY,
    INSPECTOR_PANEL_PAYLOAD_NAME, INSPECTOR_ROW_SPEC, InspectorPanelDetailModel, InspectorPanelId,
    InspectorPanelModel, InspectorPanelRow, InspectorPanelRowKind, InspectorWorkspaceEvent,
    InspectorWorkspaceState, build_inspector_panel_models, clear_missing_inspector_targets,
    focus_panel_from_descendant_click, handle_inspector_panel_workspace_click,
    inspector_panel_instance_id_on_root, reduce_inspector_workspace_state,
    rerender_inspector_panels, sync_and_refresh_inspector_panels,
    world_panel_selection_matches_editor_context,
};
use crate::engine::ecs::system::editor::panel_ui::{
    PanelUiRowSpec, spawn_panel_ui_row_tree, spawn_panel_ui_section_header_tree,
};
use crate::engine::ecs::system::editor::pose_panel::{
    handle_pose_panel_click, rerender_pose_panel,
};
use crate::engine::ecs::system::editor::settings_panel::{
    EDITOR_SETTINGS_ARMATURE_TOGGLE_SLOT_NAME, EDITOR_SETTINGS_ARMATURE_ROW_NAME,
    EDITOR_SETTINGS_PANEL_ROOT_SELECTOR, EDITOR_SETTINGS_PAYLOAD_NAME,
    EDITOR_SETTINGS_SELECTION_SELECTOR, EditorSettingsOption, handle_editor_settings_panel_click,
    sync_editor_settings_armature_toggle, sync_editor_settings_panel_selection,
};
use crate::engine::ecs::system::editor::workspace::EditorWorkspaceRuntime;
use crate::engine::ecs::system::editor::world_panel::{
    AuthoredWorldPanelSceneModel, ITEM_PREFIX, PANEL_CONTENT_SLOT_SELECTOR,
    WORLD_PANEL_CONTENT_ROOT_SELECTOR, WORLD_PANEL_PAYLOAD_NAME, WORLD_PANEL_ROOT_SELECTOR,
    WORLD_PANEL_SELECTION_NAME, WORLD_PANEL_SELECTION_SELECTOR, WorldPanelModel, WorldPanelRow,
    WorldPanelRowKind, apply_world_panel_semantic_selection, build_world_panel_model,
    effective_editor_roots, handle_panel_button_click, handle_world_panel_item_click,
    mark_nearest_layout_dirty, panel_status_text, rebuild_world_panel_scene_model,
    register_editor_root, rerender_world_panel_content, rerender_world_panel_status,
    spawn_world_panel_content_tree, spawn_world_panel_row_tree, sync_world_panel_selection,
    world_panel_scene_path,
};
use crate::engine::ecs::system::panel_system::{
    PANEL_LAYOUT_MOUNT_NAME, PANEL_LAYOUT_ROOT_NAME, PANEL_LAYOUT_SELECTION_NAME, PanelControlKind,
    PanelKind, PanelSlotKind, ensure_panel_layout_selection, is_descendant_or_self,
    panel_layout_root_id, panel_layout_selection_id, spawn_editor_panel_layout_tree,
};
use crate::engine::ecs::system::selection_system::{
    apply_selection_set, resolve_semantic_target_from_payload,
};
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, SignalKind, World};

const PAINT_PANEL_ROOT_SELECTOR: &str = "#paint_panel_root";
const EDITOR_SETTINGS_PANEL_WIDTH_GU: f64 = 16.0;
const EDITOR_SETTINGS_PANEL_TOTAL_HEIGHT_GU: f64 = 11.5;
const INSPECTOR_PANEL_ROOT_SELECTOR: &str = "#inspector_panel_root";
const INSPECTOR_PANEL_SELECTION_SELECTOR: &str = "#inspector_panel_selection";
// Removed: INSPECTOR_DETAIL_WORLD_PANEL_MOUNT_NAME, INSPECTOR_DETAIL_WORLD_LAYOUT_ROOT_NAME
const PANEL_STATUS_WRAP_SELECTOR: &str = "#save_status_wrap";
const PAINT_STATUS_WRAP_SELECTOR: &str = "#paint_status_wrap";
const PAINT_TOOL_SELECTION_SELECTOR: &str = "#paint_tool_selection";
const PANEL_PATH_INPUT_SELECTOR: &str = "#path_input";
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
        editor_memory_marker(
            "editor setup_panels_for_editor:after rebuild_world_panel_scene_model",
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

        if let Some(mount_root) = self.workspace_runtime.find_panel_mount_root(world) {
            self.workspace_runtime
                .resolve_and_cache_static_panels(world, editor_root, mount_root);
        }

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
        let click_world_panel_root = self
            .workspace_runtime
            .panel_instance(PanelKind::World)
            .map(|p| p.root);
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
                match handle_grid_panel_click(
                    world,
                    emit,
                    panel_query_root,
                    *renderable,
                    &click_editor_context_state,
                    &click_installed_editor_roots,
                    &mut *click_data_renderer
                        .lock()
                        .expect("data renderer mutex poisoned"),
                ) {
                    GridPanelClickOutcome::NotHandled => {}
                    GridPanelClickOutcome::Handled => return,
                    GridPanelClickOutcome::HandledNeedsFullRefresh(rebuild_world_panel) => {
                        refresh_all_panel_models(
                            world,
                            emit,
                            panel_query_root,
                            &click_editor_context_state,
                            &click_world_panel_scene_model,
                            &click_inspector_workspace_state,
                            &click_installed_editor_roots,
                            &click_rendered_inspector_models,
                            rebuild_world_panel,
                            &mut *click_data_renderer
                                .lock()
                                .expect("data renderer mutex poisoned"),
                        );
                        return;
                    }
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

                let Some(world_panel_root) = click_world_panel_root else {
                    return;
                };
                if !is_descendant_or_self(world, world_panel_root, *renderable) {
                    return;
                }

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

                if handle_world_panel_item_click(
                    world,
                    emit,
                    world_panel_root,
                    *renderable,
                    &click_editor_context_state,
                ) {
                    sync_and_refresh_inspector_panels(
                        world,
                        emit,
                        panel_query_root,
                        &click_editor_context_state,
                        &click_world_panel_scene_model,
                        &click_inspector_workspace_state,
                        &click_rendered_inspector_models,
                        &mut *click_data_renderer
                            .lock()
                            .expect("data renderer mutex poisoned"),
                    );
                }
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
                if apply_world_panel_semantic_selection(
                    world,
                    emit,
                    *selection_root,
                    world_panel_root,
                    status_wrap,
                    &world_selection_editor_context_state,
                ) {
                    sync_and_refresh_inspector_panels(
                        world,
                        emit,
                        panel_query_root,
                        &world_selection_editor_context_state,
                        &world_selection_scene_model,
                        &world_selection_inspector_workspace_state,
                        &world_selection_rendered_inspector_models,
                        &mut *world_selection_data_renderer
                            .lock()
                            .expect("data renderer mutex poisoned"),
                    );
                }

                let Some(panel_layout_selection) = world
                    .find_component(panel_query_root, &format!("#{PANEL_LAYOUT_SELECTION_NAME}"))
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
                let Some(EventSignal::SelectionChanged {
                    selection_root,
                    selected_component,
                    selected_payload,
                    ..
                }) = signal.event.as_ref()
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
                let Some(EventSignal::SelectionChanged {
                    selection_root,
                    selected_component,
                    selected_payload,
                    ..
                }) =
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
                let Some(EventSignal::SelectionChanged {
                    selection_root,
                    selected_component,
                    selected_payload,
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

        rx.add_handler_closure(
            SignalKind::SelectionChanged,
            panel_query_root,
            move |world, emit, signal| {
                let Some(EventSignal::SelectionChanged {
                    selection_root,
                    selected_component,
                    selected_payload,
                    ..
                }) = signal.event.as_ref()
                else {
                    return;
                };

                let Some(expected_selection_root) =
                    world.find_component(panel_query_root, GRID_PANEL_SELECTION_SELECTOR)
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
                let Some(grid_panel_root) =
                    world.find_component(panel_query_root, GRID_PANEL_ROOT_SELECTOR)
                else {
                    return;
                };

                emit.push_intent_now(
                    panel_layout_selection,
                    IntentValue::SelectionSet {
                        component_ids: vec![panel_layout_selection],
                        entries: vec![SelectionEntry {
                            index: None,
                            component: grid_panel_root,
                        }],
                        primary: Some(grid_panel_root),
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

        let Some(world_panel) = self.workspace_runtime.panel_instance(PanelKind::World) else {
            return;
        };
        let Some(content_slot) = world_panel.slots.get(&PanelSlotKind::List).copied() else {
            return;
        };
        let Some(selection_root) = world_panel
            .controls
            .get(&PanelControlKind::Selection)
            .copied()
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
        editor_memory_marker(
            "editor refresh_world_panels:after sync_editor_settings_panel_selection",
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
        editor_memory_marker("editor refresh_world_panels:after build_inspector_panel_models");
        let Some(bottom_row_root) = panel_layout_root_id(world, panel_query_root) else {
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
        _inspector_panel_pos: (f32, f32, f32),
        model: &WorldPanelModel,
        inspector_models: &[InspectorPanelModel],
        rendered_inspector_models: &Arc<Mutex<Vec<InspectorPanelModel>>>,
        working_file_path: &Path,
        asset_system: &crate::engine::ecs::system::AssetSystem,
        data_renderer: &mut DataRendererSystem,
    ) {
        let existing_world_panel =
            world.find_component(panel_query_root, WORLD_PANEL_ROOT_SELECTOR);
        let existing_panel_mount = world.all_components().find(|&component_id| {
            world
                .component_label(component_id)
                .is_some_and(|label| label == PANEL_LAYOUT_MOUNT_NAME)
        });

        println!(
            "[InspectorSystem][debug] reconcile_panel_layout panel_query_root={panel_query_root:?} existing_world_panel={existing_world_panel:?} existing_panel_mount={existing_panel_mount:?}"
        );

        if *panel_layout_spawned {
            if existing_world_panel.is_none() && existing_panel_mount.is_none() {
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

        if existing_world_panel.is_some() {
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

        let (panel_mount_root, layout_root_id) = match spawn_editor_panel_layout_tree(
            world,
            emit,
            model,
            working_file_path,
            world_panel_pos,
        ) {
            Some(ids) => ids,
            None => return,
        };

        // Post-spawn work
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
            PANEL_LAYOUT_MOUNT_NAME, world_panel_pos,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::command_queue::CommandQueue;
    use crate::engine::ecs::component::{
        DataComponent, DataValue, EditorComponent, SelectionComponent, TransformComponent,
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

        let world_panel_root = world
            .add_component_boxed_named("world_panel_root", Box::new(TransformComponent::new()));
        let status_wrap = world
            .add_component_boxed_named("save_status_wrap", Box::new(TransformComponent::new()));
        let _ = world.add_child(world_panel_root, status_wrap);

        apply_world_panel_semantic_selection(
            &mut world,
            &mut emit,
            selection_root,
            world_panel_root,
            status_wrap,
            &editor_context_state,
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
}
