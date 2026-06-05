use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::engine::ecs::component::{
    EditorComponent, SelectableComponent, SelectionComponent, TransformComponent,
    TransformGizmoComponent,
};
use crate::engine::ecs::system::grid_system::{GridSnapResult, GridStep, GridSystem};
use crate::engine::ecs::system::paint_placement::{resolve_placement_pose, PlacementError};
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, RxWorld, Signal, SignalEmitter, SignalKind, World,
};
use crate::meow_meow::object::Value;
use crate::meow_meow::runner::{LoadedMmsModule, MeowMeowRunner};

const PANEL_LAYOUT_SELECTION_SELECTOR: &str = "#editor_panel_layout_selection";
const PAINT_PANEL_ROOT_SELECTOR: &str = "#paint_panel_root";
const ASSETS_SELECTION_SELECTOR: &str = "#assets_selection";
const PAINT_TOOL_SELECTION_SELECTOR: &str = "#paint_tool_selection";
const PAINT_STATUS_WRAP_SELECTOR: &str = "#paint_status_wrap";
const PANEL_STATUS_VALUE_SELECTOR: &str = "#panel_status_value";
const RUNTIME_UI_ROOT_NAME: &str = "editor_runtime_ui_root";
const FREE_DRAW_LABEL: &str = "Free Draw";
const LINE_LABEL: &str = "Line";
const SPRAY_CAN_LABEL: &str = "Spray Can";
const FILL_LABEL: &str = "Fill";
const ERASE_LABEL: &str = "Erase";

#[derive(Debug, Clone)]
pub struct PaintAssetTemplate {
    pub title: String,
    pub module: LoadedMmsModule,
    pub export_name: String,
    pub param_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PaintState {
    selected_asset: Option<PaintSelection>,
    selected_tool: PaintTool,
    focused_panel: Option<ComponentId>,
    stroke: PaintStrokeMode,
}

impl Default for PaintState {
    fn default() -> Self {
        Self {
            selected_asset: None,
            selected_tool: PaintTool::Unknown(None),
            focused_panel: None,
            stroke: PaintStrokeMode::Idle,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PaintSelection {
    item: Option<String>,
    component: Option<ComponentId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PaintTool {
    FreeDraw,
    Line,
    SprayCan,
    Fill,
    Erase,
    Unknown(Option<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaintStrokeMode {
    Idle,
    Dragging,
}

#[derive(Debug, Clone, Default)]
struct PaintStrokeRuntime {
    active: bool,
    captured_renderable: Option<ComponentId>,
    non_grid_placed: bool,
    last_grid_step: Option<GridStep>,
}

#[derive(Debug, Clone)]
enum PaintEvent {
    AssetSelectionChanged {
        item: Option<String>,
        component: Option<ComponentId>,
    },
    ToolSelectionChanged {
        tool: PaintTool,
        item: Option<String>,
        component: Option<ComponentId>,
    },
    PanelFocusChanged {
        focused_panel: Option<ComponentId>,
    },
    SceneClick {
        renderable: ComponentId,
        hit_point: [f32; 3],
    },
    StrokeStarted {
        renderable: ComponentId,
        hit_point: [f32; 3],
    },
    StrokeMoved {
        renderable: ComponentId,
        hit_point: [f32; 3],
    },
    StrokeEnded,
}

#[derive(Debug, Default)]
pub struct PaintSystem {
    installed_editor_roots: HashSet<ComponentId>,
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
        if self.installed_editor_roots.contains(&editor_root) {
            return;
        }
        self.installed_editor_roots.insert(editor_root);
        println!(
            "[PaintSystem][trace] install editor_root={editor_root:?} panel_query_root={panel_query_root:?}"
        );

        let _ = world;
        let paint_state = Arc::new(Mutex::new(PaintState::default()));
        let stroke_runtime = Arc::new(Mutex::new(PaintStrokeRuntime::default()));

        let selection_state = Arc::clone(&paint_state);
        let selection_runtime = Arc::clone(&stroke_runtime);
        let selection_templates = templates.clone();
        rx.add_handler_closure(
            SignalKind::SelectionChanged,
            panel_query_root,
            move |world, emit, signal| {
                let Some(event) =
                    paint_event_from_signal(world, editor_root, panel_query_root, signal)
                else {
                    return;
                };
                handle_paint_event(
                    world,
                    emit,
                    editor_root,
                    panel_query_root,
                    &selection_templates,
                    &selection_state,
                    &selection_runtime,
                    &event,
                );
            },
        );

        bootstrap_paint_state(world, panel_query_root, &paint_state);

        for signal_kind in [
            SignalKind::Click,
            SignalKind::DragStart,
            SignalKind::DragMove,
            SignalKind::DragEnd,
        ] {
            let state = Arc::clone(&paint_state);
            let runtime = Arc::clone(&stroke_runtime);
            let templates = templates.clone();
            rx.add_handler_closure(signal_kind, editor_root, move |world, emit, env| {
                let Some(event) =
                    paint_event_from_signal(world, editor_root, panel_query_root, env)
                else {
                    return;
                };
                handle_paint_event(
                    world,
                    emit,
                    editor_root,
                    panel_query_root,
                    &templates,
                    &state,
                    &runtime,
                    &event,
                );
            });
        }
    }
}

fn handle_paint_event(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    panel_query_root: ComponentId,
    templates: &[PaintAssetTemplate],
    paint_state: &Arc<Mutex<PaintState>>,
    stroke_runtime: &Arc<Mutex<PaintStrokeRuntime>>,
    event: &PaintEvent,
) {
    let (old_state, new_state) = {
        let mut state = paint_state.lock().expect("paint state mutex poisoned");
        let old_state = state.clone();
        let new_state = reduce_paint_state(&old_state, event);
        println!(
            "[PaintSystem][trace] reduce editor_root={editor_root:?} panel_query_root={panel_query_root:?} old_state={old_state:?} event={event:?} new_state={new_state:?}"
        );
        *state = new_state.clone();
        (old_state, new_state)
    };

    apply_paint_side_effects(
        world,
        emit,
        editor_root,
        panel_query_root,
        templates,
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

fn paint_event_from_signal(
    world: &World,
    editor_root: ComponentId,
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
            let asset_selection_root = world.find_component(panel_query_root, ASSETS_SELECTION_SELECTOR);
            let tool_selection_root =
                world.find_component(panel_query_root, PAINT_TOOL_SELECTION_SELECTOR);
            let panel_layout_selection_root =
                world.find_component(panel_query_root, PANEL_LAYOUT_SELECTION_SELECTOR);

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
            } else {
                println!(
                    "[PaintSystem][trace] ignored selection_changed selection_root={selection_root:?} asset_selection_root={asset_selection_root:?} tool_selection_root={tool_selection_root:?} panel_layout_selection_root={panel_layout_selection_root:?} selected_entries={selected_entries:?} selected_component={selected_component:?}"
                );
                None
            }
        }
        EventSignal::Click {
            renderable,
            hit_point,
            ..
        } => eligible_scene_hit(world, editor_root, panel_query_root, *renderable).then_some(
            PaintEvent::SceneClick {
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
                renderable: *renderable,
                hit_point: *hit_point,
            },
        ),
        EventSignal::DragEnd { .. } => Some(PaintEvent::StrokeEnded),
        _ => None,
    };

    if let Some(paint_event) = &event {
        println!(
            "[PaintSystem][trace] promoted signal_scope={:?} signal={:?} paint_event={paint_event:?}",
            signal.scope, signal.event
        );
    }

    event
}

fn reduce_paint_state(old: &PaintState, event: &PaintEvent) -> PaintState {
    let mut new = old.clone();
    match event {
        PaintEvent::AssetSelectionChanged { item, component } => {
            new.selected_asset = Some(PaintSelection {
                item: item.clone(),
                component: *component,
            });
        }
        PaintEvent::ToolSelectionChanged {
            tool,
            item,
            component,
        } => {
            let _ = (item, component);
            new.selected_tool = tool.clone();
        }
        PaintEvent::PanelFocusChanged { focused_panel } => {
            new.focused_panel = *focused_panel;
        }
        PaintEvent::StrokeStarted { .. } => {
            new.stroke = PaintStrokeMode::Dragging;
        }
        PaintEvent::StrokeEnded => {
            new.stroke = PaintStrokeMode::Idle;
        }
        PaintEvent::SceneClick { .. } | PaintEvent::StrokeMoved { .. } => {}
    }
    new
}

fn apply_paint_side_effects(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    panel_query_root: ComponentId,
    templates: &[PaintAssetTemplate],
    _old_state: &PaintState,
    new_state: &PaintState,
    event: &PaintEvent,
    stroke_runtime: &Arc<Mutex<PaintStrokeRuntime>>,
) {
    let mut status_override = None;
    let activity = paint_activity_status(world, editor_root, panel_query_root, new_state);
    println!(
        "[PaintSystem][trace] side_effects editor_root={editor_root:?} panel_query_root={panel_query_root:?} event={event:?} active={} reason={}",
        activity.active, activity.reason
    );

    match event {
        PaintEvent::SceneClick {
            renderable,
            hit_point,
        } => {
            status_override = handle_scene_click(
                world,
                emit,
                editor_root,
                panel_query_root,
                templates,
                new_state,
                stroke_runtime,
                *renderable,
                *hit_point,
            );
        }
        PaintEvent::StrokeStarted {
            renderable,
            hit_point,
        } => {
            let _ = hit_point;
            let mut runtime = stroke_runtime
                .lock()
                .expect("paint stroke runtime mutex poisoned");
            if resolve_paint_context(world, editor_root, panel_query_root, new_state, templates)
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
        PaintEvent::StrokeMoved {
            renderable,
            hit_point,
        } => {
            status_override = handle_stroke_move(
                world,
                emit,
                editor_root,
                panel_query_root,
                templates,
                new_state,
                stroke_runtime,
                *renderable,
                *hit_point,
            );
        }
        PaintEvent::StrokeEnded => {
            *stroke_runtime
                .lock()
                .expect("paint stroke runtime mutex poisoned") = PaintStrokeRuntime::default();
        }
        PaintEvent::AssetSelectionChanged { .. }
        | PaintEvent::ToolSelectionChanged { .. }
        | PaintEvent::PanelFocusChanged { .. } => {}
    }

    update_paint_status(
        world,
        emit,
        editor_root,
        panel_query_root,
        new_state,
        status_override,
    );
}

fn handle_scene_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    panel_query_root: ComponentId,
    templates: &[PaintAssetTemplate],
    paint_state: &PaintState,
    stroke_runtime: &Arc<Mutex<PaintStrokeRuntime>>,
    renderable: ComponentId,
    hit_point: [f32; 3],
) -> Option<String> {
    let context =
        resolve_paint_context(world, editor_root, panel_query_root, paint_state, templates)?;
    let mut runtime = stroke_runtime
        .lock()
        .expect("paint stroke runtime mutex poisoned");
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
    stroke_runtime: &Arc<Mutex<PaintStrokeRuntime>>,
    renderable: ComponentId,
    hit_point: [f32; 3],
) -> Option<String> {
    let context =
        resolve_paint_context(world, editor_root, panel_query_root, paint_state, templates)?;
    let mut runtime = stroke_runtime
        .lock()
        .expect("paint stroke runtime mutex poisoned");
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

fn paint_tool_from_item(item: Option<String>) -> PaintTool {
    match item.as_deref() {
        Some(FREE_DRAW_LABEL) => PaintTool::FreeDraw,
        Some(LINE_LABEL) => PaintTool::Line,
        Some(SPRAY_CAN_LABEL) => PaintTool::SprayCan,
        Some(FILL_LABEL) => PaintTool::Fill,
        Some(ERASE_LABEL) => PaintTool::Erase,
        _ => PaintTool::Unknown(item),
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
    if !is_paint_active(world, panel_query_root, paint_state) {
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
    editor_root: ComponentId,
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
    if !is_paint_panel_focused(world, panel_query_root, paint_state) {
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

fn is_paint_active(world: &World, panel_query_root: ComponentId, paint_state: &PaintState) -> bool {
    is_paint_panel_focused(world, panel_query_root, paint_state)
        && paint_state.selected_tool == PaintTool::FreeDraw
        && paint_state
            .selected_asset
            .as_ref()
            .and_then(|selection| selection.item.as_ref())
            .is_some()
}

fn is_paint_panel_focused(
    world: &World,
    panel_query_root: ComponentId,
    paint_state: &PaintState,
) -> bool {
    world
        .find_component(panel_query_root, PAINT_PANEL_ROOT_SELECTOR)
        .is_some_and(|paint_panel_root| paint_state.focused_panel == Some(paint_panel_root))
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
    let _ = world.add_child(wrapper, asset_root);
    world.init_component_tree(wrapper, emit);
    emit.push_intent_now(
        wrapper,
        IntentValue::Attach {
            parents: vec![scene_parent],
            child: wrapper,
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
    editor_root: ComponentId,
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
        .unwrap_or_else(|| base_status_text(world, editor_root, panel_query_root, paint_state));
    println!(
        "[PaintSystem][trace] status_update paint_panel_root={paint_panel_root:?} text={text:?}"
    );
    set_status_text(world, emit, status_wrap, &text);
}

fn base_status_text(
    world: &World,
    editor_root: ComponentId,
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
    if !is_paint_panel_focused(world, panel_query_root, paint_state) {
        return "paint inactive: focus Paint panel".to_string();
    }
    if paint_state.selected_tool != PaintTool::FreeDraw {
        return "paint inactive: tool is not Free Draw".to_string();
    }
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
        println!(
            "[PaintSystem][trace] status_skip status_text={status_text:?} reason=unchanged"
        );
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
        let asset_item = world
            .find_component(runtime_ui_root, "[name='asset_item']")
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

        assert_eq!(
            world
                .children_of(scene_root)
                .iter()
                .filter(|&&child| world.component_label(child) == Some("painted_asset_root"))
                .count(),
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
            _editor_root,
            scene_root,
            renderable,
            _paint_panel_root,
        ) = init_editor_fixture();

        push_click(&mut systems, renderable);
        let _ =
            systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        assert_eq!(
            world
                .children_of(scene_root)
                .iter()
                .filter(|&&child| world.component_label(child) == Some("painted_asset_root"))
                .count(),
            1
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
            _editor_root,
            scene_root,
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

        assert_eq!(
            world
                .children_of(scene_root)
                .iter()
                .filter(|&&child| world.component_label(child) == Some("painted_asset_root"))
                .count(),
            1
        );
    }

    #[test]
    fn grid_drag_places_only_when_cell_changes() {
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

        assert_eq!(
            world
                .children_of(scene_root)
                .iter()
                .filter(|&&child| world.component_label(child) == Some("painted_asset_root"))
                .count(),
            2
        );
    }
}
