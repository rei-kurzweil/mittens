use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::engine::ecs::component::TransformComponent;
use crate::engine::ecs::component::style::VerticalAlign;
use crate::engine::ecs::component::{
    ColorComponent, DataComponent, DataValue, Display, EdgeInsets, LayoutComponent,
    OptionComponent, Overflow, RaycastableComponent, SelectableComponent, SelectionComponent,
    SelectionEntry, SelectionMode, SerializeComponent, SizeDimension, StyleComponent, TextAlign,
    TextComponent,
};
use crate::engine::ecs::rx::RxWorld;
use crate::engine::ecs::system::editor_context_system::EditorContextState;
use crate::engine::ecs::system::editor_inspector_system::{
    InspectorPanelId, InspectorWorkspaceEvent, InspectorWorkspaceState,
    clear_missing_inspector_targets, reduce_inspector_workspace_state,
};
use crate::engine::ecs::system::selection_system::resolve_semantic_target_from_payload;
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, SignalKind, World};
use crate::meow_meow::component_registry::{
    filtered_root_ids_for_roots, filtered_roots_to_ce_ast, spawn_tree,
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
const WORLD_PANEL_PAYLOAD_NAME: &str = "world_panel_payload";
const INSPECTOR_PANEL_SELECTION_NAME: &str = "inspector_panel_selection";
const PANEL_CONTENT_SLOT_SELECTOR: &str = "#content_slot";
const INSPECTOR_PANEL_ROOT_SELECTOR: &str = "#inspector_panel_root";
const INSPECTOR_PANEL_CONTENT_ROOT_SELECTOR: &str = "#inspector_panel_content_root";
const INSPECTOR_PANEL_SELECTION_SELECTOR: &str = "#inspector_panel_selection";
const INSPECTOR_PANEL_INSTANCE_PREFIX: &str = "inspector_panel_instance_";
const INSPECTOR_PANEL_INSTANCE_DATA_NAME: &str = "inspector_panel_instance_data";
const INSPECTOR_PANEL_INSTANCE_ID_KEY: &str = "inspector_panel_id";
const INSPECTOR_PANEL_PIN_BUTTON_NAME: &str = "pin_button";
const INSPECTOR_PANEL_PIN_BUTTON_SELECTOR: &str = "#pin_button";
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
const ASSET_PANEL_WIDTH_GU: f64 = 39.0;
const ASSET_PANEL_TOTAL_HEIGHT_GU: f64 = 60.5;
const PAINT_PANEL_WIDTH_GU: f64 = 41.0;
const PAINT_PANEL_TOTAL_HEIGHT_GU: f64 = 32.0;
const PANEL_LAYOUT_GAP_GU: f64 = 2.0;
const PANEL_ROOT_MARGIN_X_GU: f64 = 0.5;
const PANEL_ROOT_MARGIN_Y_GU: f64 = 0.5;
const PANEL_LAYOUT_AVAILABLE_WIDTH_GU: f64 = 200000.0;
const MAX_INSPECTOR_PANEL_ROWS: usize = 256;

#[cfg(test)]
static WORLD_PANEL_SCENE_PATH_OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);
#[derive(Debug, Clone, PartialEq, Eq)]
struct WorldPanelModel {
    title: String,
    rows: Vec<WorldPanelRow>,
    selected_index: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct AuthoredWorldPanelSceneModel {
    sections: Vec<AuthoredWorldPanelSection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AuthoredWorldPanelSection {
    editor_root: ComponentId,
    rows: Vec<AuthoredWorldPanelRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AuthoredWorldPanelRow {
    target_component: ComponentId,
    label: String,
    depth: usize,
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
    EditorRoot,
    Info,
    Component,
}

#[derive(Debug)]
pub(crate) struct EditorInspectorSystemStopgapMmsAdapter {
    reconciler: EditorInspectorSystemStopgapMmsReconciler,
    panel_handler_installed: bool,
    panel_layout_spawned: bool,
    installed_editor_roots: Arc<Mutex<Vec<ComponentId>>>,
    refresh_handler_editor_roots: Arc<Mutex<Vec<ComponentId>>>,
    editor_context_state: Option<Arc<Mutex<EditorContextState>>>,
    runtime_ui_root: Arc<Mutex<Option<ComponentId>>>,
    working_file_path: Arc<Mutex<PathBuf>>,
    world_panel_scene_model: Arc<Mutex<AuthoredWorldPanelSceneModel>>,
    inspector_workspace_state: Arc<Mutex<InspectorWorkspaceState>>,
}

impl Default for EditorInspectorSystemStopgapMmsAdapter {
    fn default() -> Self {
        Self {
            reconciler: EditorInspectorSystemStopgapMmsReconciler,
            panel_handler_installed: false,
            panel_layout_spawned: false,
            installed_editor_roots: Arc::new(Mutex::new(Vec::new())),
            refresh_handler_editor_roots: Arc::new(Mutex::new(Vec::new())),
            editor_context_state: None,
            runtime_ui_root: Arc::new(Mutex::new(None)),
            working_file_path: Arc::new(Mutex::new(world_panel_scene_path())),
            world_panel_scene_model: Arc::new(Mutex::new(AuthoredWorldPanelSceneModel::default())),
            inspector_workspace_state: Arc::new(Mutex::new(InspectorWorkspaceState::default())),
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
        let runtime_ui_root = self.get_or_create_runtime_ui_root(world);

        println!(
            "[InspectorSystem][debug] setup_panels_for_editor editor_root={editor_root:?} runtime_ui_root={runtime_ui_root:?} world_panel_pos={:?} inspector_panel_pos={:?}",
            world_panel_pos, inspector_panel_pos,
        );

        register_editor_root(&self.installed_editor_roots, editor_root);
        rebuild_world_panel_scene_model(
            &self.world_panel_scene_model,
            world,
            &self.installed_editor_roots,
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
                &mut self.panel_layout_spawned,
                runtime_ui_root,
                world_panel_pos,
                inspector_panel_pos,
                &model,
                &inspector_models,
                &working_file_path,
                asset_system,
            );
        }

        self.refresh_world_panels(world, emit);

        self.install_shared_panel_handlers(rx, runtime_ui_root);
        self.install_editor_refresh_handlers(rx, editor_root);
    }

    fn install_shared_panel_handlers(&mut self, rx: &mut RxWorld, panel_query_root: ComponentId) {
        if self.panel_handler_installed {
            return;
        }
        self.panel_handler_installed = true;

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
        let click_installed_editor_roots = Arc::clone(&self.installed_editor_roots);
        let click_inspector_workspace_state = Arc::clone(&self.inspector_workspace_state);
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
                );

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

                    refresh_all_panel_models(
                        world,
                        emit,
                        panel_query_root,
                        &click_editor_context_state,
                        &click_world_panel_scene_model,
                        &click_inspector_workspace_state,
                        &click_installed_editor_roots,
                        true,
                    );
                    return;
                }

                let Some(row_name) = clicked_named_ancestor(world, *renderable, ITEM_PREFIX) else {
                    return;
                };
                let Some(row_index) = parse_item_index(&row_name) else {
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
                let row_label = world
                    .find_component(row_root, "Text")
                    .and_then(|text_id| world.get_component_by_id_as::<TextComponent>(text_id))
                    .map(|text| text.text.trim().to_string());

                crate::engine::ecs::system::selection_system::apply_selection_set(
                    world,
                    emit,
                    selection_root,
                    vec![SelectionEntry {
                        index: Some(row_index),
                        item: row_label,
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
                    selection_root,
                );
            },
        );

        let world_selection_editor_context_state = editor_context_state.clone();
        let world_selection_scene_model = Arc::clone(&self.world_panel_scene_model);
        let world_selection_inspector_workspace_state = Arc::clone(&self.inspector_workspace_state);
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
                    *selection_root,
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
        let runtime_ui_selectable = world.add_component_boxed_named(
            "editor_runtime_ui_selectable",
            Box::new(SelectableComponent::off()),
        );
        let _ = world.add_child(runtime_ui_root, runtime_ui_selectable);
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

    fn install_editor_refresh_handlers(&mut self, rx: &mut RxWorld, editor_root: ComponentId) {
        let already_installed = self
            .refresh_handler_editor_roots
            .lock()
            .expect("refresh handler editor roots mutex poisoned")
            .contains(&editor_root);
        if already_installed {
            return;
        }
        register_editor_root(&self.refresh_handler_editor_roots, editor_root);

        let panel_query_root = Arc::clone(&self.runtime_ui_root);
        let editor_context_state = self
            .editor_context_state
            .as_ref()
            .expect("editor context state must be installed before panels")
            .clone();
        let world_panel_scene_model = Arc::clone(&self.world_panel_scene_model);
        let inspector_workspace_state = Arc::clone(&self.inspector_workspace_state);
        rx.add_handler_closure(
            SignalKind::SelectionChanged,
            editor_root,
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
            panel_query_root,
            content_slot,
            &model.rows,
            model.selected_index,
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
        let Some(bottom_row_root) = panel_layout_bottom_row_id(world, panel_query_root) else {
            return;
        };
        rerender_inspector_panels(world, emit, bottom_row_root, &inspector_models);
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
    rebuild_world_panel: bool,
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
            panel_query_root,
            content_slot,
            &world_model.rows,
            world_model.selected_index,
        );
    }

    sync_and_refresh_inspector_panels(
        world,
        emit,
        panel_query_root,
        editor_context_state,
        world_panel_scene_model,
        inspector_workspace_state,
    );
}

fn sync_and_refresh_inspector_panels(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    editor_context_state: &Arc<Mutex<EditorContextState>>,
    world_panel_scene_model: &Arc<Mutex<AuthoredWorldPanelSceneModel>>,
    inspector_workspace_state: &Arc<Mutex<InspectorWorkspaceState>>,
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
    rerender_inspector_panels(world, emit, bottom_row_root, &inspector_models);
}

fn refresh_inspector_panels_from_workspace(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    world_panel_scene_model: &Arc<Mutex<AuthoredWorldPanelSceneModel>>,
    inspector_workspace_state: &Arc<Mutex<InspectorWorkspaceState>>,
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
    rerender_inspector_panels(world, emit, bottom_row_root, &inspector_models);
}

fn apply_world_panel_semantic_selection(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    editor_context_state: &Arc<Mutex<EditorContextState>>,
    world_panel_scene_model: &Arc<Mutex<AuthoredWorldPanelSceneModel>>,
    inspector_workspace_state: &Arc<Mutex<InspectorWorkspaceState>>,
    selection_root: ComponentId,
) {
    let Some(selection) = world.get_component_by_id_as::<SelectionComponent>(selection_root) else {
        return;
    };
    let Some(target_component) = resolve_semantic_target_from_payload(
        world,
        selection.selected_payload,
        selection.selected_component,
    ) else {
        return;
    };
    let active_editor = nearest_editor_ancestor(world, target_component);
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
        "[InspectorSystem][trace] world_panel selection_root={selection_root:?} clicked_row={:?} payload={:?} authored_target={target_component:?} active_editor={active_editor:?}",
        selection.selected_component, selection.selected_payload
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

fn sync_world_panel_selection(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    editor_context_state: &Arc<Mutex<EditorContextState>>,
    world_panel_scene_model: &Arc<Mutex<AuthoredWorldPanelSceneModel>>,
) {
    let editor_context = editor_context_state
        .lock()
        .expect("editor context state mutex poisoned")
        .clone();
    let Some(world_panel_root) = world.find_component(panel_query_root, WORLD_PANEL_ROOT_SELECTOR)
    else {
        return;
    };
    let Some(selection_root) =
        world.find_component(world_panel_root, &format!("#{WORLD_PANEL_SELECTION_NAME}"))
    else {
        return;
    };

    let Some(target_component) = editor_context
        .selected_component
        .or(editor_context.active_editor)
    else {
        crate::engine::ecs::system::selection_system::apply_selection_set(
            world,
            emit,
            selection_root,
            Vec::new(),
            None,
        );
        return;
    };
    let model = build_world_panel_model(
        world,
        &editor_context,
        &world_panel_scene_model
            .lock()
            .expect("world panel scene model mutex poisoned"),
    );
    let Some((selected_index, row)) = model
        .rows
        .iter()
        .enumerate()
        .find(|(_, row)| row.target_component == Some(target_component))
    else {
        crate::engine::ecs::system::selection_system::apply_selection_set(
            world,
            emit,
            selection_root,
            Vec::new(),
            None,
        );
        return;
    };
    let label = row.label.clone();

    let already_selected = world
        .get_component_by_id_as::<SelectionComponent>(selection_root)
        .is_some_and(|selection| {
            selection.selected_payload.is_some_and(|payload| {
                resolve_semantic_target_from_payload(
                    world,
                    Some(payload),
                    selection.selected_component,
                ) == Some(target_component)
            }) && selection.selected_index == Some(selected_index)
        });
    if already_selected {
        return;
    }

    let Some(row_root) =
        world.find_component(world_panel_root, &format!("#{ITEM_PREFIX}{selected_index}"))
    else {
        return;
    };

    crate::engine::ecs::system::selection_system::apply_selection_set(
        world,
        emit,
        selection_root,
        vec![SelectionEntry {
            index: Some(selected_index),
            item: Some(label),
            component: row_root,
        }],
        Some(row_root),
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
        world_panel_pos: (f32, f32, f32),
        inspector_panel_pos: (f32, f32, f32),
        model: &WorldPanelModel,
        inspector_models: &[InspectorPanelModel],
        working_file_path: &Path,
        asset_system: &crate::engine::ecs::system::AssetSystem,
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

        let total_height_gu = WORLD_PANEL_TOTAL_HEIGHT_GU
            .max(INSPECTOR_PANEL_TOTAL_HEIGHT_GU)
            .max(ASSET_PANEL_TOTAL_HEIGHT_GU)
            .max(PAINT_PANEL_TOTAL_HEIGHT_GU)
            * 2.0
            + PANEL_LAYOUT_GAP_GU
            + (PANEL_ROOT_MARGIN_Y_GU * 2.0);

        let world_panel = decorate_panel_root_ce(world_panel);
        let paint_panel = decorate_panel_root_ce(paint_panel);
        let asset_panel = decorate_panel_root_ce(asset_panel);

        let shared_layout_root = MaterializedCE {
            component_type: "LayoutRoot".to_string(),
            ctor_method: None,
            ctor_args: Vec::new(),
            calls: vec![
                (
                    "available_width".to_string(),
                    vec![Value::Number(PANEL_LAYOUT_AVAILABLE_WIDTH_GU)],
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
                CeChild::Spawn(paint_panel),
                CeChild::Spawn(asset_panel),
                CeChild::Spawn(world_panel),
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

        if let Some(layout_root) =
            world.find_component(panel_mount_root, &format!("#{PANEL_LAYOUT_ROOT_NAME}"))
        {
            rerender_inspector_panels(world, emit, layout_root, inspector_models);
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
    panel_id: InspectorPanelId,
    title: String,
    rows: Vec<InspectorPanelRow>,
    pinned: bool,
    active: bool,
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
            | "world_panel_content_root"
            | "inspector_panel_content_root"
            | "panel_status_root"
            | "paint_panel_item"
            | "rows_mount"
    ) || name.starts_with(INSPECTOR_PANEL_INSTANCE_PREFIX)
}

fn panel_layout_root_id(world: &World, panel_query_root: ComponentId) -> Option<ComponentId> {
    world.find_component(panel_query_root, "#editor_panel_layout_root")
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

fn editor_scene_roots(world: &World) -> Vec<ComponentId> {
    world
        .all_components()
        .filter(|&component_id| {
            world.parent_of(component_id).is_none()
                && world
                    .get_component_by_id_as::<crate::engine::ecs::component::EditorComponent>(
                        component_id,
                    )
                    .is_some()
        })
        .collect()
}

fn build_world_panel_model(
    world: &World,
    editor_context: &EditorContextState,
    scene_model: &AuthoredWorldPanelSceneModel,
) -> WorldPanelModel {
    let rows = build_world_panel_rows(world, scene_model);
    let selected_target = editor_context
        .selected_component
        .or(editor_context.active_editor);
    let selected_index = selected_target.and_then(|selected| {
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

fn build_inspector_panel_models(
    world: &World,
    scene_model: &AuthoredWorldPanelSceneModel,
    workspace: &InspectorWorkspaceState,
) -> Vec<InspectorPanelModel> {
    if workspace.panels.is_empty() {
        return vec![InspectorPanelModel {
            panel_id: 0,
            title: "Inspector".to_string(),
            rows: vec![InspectorPanelRow {
                display_label: "<nothing selected>".to_string(),
                kind: InspectorPanelRowKind::Info,
            }],
            pinned: false,
            active: true,
        }];
    }

    workspace
        .panels
        .iter()
        .map(|panel| {
            println!(
                "[InspectorSystem][trace] build_inspector_panel_model panel_id={} target={:?} pinned={}",
                panel.panel_id, panel.inspected, panel.pinned
            );
            let rows = panel
                .inspected
                .filter(|&component_id| world.get_component_record(component_id).is_some())
                .map(|component_id| build_inspector_panel_rows(world, scene_model, component_id))
                .unwrap_or_else(|| {
                    vec![InspectorPanelRow {
                        display_label: "<nothing selected>".to_string(),
                        kind: InspectorPanelRowKind::Info,
                    }]
                });

            let target_label = panel
                .inspected
                .filter(|&component_id| world.get_component_record(component_id).is_some())
                .map(|component_id| world_panel_item_label(world, component_id))
                .unwrap_or_else(|| "Inspector".to_string());

            InspectorPanelModel {
                panel_id: panel.panel_id,
                title: if panel.pinned {
                    format!("{target_label} [Pinned]")
                } else {
                    target_label
                },
                rows,
                pinned: panel.pinned,
                active: workspace.active_panel == Some(panel.panel_id),
            }
        })
        .collect()
}

fn build_inspector_panel_rows(
    world: &World,
    scene_model: &AuthoredWorldPanelSceneModel,
    root: ComponentId,
) -> Vec<InspectorPanelRow> {
    if let Some(rows) = build_authored_inspector_panel_rows(world, scene_model, root) {
        return rows;
    }

    if matches!(
        authored_scene_node_policy(world, root),
        AuthoredSceneNodePolicy::Skip
    ) {
        return vec![InspectorPanelRow {
            display_label: "<selection hidden>".to_string(),
            kind: InspectorPanelRowKind::Info,
        }];
    }

    let mut rows = Vec::new();
    push_inspector_panel_rows(world, root, 0, &mut rows);
    rows
}

fn build_authored_inspector_panel_rows(
    world: &World,
    scene_model: &AuthoredWorldPanelSceneModel,
    root: ComponentId,
) -> Option<Vec<InspectorPanelRow>> {
    for section in &scene_model.sections {
        if section.editor_root == root {
            let mut rows = Vec::with_capacity(section.rows.len() + 1);
            rows.push(InspectorPanelRow {
                display_label: editor_chunk_label(world, section.editor_root),
                kind: InspectorPanelRowKind::Component,
            });
            rows.extend(section.rows.iter().map(|row| InspectorPanelRow {
                display_label: format!("{}{}", "  ".repeat(row.depth + 1), row.label),
                kind: InspectorPanelRowKind::Component,
            }));
            return Some(rows);
        }

        let Some((root_index, root_row)) = section
            .rows
            .iter()
            .enumerate()
            .find(|(_, row)| row.target_component == root)
        else {
            continue;
        };

        let mut rows = Vec::new();
        rows.push(InspectorPanelRow {
            display_label: root_row.label.clone(),
            kind: InspectorPanelRowKind::Component,
        });

        for row in section.rows.iter().skip(root_index + 1) {
            if row.depth <= root_row.depth {
                break;
            }

            rows.push(InspectorPanelRow {
                display_label: format!("{}{}", "  ".repeat(row.depth - root_row.depth), row.label),
                kind: InspectorPanelRowKind::Component,
            });
        }

        return Some(rows);
    }

    None
}

fn push_inspector_panel_rows(
    world: &World,
    component_id: ComponentId,
    depth: usize,
    out: &mut Vec<InspectorPanelRow>,
) {
    match authored_scene_node_policy(world, component_id) {
        AuthoredSceneNodePolicy::Skip => return,
        AuthoredSceneNodePolicy::Flatten => {
            for &child in world.children_of(component_id) {
                push_inspector_panel_rows(world, child, depth, out);
                if out.len() >= MAX_INSPECTOR_PANEL_ROWS {
                    return;
                }
            }
            return;
        }
        AuthoredSceneNodePolicy::Include => {}
    }

    if out.len() >= MAX_INSPECTOR_PANEL_ROWS {
        return;
    }

    out.push(InspectorPanelRow {
        display_label: format!(
            "{}{}",
            "  ".repeat(depth),
            world_panel_item_label(world, component_id)
        ),
        kind: InspectorPanelRowKind::Component,
    });

    for &child in world.children_of(component_id) {
        if out.len() >= MAX_INSPECTOR_PANEL_ROWS {
            out.push(InspectorPanelRow {
                display_label: "… inspector truncated …".to_string(),
                kind: InspectorPanelRowKind::Info,
            });
            return;
        }
        push_inspector_panel_rows(world, child, depth + 1, out);
    }
}

fn build_world_panel_rows(
    world: &World,
    scene_model: &AuthoredWorldPanelSceneModel,
) -> Vec<WorldPanelRow> {
    let mut out = Vec::new();
    for (editor_index, section) in scene_model.sections.iter().enumerate() {
        if editor_index > 0 {
            out.push(WorldPanelRow {
                target_component: None,
                label: String::new(),
                display_label: String::new(),
                kind: WorldPanelRowKind::Spacer,
            });
        }

        let header_label = editor_chunk_label(world, section.editor_root);
        out.push(WorldPanelRow {
            target_component: Some(section.editor_root),
            label: header_label.clone(),
            display_label: header_label,
            kind: WorldPanelRowKind::EditorRoot,
        });

        for row in &section.rows {
            out.push(WorldPanelRow {
                target_component: Some(row.target_component),
                display_label: format!("{}{}", "  ".repeat(row.depth), row.label),
                label: row.label.clone(),
                kind: WorldPanelRowKind::Component,
            });
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

fn parse_item_index(row_name: &str) -> Option<usize> {
    row_name.strip_prefix(ITEM_PREFIX)?.parse().ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthoredSceneNodePolicy {
    Include,
    Skip,
    Flatten,
}

fn authored_scene_node_policy(world: &World, component_id: ComponentId) -> AuthoredSceneNodePolicy {
    if world
        .get_component_by_id_as::<crate::engine::ecs::component::EditorComponent>(component_id)
        .is_some()
    {
        return AuthoredSceneNodePolicy::Include;
    }

    match world.component_label(component_id) {
        Some("editor_auto_raycastable") => return AuthoredSceneNodePolicy::Flatten,
        Some("selection_highlight")
        | Some(EDITOR_RUNTIME_UI_ROOT_NAME)
        | Some("editor_gizmo_anchor")
        | Some("editor_transform_gizmo") => return AuthoredSceneNodePolicy::Skip,
        _ => {}
    }

    if world
        .get_component_by_id_as::<crate::engine::ecs::component::TransformGizmoComponent>(
            component_id,
        )
        .is_some()
    {
        return AuthoredSceneNodePolicy::Skip;
    }

    AuthoredSceneNodePolicy::Include
}

fn rebuild_world_panel_scene_model(
    scene_model: &Arc<Mutex<AuthoredWorldPanelSceneModel>>,
    world: &World,
    installed_editor_roots: &Arc<Mutex<Vec<ComponentId>>>,
) {
    let editor_roots = effective_editor_roots(world, installed_editor_roots);
    let rebuilt = build_authored_world_panel_scene_model(world, &editor_roots);
    *scene_model
        .lock()
        .expect("world panel scene model mutex poisoned") = rebuilt;
}

fn register_editor_root(
    installed_editor_roots: &Arc<Mutex<Vec<ComponentId>>>,
    editor_root: ComponentId,
) {
    let mut roots = installed_editor_roots
        .lock()
        .expect("installed editor roots mutex poisoned");
    if !roots.contains(&editor_root) {
        roots.push(editor_root);
    }
}

fn effective_editor_roots(
    world: &World,
    installed_editor_roots: &Arc<Mutex<Vec<ComponentId>>>,
) -> Vec<ComponentId> {
    let mut roots = installed_editor_roots
        .lock()
        .expect("installed editor roots mutex poisoned")
        .iter()
        .copied()
        .filter(|&component_id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::EditorComponent>(
                    component_id,
                )
                .is_some()
        })
        .collect::<Vec<_>>();

    for editor_root in editor_scene_roots(world) {
        if !roots.contains(&editor_root) {
            roots.push(editor_root);
        }
    }

    roots.sort_by_key(|component_id| component_id_short(*component_id));
    roots
}

fn build_authored_world_panel_scene_model(
    world: &World,
    editor_roots: &[ComponentId],
) -> AuthoredWorldPanelSceneModel {
    let mut sections = Vec::new();

    for &editor_root in editor_roots {
        let mut rows = Vec::new();
        for &child in world.children_of(editor_root) {
            push_authored_world_panel_rows(world, child, 0, &mut rows);
        }
        sections.push(AuthoredWorldPanelSection { editor_root, rows });
    }

    AuthoredWorldPanelSceneModel { sections }
}

fn push_authored_world_panel_rows(
    world: &World,
    component_id: ComponentId,
    depth: usize,
    out: &mut Vec<AuthoredWorldPanelRow>,
) {
    match authored_scene_node_policy(world, component_id) {
        AuthoredSceneNodePolicy::Skip => return,
        AuthoredSceneNodePolicy::Flatten => {
            for &child in world.children_of(component_id) {
                push_authored_world_panel_rows(world, child, depth, out);
            }
        }
        AuthoredSceneNodePolicy::Include => {
            out.push(AuthoredWorldPanelRow {
                target_component: component_id,
                label: world_panel_item_label(world, component_id),
                depth,
            });

            for &child in world.children_of(component_id) {
                push_authored_world_panel_rows(world, child, depth + 1, out);
            }
        }
    }
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

fn rerender_inspector_panels(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    layout_root: ComponentId,
    models: &[InspectorPanelModel],
) {
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
            update_inspector_panel_instance_tree(world, emit, instance_root, model);
            emit.push_intent_now(
                instance_root,
                IntentValue::Detach {
                    component_ids: vec![instance_root],
                },
            );
            emit.push_intent_now(
                layout_root,
                IntentValue::Attach {
                    parents: vec![layout_root],
                    child: instance_root,
                },
            );
            continue;
        }

        let instance_root = spawn_inspector_panel_instance_tree(world, emit, model, index);
        emit.push_intent_now(
            layout_root,
            IntentValue::Attach {
                parents: vec![layout_root],
                child: instance_root,
            },
        );
    }
    mark_nearest_layout_dirty(world, layout_root);
}

fn rerender_single_inspector_panel_content(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    inspector_panel_root: ComponentId,
    content_slot: ComponentId,
    rows: &[InspectorPanelRow],
) {
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

fn update_inspector_panel_instance_tree(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    instance_root: ComponentId,
    model: &InspectorPanelModel,
) {
    let inspector_panel_root = instance_root;
    let Some(content_slot) =
        world.find_component(inspector_panel_root, PANEL_CONTENT_SLOT_SELECTOR)
    else {
        return;
    };

    if let Some(title_label) = world.find_component(inspector_panel_root, "#title_label") {
        emit.push_intent_now(
            title_label,
            IntentValue::SetText {
                component_ids: vec![title_label],
                text: model.title.clone(),
            },
        );
    }

    if let Some(existing_pin_button) =
        world.find_component(inspector_panel_root, INSPECTOR_PANEL_PIN_BUTTON_SELECTOR)
    {
        emit.push_intent_now(
            existing_pin_button,
            IntentValue::RemoveSubtree {
                component_ids: vec![existing_pin_button],
            },
        );
    }
    if let Some(title_bar) = world.find_component(inspector_panel_root, "#title_bar") {
        let pin_button = spawn_inspector_pin_button(world, model.pinned);
        let _ = world.add_child(title_bar, pin_button);
    }

    rerender_single_inspector_panel_content(
        world,
        emit,
        inspector_panel_root,
        content_slot,
        &model.rows,
    );
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

fn inspector_panel_instance_id_on_root(
    world: &World,
    root: ComponentId,
) -> Option<InspectorPanelId> {
    world.children_of(root).iter().find_map(|&child| {
        world
            .get_component_by_id_as::<DataComponent>(child)
            .and_then(|data| match data.get(INSPECTOR_PANEL_INSTANCE_ID_KEY) {
                Some(DataValue::Integer(id)) => Some(*id as InspectorPanelId),
                _ => None,
            })
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
}

fn handle_inspector_panel_workspace_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    renderable: ComponentId,
    world_panel_scene_model: &Arc<Mutex<AuthoredWorldPanelSceneModel>>,
    inspector_workspace_state: &Arc<Mutex<InspectorWorkspaceState>>,
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
    }

    if rerender_needed {
        refresh_inspector_panels_from_workspace(
            world,
            emit,
            panel_query_root,
            world_panel_scene_model,
            inspector_workspace_state,
        );
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
                    item: Some(format!("{INSPECTOR_PANEL_INSTANCE_PREFIX}{panel_id}")),
                    component: panel_root,
                }],
                primary: Some(panel_root),
            },
        );
        return;
    }

    for (selector, item_name) in [
        (WORLD_PANEL_ROOT_SELECTOR, "world_panel_root"),
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

fn decorate_panel_root_ce(mut panel_root: MaterializedCE) -> MaterializedCE {
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
            "margin_right".to_string(),
            vec![Value::Number(PANEL_LAYOUT_GAP_GU)],
        ));
    }

    panel_root
}

fn spawn_inspector_panel_instance_tree(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    model: &InspectorPanelModel,
    index: usize,
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

    let panel_ce = match build_panel_component_expr(
        world,
        emit,
        inspector_panel_asset_path(),
        "inspector_panel",
        vec![
            Value::String(model.title.clone()),
            Value::Array(Vec::new()),
            title_color,
            panel_bg,
            item_bg,
        ],
        "inspector panel",
    ) {
        Some(panel) => {
            decorate_panel_root_ce(panel)
        }
        None => {
            return spawn_inspector_panel_instance_fallback_root(world, model.panel_id);
        }
    };

    let instance_root = match spawn_tree(&panel_ce, None, world, emit) {
        Ok(component_id) => component_id,
        Err(error) => {
            eprintln!("[InspectorSystemStopgapMmsAdapter] inspector instance spawn error: {error}");
            return spawn_inspector_panel_instance_fallback_root(world, model.panel_id);
        }
    };

    attach_inspector_panel_instance_id(world, instance_root, model.panel_id);

    let inspector_panel_root = instance_root;
    let Some(title_bar) = world.find_component(inspector_panel_root, "#title_bar") else {
        return inspector_panel_root;
    };
    let Some(content_slot) =
        world.find_component(inspector_panel_root, PANEL_CONTENT_SLOT_SELECTOR)
    else {
        return inspector_panel_root;
    };

    let pin_button = spawn_inspector_pin_button(world, model.pinned);
    let _ = world.add_child(title_bar, pin_button);
    rerender_single_inspector_panel_content(
        world,
        emit,
        inspector_panel_root,
        content_slot,
        &model.rows,
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

fn spawn_inspector_pin_button(world: &mut World, pinned: bool) -> ComponentId {
    let root = world.add_component_boxed_named(
        INSPECTOR_PANEL_PIN_BUTTON_NAME,
        Box::new(TransformComponent::new()),
    );
    let raycastable = world.add_component_boxed_named(
        "pin_button_raycastable",
        Box::new(RaycastableComponent::click_only()),
    );
    let _ = world.add_child(root, raycastable);
    let style = world.add_component_boxed_named(
        "pin_button_style",
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::InlineBlock);
            style.width = SizeDimension::GlyphUnits(5.0);
            style.height = SizeDimension::GlyphUnits(2.4);
            style.margin = EdgeInsets {
                left: SizeDimension::GlyphUnits(15.5),
                right: SizeDimension::GlyphUnits(0.0),
                top: SizeDimension::GlyphUnits(0.3),
                bottom: SizeDimension::GlyphUnits(0.3),
            };
            style.padding = EdgeInsets::axes(0.0, 0.35);
            style.text_align = TextAlign::Center;
            style.vertical_align = VerticalAlign::Middle;
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
            style
        }),
    );
    let text_root = world
        .add_component_boxed_named("pin_button_text_root", Box::new(TransformComponent::new()));
    let text = world.add_component_boxed_named(
        "pin_button_text",
        Box::new(TextComponent::new(if pinned { "Unpin" } else { "Pin" })),
    );

    let _ = world.add_child(root, style);
    let _ = world.add_child(root, text_root);
    let _ = world.add_child(text_root, text);
    root
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
        && let Some(row) = rows.get(index)
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
            item: Some(row.label.clone()),
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
        WorldPanelRowKind::EditorRoot | WorldPanelRowKind::Info | WorldPanelRowKind::Component => {
            let (background_rgba, text_rgba, interactive) = match row.kind {
                WorldPanelRowKind::EditorRoot => {
                    ([0.30, 0.84, 0.38, 0.98], [0.03, 0.08, 0.04, 1.0], true)
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

            if let Some(target_component) = row.target_component {
                let payload = world.add_component_boxed_named(
                    WORLD_PANEL_PAYLOAD_NAME,
                    Box::new(
                        DataComponent::new()
                            .with_entry("target_component", DataValue::Component(target_component))
                            .with_entry("row_kind", DataValue::Text(format!("{:?}", row.kind)))
                            .with_entry("label", DataValue::Text(row.label.clone())),
                    ),
                );
                let _ = world.add_child(row_root, payload);
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

fn resolve_selected_world_panel_payload(
    world: &World,
    row_root: ComponentId,
) -> Option<ComponentId> {
    let matches =
        world.find_all_components(row_root, &format!("[name='{WORLD_PANEL_PAYLOAD_NAME}']"));
    if matches.len() == 1 {
        matches.into_iter().next()
    } else {
        None
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
