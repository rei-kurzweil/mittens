use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::engine::ecs::component::{
    EditorComponent, RaycastableComponent, SelectableComponent, SelectionComponent,
    TransformComponent,
    TransformGizmoComponent,
};
use crate::engine::ecs::system::grid_system::{GridSnapResult, GridStep, GridSystem};
use crate::engine::ecs::system::paint_placement::{resolve_placement_pose, PlacementError};
use crate::engine::ecs::system::paint_system_state_manager::{
    is_paint_active, is_paint_panel_focused, paint_tool_from_item, reduce_paint_state, PaintEvent,
    PaintState, PaintTool,
};
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, RxWorld, Signal, SignalEmitter, SignalKind, World,
};
use crate::meow_meow::object::Value;
use crate::meow_meow::runner::{LoadedMmsModule, MeowMeowRunner};

const PANEL_LAYOUT_SELECTION_SELECTOR: &str = "#editor_panel_layout_selection";
const PAINT_PANEL_ROOT_SELECTOR: &str = "#paint_panel_root";
const WORLD_PANEL_SELECTION_SELECTOR: &str = "#world_panel_selection";
const ASSETS_SELECTION_SELECTOR: &str = "#assets_selection";
const PAINT_TOOL_SELECTION_SELECTOR: &str = "#paint_tool_selection";
const PAINT_STATUS_WRAP_SELECTOR: &str = "#paint_status_wrap";
const PANEL_STATUS_VALUE_SELECTOR: &str = "#panel_status_value";
const RUNTIME_UI_ROOT_NAME: &str = "editor_runtime_ui_root";

#[derive(Debug, Clone)]
pub struct PaintAssetTemplate {
    pub title: String,
    pub module: LoadedMmsModule,
    pub export_name: String,
    pub param_names: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct SharedEditorWorkspaceState {
    panel_query_root: Option<ComponentId>,
    registered_editors: Vec<ComponentId>,
}

impl SharedEditorWorkspaceState {
    fn register_editor(&mut self, editor_root: ComponentId) -> bool {
        if self.registered_editors.contains(&editor_root) {
            return false;
        }
        self.registered_editors.push(editor_root);
        true
    }

    fn default_active_editor(&self) -> Option<ComponentId> {
        self.registered_editors.first().copied()
    }
}

#[derive(Debug, Clone, Default)]
struct PaintStrokeRuntime {
    active: bool,
    captured_renderable: Option<ComponentId>,
    non_grid_placed: bool,
    last_grid_step: Option<GridStep>,
}

#[derive(Debug, Default)]
pub struct PaintSystem {
    installed_editor_roots: HashSet<ComponentId>,
    shared_panel_handlers_installed: bool,
    shared_state: Arc<Mutex<PaintState>>,
    shared_workspace_state: Arc<Mutex<SharedEditorWorkspaceState>>,
    shared_templates: Arc<Mutex<Vec<PaintAssetTemplate>>>,
}

impl PaintSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn install_scoped_handlers_for_editor(
        &mut self,
        rx: &mut RxWorld,
        world: &World,
        editor_root: ComponentId,
        panel_query_root: ComponentId,
        templates: Vec<PaintAssetTemplate>,
    ) {
        {
            let mut workspace = self
                .shared_workspace_state
                .lock()
                .expect("paint workspace mutex poisoned");
            workspace.panel_query_root = Some(panel_query_root);
            let registered = workspace.register_editor(editor_root);
            drop(workspace);

            if registered {
                ensure_default_active_editor(&self.shared_state, &self.shared_workspace_state);
            }
        }

        *self
            .shared_templates
            .lock()
            .expect("paint templates mutex poisoned") = templates;

        if !self.shared_panel_handlers_installed {
            self.shared_panel_handlers_installed = true;
            println!(
                "[PaintSystem][trace] install shared_panel_handlers panel_query_root={panel_query_root:?}"
            );
            install_shared_panel_handlers(
                rx,
                world,
                panel_query_root,
                Arc::clone(&self.shared_state),
                Arc::clone(&self.shared_workspace_state),
                Arc::clone(&self.shared_templates),
            );
            bootstrap_paint_state(
                world,
                panel_query_root,
                &self.shared_state,
                &self.shared_workspace_state,
            );
        }

        if self.installed_editor_roots.contains(&editor_root) {
            return;
        }
        self.installed_editor_roots.insert(editor_root);
        println!(
            "[PaintSystem][trace] install editor_scene_handlers editor_root={editor_root:?} panel_query_root={panel_query_root:?}"
        );

        let stroke_runtime = Arc::new(Mutex::new(PaintStrokeRuntime::default()));
        install_editor_scene_handlers(
            rx,
            editor_root,
            panel_query_root,
            Arc::clone(&self.shared_state),
            Arc::clone(&self.shared_workspace_state),
            Arc::clone(&self.shared_templates),
            stroke_runtime,
        );
    }
}

fn install_shared_panel_handlers(
    rx: &mut RxWorld,
    world: &World,
    panel_query_root: ComponentId,
    paint_state: Arc<Mutex<PaintState>>,
    workspace_state: Arc<Mutex<SharedEditorWorkspaceState>>,
    templates: Arc<Mutex<Vec<PaintAssetTemplate>>>,
) {
    let _ = world;
    rx.add_handler_closure(
        SignalKind::SelectionChanged,
        panel_query_root,
        move |world, emit, signal| {
            let Some(event) = paint_event_from_shared_signal(world, panel_query_root, signal)
            else {
                return;
            };

            handle_paint_event(
                world,
                emit,
                panel_query_root,
                &templates,
                &paint_state,
                &workspace_state,
                None,
                &event,
            );
            sync_paint_state_from_shared_selections(world, panel_query_root, &paint_state);
        },
    );
}

fn install_editor_scene_handlers(
    rx: &mut RxWorld,
    editor_root: ComponentId,
    panel_query_root: ComponentId,
    paint_state: Arc<Mutex<PaintState>>,
    workspace_state: Arc<Mutex<SharedEditorWorkspaceState>>,
    templates: Arc<Mutex<Vec<PaintAssetTemplate>>>,
    stroke_runtime: Arc<Mutex<PaintStrokeRuntime>>,
) {
    for signal_kind in [
        SignalKind::SelectionChanged,
        SignalKind::Click,
        SignalKind::DragStart,
        SignalKind::DragMove,
        SignalKind::DragEnd,
    ] {
        let state = Arc::clone(&paint_state);
        let workspace = Arc::clone(&workspace_state);
        let shared_templates = Arc::clone(&templates);
        let runtime = Arc::clone(&stroke_runtime);
        rx.add_handler_closure(signal_kind, editor_root, move |world, emit, signal| {
            let Some(event) =
                paint_event_from_editor_signal(world, editor_root, panel_query_root, signal)
            else {
                return;
            };

            handle_paint_event(
                world,
                emit,
                panel_query_root,
                &shared_templates,
                &state,
                &workspace,
                Some(&runtime),
                &event,
            );
        });
    }
}

fn handle_paint_event(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    templates: &Arc<Mutex<Vec<PaintAssetTemplate>>>,
    paint_state: &Arc<Mutex<PaintState>>,
    workspace_state: &Arc<Mutex<SharedEditorWorkspaceState>>,
    stroke_runtime: Option<&Arc<Mutex<PaintStrokeRuntime>>>,
    event: &PaintEvent,
) {
    let (old_state, new_state) = {
        let mut state = paint_state.lock().expect("paint state mutex poisoned");
        let old_state = state.clone();
        let new_state = reduce_paint_state(&old_state, event);
        println!(
            "[PaintSystem][trace] reduce panel_query_root={panel_query_root:?} old_state={old_state:?} event={event:?} new_state={new_state:?}"
        );
        *state = new_state.clone();
        (old_state, new_state)
    };

    apply_paint_side_effects(
        world,
        emit,
        panel_query_root,
        templates,
        workspace_state,
        &old_state,
        &new_state,
        event,
        stroke_runtime,
    );
}

fn bootstrap_paint_state(
    world: &World,
    panel_query_root: ComponentId,
    paint_state: &Arc<Mutex<PaintState>>,
    workspace_state: &Arc<Mutex<SharedEditorWorkspaceState>>,
) {
    let mut events = Vec::new();

    if let Some(asset_event) = bootstrap_selection_event(
        world,
        panel_query_root,
        ASSETS_SELECTION_SELECTOR,
        |selection| PaintEvent::AssetSelectionChanged {
            item: selection.selected_item.clone(),
            component: selection.selected_component,
        },
    ) {
        events.push(asset_event);
    }

    if let Some(tool_event) = bootstrap_selection_event(
        world,
        panel_query_root,
        PAINT_TOOL_SELECTION_SELECTOR,
        |selection| PaintEvent::ToolSelectionChanged {
            tool: paint_tool_from_item(selection.selected_item.clone()),
            item: selection.selected_item.clone(),
            component: selection.selected_component,
        },
    ) {
        events.push(tool_event);
    }

    if let Some(panel_event) = bootstrap_selection_event(
        world,
        panel_query_root,
        PANEL_LAYOUT_SELECTION_SELECTOR,
        |selection| PaintEvent::PanelFocusChanged {
            focused_panel: selection.selected_component,
        },
    ) {
        events.push(panel_event);
    }

    if let Some(editor) = workspace_state
        .lock()
        .expect("paint workspace mutex poisoned")
        .default_active_editor()
    {
        events.push(PaintEvent::ActiveEditorChanged {
            editor: Some(editor),
        });
    }

    for event in events {
        let mut state = paint_state.lock().expect("paint state mutex poisoned");
        *state = reduce_paint_state(&state, &event);
    }
}

fn bootstrap_selection_event<F>(
    world: &World,
    panel_query_root: ComponentId,
    selector: &str,
    event_builder: F,
) -> Option<PaintEvent>
where
    F: FnOnce(&SelectionComponent) -> PaintEvent,
{
    let selection_root = world.find_component(panel_query_root, selector)?;
    let selection = world.get_component_by_id_as::<SelectionComponent>(selection_root)?;
    Some(event_builder(selection))
}

fn sync_paint_state_from_shared_selections(
    world: &World,
    panel_query_root: ComponentId,
    paint_state: &Arc<Mutex<PaintState>>,
) {
    let mut events = Vec::new();

    if let Some(event) = bootstrap_selection_event(
        world,
        panel_query_root,
        ASSETS_SELECTION_SELECTOR,
        |selection| PaintEvent::AssetSelectionChanged {
            item: selection.selected_item.clone(),
            component: selection.selected_component,
        },
    ) {
        events.push(event);
    }

    if let Some(event) = bootstrap_selection_event(
        world,
        panel_query_root,
        PAINT_TOOL_SELECTION_SELECTOR,
        |selection| PaintEvent::ToolSelectionChanged {
            tool: paint_tool_from_item(selection.selected_item.clone()),
            item: selection.selected_item.clone(),
            component: selection.selected_component,
        },
    ) {
        events.push(event);
    }

    if let Some(event) = bootstrap_selection_event(
        world,
        panel_query_root,
        PANEL_LAYOUT_SELECTION_SELECTOR,
        |selection| PaintEvent::PanelFocusChanged {
            focused_panel: selection.selected_component,
        },
    ) {
        events.push(event);
    }

    if events.is_empty() {
        return;
    }

    let mut state = paint_state.lock().expect("paint state mutex poisoned");
    for event in events {
        *state = reduce_paint_state(&state, &event);
    }
}

fn paint_event_from_shared_signal(
    world: &World,
    panel_query_root: ComponentId,
    signal: &Signal,
) -> Option<PaintEvent> {
    let event = match signal.event.as_ref()? {
        EventSignal::SelectionChanged {
            selection_root,
            selected_entries,
            selected_component,
            ..
        } => {
            let item = selected_entries.last().and_then(|entry| entry.item.clone());
            let component =
                selected_component.or_else(|| selected_entries.last().map(|entry| entry.component));
            let asset_selection_root =
                world.find_component(panel_query_root, ASSETS_SELECTION_SELECTOR);
            let tool_selection_root =
                world.find_component(panel_query_root, PAINT_TOOL_SELECTION_SELECTOR);
            let panel_layout_selection_root =
                world.find_component(panel_query_root, PANEL_LAYOUT_SELECTION_SELECTOR);
            let world_panel_selection_root =
                world.find_component(panel_query_root, WORLD_PANEL_SELECTION_SELECTOR);

            if asset_selection_root == Some(*selection_root) {
                Some(PaintEvent::AssetSelectionChanged { item, component })
            } else if tool_selection_root == Some(*selection_root) {
                Some(PaintEvent::ToolSelectionChanged {
                    tool: paint_tool_from_item(item.clone()),
                    item,
                    component,
                })
            } else if panel_layout_selection_root == Some(*selection_root) {
                Some(PaintEvent::PanelFocusChanged {
                    focused_panel: component,
                })
            } else if world_panel_selection_root == Some(*selection_root) {
                Some(PaintEvent::WorldPanelSelectionChanged {
                    component,
                    editor: component.and_then(|target| nearest_editor_ancestor(world, target)),
                })
            } else {
                println!(
                    "[PaintSystem][trace] ignored shared_selection_changed selection_root={selection_root:?} asset_selection_root={asset_selection_root:?} tool_selection_root={tool_selection_root:?} panel_layout_selection_root={panel_layout_selection_root:?} world_panel_selection_root={world_panel_selection_root:?} selected_entries={selected_entries:?} selected_component={selected_component:?}"
                );
                None
            }
        }
        _ => None,
    };

    if let Some(paint_event) = &event {
        println!(
            "[PaintSystem][trace] promoted_shared signal_scope={:?} signal={:?} paint_event={paint_event:?}",
            signal.scope, signal.event
        );
    }

    event
}

fn paint_event_from_editor_signal(
    world: &World,
    editor_root: ComponentId,
    panel_query_root: ComponentId,
    signal: &Signal,
) -> Option<PaintEvent> {
    let event = match signal.event.as_ref()? {
        EventSignal::SelectionChanged {
            selection_root,
            selected_component,
            ..
        } if *selection_root == editor_root => Some(PaintEvent::EditorSelectionChanged {
            editor: editor_root,
            component: *selected_component,
        }),
        EventSignal::Click {
            renderable,
            hit_point,
            ..
        } => eligible_scene_hit(world, editor_root, panel_query_root, *renderable).then_some(
            PaintEvent::SceneClick {
                editor: editor_root,
                renderable: *renderable,
                hit_point: *hit_point,
            },
        ),
        EventSignal::DragStart {
            renderable,
            hit_point,
            ..
        } => eligible_scene_hit(world, editor_root, panel_query_root, *renderable).then_some(
            PaintEvent::StrokeStarted {
                editor: editor_root,
                renderable: *renderable,
                hit_point: *hit_point,
            },
        ),
        EventSignal::DragMove {
            renderable,
            hit_point,
            ..
        } => eligible_scene_hit(world, editor_root, panel_query_root, *renderable).then_some(
            PaintEvent::StrokeMoved {
                editor: editor_root,
                renderable: *renderable,
                hit_point: *hit_point,
            },
        ),
        EventSignal::DragEnd { .. } => Some(PaintEvent::StrokeEnded {
            editor: editor_root,
        }),
        _ => None,
    };

    if let Some(paint_event) = &event {
        println!(
            "[PaintSystem][trace] promoted_editor editor_root={editor_root:?} signal_scope={:?} signal={:?} paint_event={paint_event:?}",
            signal.scope, signal.event
        );
    }

    event
}

fn apply_paint_side_effects(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    templates: &Arc<Mutex<Vec<PaintAssetTemplate>>>,
    workspace_state: &Arc<Mutex<SharedEditorWorkspaceState>>,
    _old_state: &PaintState,
    new_state: &PaintState,
    event: &PaintEvent,
    stroke_runtime: Option<&Arc<Mutex<PaintStrokeRuntime>>>,
) {
    let active_editor = resolve_active_editor(new_state, workspace_state);
    let mut status_override = None;
    let activity = paint_activity_status(world, active_editor, panel_query_root, new_state);
    println!(
        "[PaintSystem][trace] side_effects panel_query_root={panel_query_root:?} active_editor={active_editor:?} event={event:?} active={} reason={}",
        activity.active, activity.reason
    );

    let templates = templates
        .lock()
        .expect("paint templates mutex poisoned")
        .clone();

    match event {
        PaintEvent::SceneClick {
            editor,
            renderable,
            hit_point,
        } if Some(*editor) == active_editor => {
            status_override = handle_scene_click(
                world,
                emit,
                *editor,
                panel_query_root,
                &templates,
                new_state,
                stroke_runtime,
                *renderable,
                *hit_point,
            );
        }
        PaintEvent::StrokeStarted {
            editor,
            renderable,
            hit_point,
        } if Some(*editor) == active_editor => {
            let _ = hit_point;
            if let Some(runtime) = stroke_runtime {
                let mut runtime = runtime.lock().expect("paint stroke runtime mutex poisoned");
                if resolve_paint_context(world, *editor, panel_query_root, new_state, &templates)
                    .is_some()
                {
                    *runtime = PaintStrokeRuntime {
                        active: true,
                        captured_renderable: Some(*renderable),
                        non_grid_placed: false,
                        last_grid_step: None,
                    };
                } else {
                    *runtime = PaintStrokeRuntime::default();
                }
            }
        }
        PaintEvent::StrokeMoved {
            editor,
            renderable,
            hit_point,
        } if Some(*editor) == active_editor => {
            status_override = handle_stroke_move(
                world,
                emit,
                *editor,
                panel_query_root,
                &templates,
                new_state,
                stroke_runtime,
                *renderable,
                *hit_point,
            );
        }
        PaintEvent::StrokeEnded { .. } => {
            if let Some(runtime) = stroke_runtime {
                *runtime.lock().expect("paint stroke runtime mutex poisoned") =
                    PaintStrokeRuntime::default();
            }
        }
        _ => {}
    }

    update_paint_status(
        world,
        emit,
        active_editor,
        panel_query_root,
        new_state,
        status_override,
    );
}

fn resolve_active_editor(
    paint_state: &PaintState,
    workspace_state: &Arc<Mutex<SharedEditorWorkspaceState>>,
) -> Option<ComponentId> {
    paint_state.active_editor.or_else(|| {
        workspace_state
            .lock()
            .expect("paint workspace mutex poisoned")
            .default_active_editor()
    })
}

fn ensure_default_active_editor(
    paint_state: &Arc<Mutex<PaintState>>,
    workspace_state: &Arc<Mutex<SharedEditorWorkspaceState>>,
) {
    let default_editor = workspace_state
        .lock()
        .expect("paint workspace mutex poisoned")
        .default_active_editor();
    let Some(default_editor) = default_editor else {
        return;
    };

    let mut state = paint_state.lock().expect("paint state mutex poisoned");
    if state.active_editor.is_none() {
        *state = reduce_paint_state(
            &state,
            &PaintEvent::ActiveEditorChanged {
                editor: Some(default_editor),
            },
        );
    }
}

fn handle_scene_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    panel_query_root: ComponentId,
    templates: &[PaintAssetTemplate],
    paint_state: &PaintState,
    stroke_runtime: Option<&Arc<Mutex<PaintStrokeRuntime>>>,
    renderable: ComponentId,
    hit_point: [f32; 3],
) -> Option<String> {
    let context =
        resolve_paint_context(world, editor_root, panel_query_root, paint_state, templates)?;
    let runtime = stroke_runtime?;
    let mut runtime = runtime.lock().expect("paint stroke runtime mutex poisoned");
    if runtime.non_grid_placed {
        return None;
    }
    let grid_snap = context.grid_snap(hit_point);
    if let Some(snap) = grid_snap {
        runtime.last_grid_step = Some(snap.step);
    }
    runtime.non_grid_placed = true;
    Some(place_asset(
        world,
        emit,
        editor_root,
        renderable,
        hit_point,
        &context.asset,
        grid_snap,
    ))
}

fn handle_stroke_move(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    panel_query_root: ComponentId,
    templates: &[PaintAssetTemplate],
    paint_state: &PaintState,
    stroke_runtime: Option<&Arc<Mutex<PaintStrokeRuntime>>>,
    renderable: ComponentId,
    hit_point: [f32; 3],
) -> Option<String> {
    let context =
        resolve_paint_context(world, editor_root, panel_query_root, paint_state, templates)?;
    let runtime = stroke_runtime?;
    let mut runtime = runtime.lock().expect("paint stroke runtime mutex poisoned");
    if !runtime.active {
        return None;
    }
    if runtime
        .captured_renderable
        .is_some_and(|captured| captured != renderable)
    {
        return None;
    }

    match context.grid_snap(hit_point) {
        Some(grid_snap) => {
            if GridSystem::same_step(runtime.last_grid_step, grid_snap.step) {
                return None;
            }
            runtime.last_grid_step = Some(grid_snap.step);
            Some(place_asset(
                world,
                emit,
                editor_root,
                renderable,
                hit_point,
                &context.asset,
                Some(grid_snap),
            ))
        }
        None => {
            if runtime.non_grid_placed {
                return None;
            }
            runtime.non_grid_placed = true;
            Some(place_asset(
                world,
                emit,
                editor_root,
                renderable,
                hit_point,
                &context.asset,
                None,
            ))
        }
    }
}

#[derive(Debug, Clone)]
struct PaintContext {
    asset: PaintAssetTemplate,
    active_grid: Option<crate::engine::ecs::system::grid_system::ActiveGrid>,
}

impl PaintContext {
    fn grid_snap(&self, hit_point: [f32; 3]) -> Option<GridSnapResult> {
        self.active_grid
            .as_ref()
            .map(|grid| GridSystem::snap_hit(grid, hit_point))
    }
}

fn resolve_paint_context(
    world: &World,
    editor_root: ComponentId,
    panel_query_root: ComponentId,
    paint_state: &PaintState,
    templates: &[PaintAssetTemplate],
) -> Option<PaintContext> {
    let paint_panel_root = world.find_component(panel_query_root, PAINT_PANEL_ROOT_SELECTOR);
    if !is_paint_active(paint_panel_root, paint_state) {
        return None;
    }
    let asset_title = paint_state.selected_asset.as_ref()?.item.as_ref()?;
    let asset = templates
        .iter()
        .find(|template| template.title == *asset_title)?
        .clone();
    Some(PaintContext {
        asset,
        active_grid: GridSystem::active_grid_for_editor(world, editor_root),
    })
}

struct PaintActivityStatus {
    active: bool,
    reason: String,
}

fn paint_activity_status(
    world: &World,
    active_editor: Option<ComponentId>,
    panel_query_root: ComponentId,
    paint_state: &PaintState,
) -> PaintActivityStatus {
    if paint_state
        .selected_asset
        .as_ref()
        .and_then(|selection| selection.item.as_ref())
        .is_none()
    {
        return PaintActivityStatus {
            active: false,
            reason: "no asset selected".to_string(),
        };
    }
    let paint_panel_root = world.find_component(panel_query_root, PAINT_PANEL_ROOT_SELECTOR);
    if !is_paint_panel_focused(paint_panel_root, paint_state) {
        return PaintActivityStatus {
            active: false,
            reason: "focus Paint panel".to_string(),
        };
    }
    if paint_state.selected_tool != PaintTool::FreeDraw {
        return PaintActivityStatus {
            active: false,
            reason: format!("tool is not Free Draw ({:?})", paint_state.selected_tool),
        };
    }
    let Some(editor_root) = active_editor else {
        return PaintActivityStatus {
            active: false,
            reason: "no active editor".to_string(),
        };
    };
    if let Some(grid) = GridSystem::active_grid_for_editor(world, editor_root) {
        return PaintActivityStatus {
            active: true,
            reason: format!("grid active @ {:.2}", grid.spacing),
        };
    }
    PaintActivityStatus {
        active: true,
        reason: "grid inactive".to_string(),
    }
}

fn place_asset(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    target_renderable: ComponentId,
    hit_point: [f32; 3],
    asset: &PaintAssetTemplate,
    grid_snap: Option<GridSnapResult>,
) -> String {
    let scene_parent = resolve_scene_parent(world, editor_root);

    let asset_root = match MeowMeowRunner::spawn_mms_module_component_uninitialized(
        &asset.module,
        &asset.export_name,
        default_asset_args(asset),
        world,
        emit,
    ) {
        Ok(asset_root) => asset_root,
        Err(error) => return format!("paint failed: asset spawn error: {error}"),
    };

    let pose =
        match resolve_placement_pose(world, target_renderable, hit_point, asset_root, grid_snap) {
            Ok(pose) => pose,
            Err(PlacementError::UnsupportedSurface) => {
                let _ = world.remove_component_subtree(asset_root);
                return "paint inactive: unsupported target surface".to_string();
            }
            Err(PlacementError::MissingAssetBounds) => {
                let _ = world.remove_component_subtree(asset_root);
                return "paint failed: asset bounds unavailable".to_string();
            }
            Err(PlacementError::MissingTargetTransform) => {
                let _ = world.remove_component_subtree(asset_root);
                return "paint failed: target transform unavailable".to_string();
            }
        };

    let raycastable_root = world.add_component_boxed_named(
        "painted_asset_raycastable",
        Box::new(RaycastableComponent::enabled()),
    );
    let wrapper = world.add_component_boxed_named(
        "painted_asset_root",
        Box::new(
            TransformComponent::new()
                .with_position(
                    pose.translation[0],
                    pose.translation[1],
                    pose.translation[2],
                )
                .with_rotation_quat(pose.rotation),
        ),
    );
    let _ = world.add_child(raycastable_root, wrapper);
    let _ = world.add_child(wrapper, asset_root);
    world.init_component_tree(raycastable_root, emit);
    emit.push_intent_now(
        raycastable_root,
        IntentValue::Attach {
            parents: vec![scene_parent],
            child: raycastable_root,
        },
    );

    let grid_text = if grid_snap.is_some() {
        "grid active"
    } else {
        "grid inactive"
    };
    format!("paint placed: {} | {}", asset.title, grid_text)
}

fn default_asset_args(asset: &PaintAssetTemplate) -> Vec<Value> {
    asset
        .param_names
        .iter()
        .map(|name| {
            let lower_name = name.to_lowercase();
            if lower_name.contains("color") {
                Value::Array(vec![
                    Value::Number(0.5),
                    Value::Number(0.5),
                    Value::Number(0.5),
                    Value::Number(1.0),
                ])
            } else if lower_name.contains("items") || lower_name.contains("sequence") {
                Value::Array(Vec::new())
            } else if lower_name.contains("path")
                || lower_name.contains("url")
                || lower_name.contains("uri")
            {
                Value::String("assets/world/default.mms".to_string())
            } else if lower_name.contains("title")
                || lower_name.contains("label")
                || lower_name.contains("name")
                || lower_name.contains("text")
            {
                Value::String("Paint".to_string())
            } else {
                Value::Null
            }
        })
        .collect()
}

fn update_paint_status(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    active_editor: Option<ComponentId>,
    panel_query_root: ComponentId,
    paint_state: &PaintState,
    override_text: Option<String>,
) {
    let Some(paint_panel_root) = world.find_component(panel_query_root, PAINT_PANEL_ROOT_SELECTOR)
    else {
        println!(
            "[PaintSystem][trace] status_skip panel_query_root={panel_query_root:?} reason=missing paint panel root"
        );
        return;
    };
    let Some(status_wrap) = world.find_component(paint_panel_root, PAINT_STATUS_WRAP_SELECTOR)
    else {
        println!(
            "[PaintSystem][trace] status_skip paint_panel_root={paint_panel_root:?} reason=missing status wrap"
        );
        return;
    };
    let text = override_text
        .unwrap_or_else(|| base_status_text(world, active_editor, panel_query_root, paint_state));
    println!(
        "[PaintSystem][trace] status_update paint_panel_root={paint_panel_root:?} active_editor={active_editor:?} text={text:?}"
    );
    set_status_text(world, emit, status_wrap, &text);
}

fn base_status_text(
    world: &World,
    active_editor: Option<ComponentId>,
    panel_query_root: ComponentId,
    paint_state: &PaintState,
) -> String {
    if paint_state
        .selected_asset
        .as_ref()
        .and_then(|selection| selection.item.as_ref())
        .is_none()
    {
        return "paint inactive: no asset selected".to_string();
    }
    let paint_panel_root = world.find_component(panel_query_root, PAINT_PANEL_ROOT_SELECTOR);
    if !is_paint_panel_focused(paint_panel_root, paint_state) {
        return "paint inactive: focus Paint panel".to_string();
    }
    if paint_state.selected_tool != PaintTool::FreeDraw {
        return "paint inactive: tool is not Free Draw".to_string();
    }
    let Some(editor_root) = active_editor else {
        return "paint inactive: no active editor".to_string();
    };
    if let Some(grid) = GridSystem::active_grid_for_editor(world, editor_root) {
        return format!("paint active | grid active @ {:.2}", grid.spacing);
    }
    "paint active | grid inactive".to_string()
}

fn set_status_text(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    status_wrap: ComponentId,
    text: &str,
) {
    let Some(status_text) = world.find_component(status_wrap, PANEL_STATUS_VALUE_SELECTOR) else {
        println!(
            "[PaintSystem][trace] status_skip status_wrap={status_wrap:?} reason=missing status text"
        );
        return;
    };
    let Some(text_component) = world
        .get_component_by_id_as_mut::<crate::engine::ecs::component::TextComponent>(status_text)
    else {
        println!(
            "[PaintSystem][trace] status_skip status_text={status_text:?} reason=missing TextComponent"
        );
        return;
    };
    if text_component.text == text {
        println!("[PaintSystem][trace] status_skip status_text={status_text:?} reason=unchanged");
        return;
    }
    text_component.text = text.to_string();
    text_component.mark_unbuilt();
    emit.push_intent_now(
        status_text,
        IntentValue::SetText {
            component_ids: vec![status_text],
            text: text.to_string(),
        },
    );
}

fn resolve_scene_parent(world: &World, editor_root: ComponentId) -> ComponentId {
    world
        .children_of(editor_root)
        .iter()
        .copied()
        .find(|&child| {
            world.component_label(child) != Some(RUNTIME_UI_ROOT_NAME)
                && world.component_label(child) != Some("editor_gizmo_anchor")
        })
        .unwrap_or(editor_root)
}

fn eligible_scene_hit(
    world: &World,
    editor_root: ComponentId,
    panel_query_root: ComponentId,
    renderable: ComponentId,
) -> bool {
    if nearest_editor_ancestor(world, renderable) != Some(editor_root) {
        return false;
    }
    if is_descendant_or_self(world, panel_query_root, renderable) {
        return false;
    }
    if has_selectable_off_ancestor(world, renderable) {
        return false;
    }
    if has_transform_gizmo_ancestor(world, renderable) {
        return false;
    }
    true
}

fn nearest_editor_ancestor(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut cur = Some(start);
    while let Some(node) = cur {
        if world
            .get_component_by_id_as::<EditorComponent>(node)
            .is_some()
        {
            return Some(node);
        }
        cur = world.parent_of(node);
    }
    None
}

fn has_selectable_off_ancestor(world: &World, start: ComponentId) -> bool {
    let mut cur = Some(start);
    while let Some(node) = cur {
        if world
            .get_component_by_id_as::<SelectableComponent>(node)
            .map(|selectable| !selectable.enabled)
            .unwrap_or(false)
        {
            return true;
        }
        cur = world.parent_of(node);
    }
    false
}

fn has_transform_gizmo_ancestor(world: &World, start: ComponentId) -> bool {
    let mut cur = Some(start);
    while let Some(node) = cur {
        if world
            .get_component_by_id_as::<TransformGizmoComponent>(node)
            .is_some()
        {
            return true;
        }
        cur = world.parent_of(node);
    }
    false
}

fn is_descendant_or_self(world: &World, ancestor: ComponentId, node: ComponentId) -> bool {
    let mut current = Some(node);
    while let Some(component) = current {
        if component == ancestor {
            return true;
        }
        current = world.parent_of(component);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::command_queue::CommandQueue;
    use crate::engine::ecs::component::{
        ColorComponent, GridComponent, RenderableComponent, SelectionComponent,
    };
    use crate::engine::ecs::system::SystemWorld;
    use crate::engine::graphics::{RenderAssets, VisualWorld};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_asset_directory() -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let tmp_dir = std::env::temp_dir().join(format!("cat_engine_paint_assets_{}", now));
        std::fs::create_dir_all(&tmp_dir).expect("create temp dir");
        tmp_dir
    }

    fn write_test_asset(dir: &std::path::Path) {
        std::fs::write(
            dir.join("paint_asset.mms"),
            r#"
                export fn cube_stamp() {
                    return T {
                        T.scale(0.2, 0.2, 0.2) {
                            C.rgba(1.0, 0.2, 0.2, 1.0) {
                                Renderable.cube()
                            }
                        }
                    }
                }
            "#,
        )
        .expect("write asset");
    }

    fn find_named_root(world: &World, name: &str) -> ComponentId {
        world
            .all_components()
            .find(|&component_id| {
                world.parent_of(component_id).is_none()
                    && world.component_label(component_id) == Some(name)
            })
            .expect("named root")
    }

    fn push_click(systems: &mut SystemWorld, renderable: ComponentId) {
        systems.rx.push_event(
            renderable,
            EventSignal::Click {
                raycaster: renderable,
                renderable,
                hit_point: [0.0, 0.0, 0.5],
                screen_pos_px: None,
            },
        );
    }

    fn count_named_descendants(world: &World, root: ComponentId, name: &str) -> usize {
        let mut count = 0;
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if world.component_label(node) == Some(name) {
                count += 1;
            }
            for &child in world.children_of(node) {
                stack.push(child);
            }
        }
        count
    }

    fn push_asset_and_panel_focus(
        world: &World,
        systems: &mut SystemWorld,
        paint_panel_root: ComponentId,
    ) {
        let asset_item = world
            .find_component(
                find_named_root(world, RUNTIME_UI_ROOT_NAME),
                "[name='asset_item']",
            )
            .expect("asset item");
        systems.rx.push_event(
            asset_item,
            EventSignal::Click {
                raycaster: asset_item,
                renderable: asset_item,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );
        systems.rx.push_event(
            paint_panel_root,
            EventSignal::Click {
                raycaster: paint_panel_root,
                renderable: paint_panel_root,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        );
    }

    fn init_editor_fixture() -> (
        World,
        CommandQueue,
        VisualWorld,
        SystemWorld,
        RenderAssets,
        ComponentId,
        ComponentId,
        ComponentId,
        ComponentId,
    ) {
        let tmp_dir = temp_asset_directory();
        write_test_asset(&tmp_dir);

        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let render_assets = RenderAssets::new();

        systems
            .asset_system
            .scan_assets_dir(&tmp_dir)
            .expect("scan");
        systems.selection.install_handlers(&mut systems.rx);

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let target = world.add_component_boxed_named("target", Box::new(TransformComponent::new()));
        let color = world.add_component(ColorComponent::rgba(0.7, 0.7, 0.7, 1.0));
        let renderable = world.add_component(RenderableComponent::cube());
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(scene_root, target);
        let _ = world.add_child(target, color);
        let _ = world.add_child(color, renderable);

        world.init_component_tree(editor_root, &mut emit);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        let runtime_ui_root = find_named_root(&world, RUNTIME_UI_ROOT_NAME);
        let paint_panel_root = world
            .find_component(runtime_ui_root, PAINT_PANEL_ROOT_SELECTOR)
            .expect("paint panel");
        let assets_selection = world
            .find_component(runtime_ui_root, ASSETS_SELECTION_SELECTOR)
            .expect("assets selection");

        push_asset_and_panel_focus(&world, &mut systems, paint_panel_root);
        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);
        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        assert!(world
            .get_component_by_id_as::<SelectionComponent>(assets_selection)
            .expect("selection")
            .selected_item
            .is_some());

        (
            world,
            emit,
            visuals,
            systems,
            render_assets,
            editor_root,
            scene_root,
            renderable,
            paint_panel_root,
        )
    }

    #[test]
    fn paint_inactive_without_asset_selection() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let render_assets = RenderAssets::new();

        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root =
            world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
        let target = world.add_component_boxed_named("target", Box::new(TransformComponent::new()));
        let color = world.add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
        let renderable = world.add_component(RenderableComponent::cube());
        let _ = world.add_child(editor_root, scene_root);
        let _ = world.add_child(scene_root, target);
        let _ = world.add_child(target, color);
        let _ = world.add_child(color, renderable);

        world.init_component_tree(editor_root, &mut emit);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        push_click(&mut systems, renderable);
        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        assert_eq!(
            count_named_descendants(&world, editor_root, "painted_asset_root"),
            0
        );
    }

    #[test]
    fn paint_places_when_focused_and_asset_selected() {
        let (
            mut world,
            mut emit,
            mut visuals,
            mut systems,
            render_assets,
            editor_root,
            _scene_root,
            renderable,
            _paint_panel_root,
        ) = init_editor_fixture();

        push_click(&mut systems, renderable);
        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        assert_eq!(
            count_named_descendants(&world, editor_root, "painted_asset_root"),
            1
        );
    }

    #[test]
    fn painted_assets_get_raycastable_ancestor() {
        let (
            mut world,
            mut emit,
            mut visuals,
            mut systems,
            render_assets,
            editor_root,
            _scene_root,
            renderable,
            _paint_panel_root,
        ) = init_editor_fixture();

        push_click(&mut systems, renderable);
        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        let painted_root = world
            .find_component(editor_root, "[name='painted_asset_root']")
            .expect("painted asset root");
        let painted_renderable = world
            .find_component(painted_root, "Renderable")
            .expect("painted renderable");

        assert!(
            crate::engine::ecs::system::BvhSystem::renderable_is_raycastable(
                &world,
                painted_renderable
            ),
            "expected painted asset renderables to inherit a raycastable ancestor"
        );
    }

    #[test]
    fn non_grid_drag_places_once_per_gesture() {
        let (
            mut world,
            mut emit,
            mut visuals,
            mut systems,
            render_assets,
            editor_root,
            _scene_root,
            renderable,
            _paint_panel_root,
        ) = init_editor_fixture();

        systems.rx.push_event(
            renderable,
            EventSignal::DragStart {
                raycaster: renderable,
                renderable,
                hit_point: [0.0, 0.0, 0.5],
                ray_dir_world: [0.0, 0.0, -1.0],
                screen_pos_px: None,
            },
        );
        systems.rx.push_event(
            renderable,
            EventSignal::DragMove {
                raycaster: renderable,
                renderable,
                hit_point: [0.1, 0.0, 0.5],
                delta_world: [0.1, 0.0, 0.0],
                screen_pos_px: None,
                screen_delta_px: None,
            },
        );
        systems.rx.push_event(
            renderable,
            EventSignal::DragMove {
                raycaster: renderable,
                renderable,
                hit_point: [0.2, 0.0, 0.5],
                delta_world: [0.1, 0.0, 0.0],
                screen_pos_px: None,
                screen_delta_px: None,
            },
        );
        systems.rx.push_event(
            renderable,
            EventSignal::DragEnd {
                raycaster: renderable,
                renderable,
                hit_point: Some([0.2, 0.0, 0.5]),
            },
        );
        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        assert_eq!(
            count_named_descendants(&world, editor_root, "painted_asset_root"),
            1
        );
    }

    #[test]
    fn grid_drag_keeps_painting_scoped_to_one_editor() {
        let (
            mut world,
            mut emit,
            mut visuals,
            mut systems,
            render_assets,
            editor_root,
            scene_root,
            renderable,
            _paint_panel_root,
        ) = init_editor_fixture();

        let grid = world.add_component_boxed_named("grid", Box::new(GridComponent::new(0.5)));
        let _ = world.add_child(scene_root, grid);
        world.init_component_tree(grid, &mut emit);
        world
            .get_component_by_id_as_mut::<EditorComponent>(editor_root)
            .expect("editor")
            .selected = Some(grid);

        systems.rx.push_event(
            renderable,
            EventSignal::DragStart {
                raycaster: renderable,
                renderable,
                hit_point: [0.0, 0.0, 0.5],
                ray_dir_world: [0.0, 0.0, -1.0],
                screen_pos_px: None,
            },
        );
        systems.rx.push_event(
            renderable,
            EventSignal::DragMove {
                raycaster: renderable,
                renderable,
                hit_point: [0.1, 0.1, 0.5],
                delta_world: [0.1, 0.1, 0.0],
                screen_pos_px: None,
                screen_delta_px: None,
            },
        );
        systems.rx.push_event(
            renderable,
            EventSignal::DragMove {
                raycaster: renderable,
                renderable,
                hit_point: [0.15, 0.1, 0.5],
                delta_world: [0.05, 0.0, 0.0],
                screen_pos_px: None,
                screen_delta_px: None,
            },
        );
        systems.rx.push_event(
            renderable,
            EventSignal::DragMove {
                raycaster: renderable,
                renderable,
                hit_point: [0.6, 0.1, 0.5],
                delta_world: [0.45, 0.0, 0.0],
                screen_pos_px: None,
                screen_delta_px: None,
            },
        );
        systems.rx.push_event(
            renderable,
            EventSignal::DragEnd {
                raycaster: renderable,
                renderable,
                hit_point: Some([0.6, 0.1, 0.5]),
            },
        );
        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        assert_eq!(
            count_named_descendants(&world, editor_root, "painted_asset_root"),
            1
        );
    }

    #[test]
    fn shared_ui_state_routes_paint_to_latest_scene_interaction_editor() {
        let tmp_dir = temp_asset_directory();
        write_test_asset(&tmp_dir);

        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let render_assets = RenderAssets::new();

        systems
            .asset_system
            .scan_assets_dir(&tmp_dir)
            .expect("scan");
        systems.selection.install_handlers(&mut systems.rx);

        let editor_a =
            world.add_component_boxed_named("editor_a", Box::new(EditorComponent::new()));
        let scene_a =
            world.add_component_boxed_named("scene_a", Box::new(TransformComponent::new()));
        let target_a =
            world.add_component_boxed_named("target_a", Box::new(TransformComponent::new()));
        let color_a = world.add_component(ColorComponent::rgba(0.7, 0.7, 0.7, 1.0));
        let renderable_a = world.add_component(RenderableComponent::cube());
        let _ = world.add_child(editor_a, scene_a);
        let _ = world.add_child(scene_a, target_a);
        let _ = world.add_child(target_a, color_a);
        let _ = world.add_child(color_a, renderable_a);

        let editor_b =
            world.add_component_boxed_named("editor_b", Box::new(EditorComponent::new()));
        let scene_b =
            world.add_component_boxed_named("scene_b", Box::new(TransformComponent::new()));
        let target_b =
            world.add_component_boxed_named("target_b", Box::new(TransformComponent::new()));
        let color_b = world.add_component(ColorComponent::rgba(0.7, 0.7, 0.7, 1.0));
        let renderable_b = world.add_component(RenderableComponent::cube());
        let _ = world.add_child(editor_b, scene_b);
        let _ = world.add_child(scene_b, target_b);
        let _ = world.add_child(target_b, color_b);
        let _ = world.add_child(color_b, renderable_b);

        world.init_component_tree(editor_a, &mut emit);
        world.init_component_tree(editor_b, &mut emit);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        let runtime_ui_root = find_named_root(&world, RUNTIME_UI_ROOT_NAME);
        let paint_panel_root = world
            .find_component(runtime_ui_root, PAINT_PANEL_ROOT_SELECTOR)
            .expect("paint panel");

        push_asset_and_panel_focus(&world, &mut systems, paint_panel_root);
        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);
        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        push_click(&mut systems, renderable_b);
        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);
        systems.process_commands(&mut world, &mut visuals, &render_assets, &mut emit);

        assert_eq!(
            count_named_descendants(&world, editor_a, "painted_asset_root"),
            0
        );
        assert_eq!(
            count_named_descendants(&world, editor_b, "painted_asset_root"),
            1
        );
    }
}
