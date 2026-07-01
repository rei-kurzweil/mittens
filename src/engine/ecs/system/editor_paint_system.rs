use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::engine::ecs::component::{
    DataComponent, DataValue, EditorComponent, OptionComponent, RaycastableComponent,
    SelectableComponent, SelectionComponent, TransformComponent, TransformGizmoComponent,
};
use crate::engine::ecs::system::editor::context::{
    EDITOR_WORKSPACE_ASSET_SELECTION_CHANGED, EditorContextState,
};
use crate::engine::ecs::system::editor_paint_system_state_manager::{
    PaintEvent, PaintState, PaintTool, is_paint_active, is_paint_panel_focused,
    paint_tool_from_item, reduce_paint_state,
};
use crate::engine::ecs::system::editor_system::select_editor_target;
use crate::engine::ecs::system::grid_system::{
    GridSnapResult, GridSpawnSpec, GridStep, GridSystem, remap_grid_rotation_to_surface_up,
};
use crate::engine::ecs::system::object_placement_preview::{
    PlacementKind, PlacementPreviewSession, PlacementPreviewStyle, commit_preview,
    create_preview_shell, update_preview_pose,
};
use crate::engine::ecs::system::paint_placement::{
    PlacementError, resolve_placement_pose, resolve_surface_aligned_pose_from_frame,
    resolve_surface_placement_frame,
};
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, RxWorld, Signal, SignalEmitter, SignalKind, World,
};
use crate::meow_meow::object::Value;
use crate::meow_meow::runner::{LoadedMmsModule, MeowMeowRunner};

const PAINT_PANEL_ROOT_SELECTOR: &str = "#paint_panel_root";
const PAINT_TOOL_SELECTION_SELECTOR: &str = "#paint_tool_selection";
const PAINT_STATUS_WRAP_SELECTOR: &str = "#paint_status_wrap";
const PANEL_STATUS_VALUE_SELECTOR: &str = "#paint_panel_status_value";
const RUNTIME_UI_ROOT_NAME: &str = "editor_runtime_ui_root";
const EDITOR_WORKSPACE_GRIDS_CHANGED: &str = "EditorWorkspaceGridsChanged";

fn paint_perf(label: &str, start: Instant, detail: impl AsRef<str>) {
    eprintln!(
        "🎨⏱️ paint_perf {label} took {:?} {}",
        start.elapsed(),
        detail.as_ref()
    );
}

#[derive(Debug, Clone)]
pub struct PaintAssetTemplate {
    pub key: String,
    pub title: String,
    pub module: Arc<LoadedMmsModule>,
    pub export_name: String,
    pub param_names: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct PaintStrokeRuntime {
    active: bool,
    captured_renderable: Option<ComponentId>,
    non_grid_placed: bool,
    last_grid_step: Option<GridStep>,
    last_placed_position: Option<[f32; 3]>,
    preview_session: Option<PlacementPreviewSession>,
}

#[derive(Debug, Default)]
pub struct EditorPaintSystem {
    installed_editor_roots: HashSet<ComponentId>,
    shared_panel_handlers_installed: bool,
    shared_state: Arc<Mutex<PaintState>>,
    shared_templates: Arc<Mutex<Vec<PaintAssetTemplate>>>,
}

impl EditorPaintSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn install_scoped_handlers_for_editor(
        &mut self,
        rx: &mut RxWorld,
        world: &World,
        grid_system: GridSystem,
        editor_root: ComponentId,
        panel_query_root: ComponentId,
        editor_context_state: Arc<Mutex<EditorContextState>>,
        templates: Vec<PaintAssetTemplate>,
    ) {
        *self
            .shared_templates
            .lock()
            .expect("paint templates mutex poisoned") = templates;

        if !self.shared_panel_handlers_installed {
            self.shared_panel_handlers_installed = true;
            install_shared_panel_handlers(
                rx,
                world,
                panel_query_root,
                grid_system.clone(),
                Arc::clone(&self.shared_state),
                Arc::clone(&editor_context_state),
                Arc::clone(&self.shared_templates),
            );
            bootstrap_paint_state(world, panel_query_root, &self.shared_state);
        }

        if self.installed_editor_roots.contains(&editor_root) {
            return;
        }
        self.installed_editor_roots.insert(editor_root);

        let stroke_runtime = Arc::new(Mutex::new(PaintStrokeRuntime::default()));
        install_editor_scene_handlers(
            rx,
            editor_root,
            panel_query_root,
            grid_system,
            Arc::clone(&self.shared_state),
            editor_context_state,
            Arc::clone(&self.shared_templates),
            stroke_runtime,
        );
    }
}

fn install_shared_panel_handlers(
    rx: &mut RxWorld,
    world: &World,
    panel_query_root: ComponentId,
    grid_system: GridSystem,
    paint_state: Arc<Mutex<PaintState>>,
    editor_context_state: Arc<Mutex<EditorContextState>>,
    templates: Arc<Mutex<Vec<PaintAssetTemplate>>>,
) {
    let _ = world;

    let state = Arc::clone(&paint_state);
    let ctx = Arc::clone(&editor_context_state);
    let tpl = Arc::clone(&templates);
    let asset_grid_system = grid_system.clone();
    rx.add_handler_closure_named(
        SignalKind::DataEvent,
        panel_query_root,
        Some("paint_system".to_string()),
        move |world, emit, signal| {
            let Some(EventSignal::DataEvent { name, payload }) = signal.event.as_ref() else {
                return;
            };
            if name != EDITOR_WORKSPACE_ASSET_SELECTION_CHANGED {
                return;
            }

            let component = *payload;
            let label = component.and_then(|id| label_from_component_id(world, id));
            let event = PaintEvent::AssetSelectionChanged {
                item: label,
                component,
            };
            handle_paint_event(
                world,
                emit,
                panel_query_root,
                &asset_grid_system,
                &tpl,
                &state,
                &ctx,
                None,
                &event,
            );
        },
    );

    let state = Arc::clone(&paint_state);
    let ctx = Arc::clone(&editor_context_state);
    let tpl = Arc::clone(&templates);
    rx.add_handler_closure_named(
        SignalKind::SelectionChanged,
        panel_query_root,
        Some("paint_system".to_string()),
        move |world, emit, signal| {
            let Some(ref event) = signal.event else { return };
            let EventSignal::SelectionChanged {
                selection_root,
                selected_component,
                selected_payload,
                ..
            } = event else { return };

            let component = selected_payload.or(*selected_component);
            let label = component.and_then(|id| label_from_component_id(world, id));

            if let Some(id) = component {
                if label.is_none() {
                    if let Some(rec) = world.get_component_record(id) {
                        let dc = world.get_component_by_id_as::<DataComponent>(id);
                        let has_dc = dc.is_some();
                        let entries_str = dc.map(|d| format!("{:?}", d.entries())).unwrap_or_default();
                        if PAINT_DEBUG_LOGS {
                            eprintln!("paint_debug bridge: label missing for component={id:?} type={} name={:?} has_DataComponent={} entries={}", rec.component_type, rec.name, has_dc, entries_str);
                        }
                    } else {
                        if PAINT_DEBUG_LOGS {
                            eprintln!("paint_debug bridge: component={id:?} has no record");
                        }
                    }
                }
            }

            let is_tool = world
                .find_component(panel_query_root, PAINT_TOOL_SELECTION_SELECTOR)
                .is_some_and(|root| root == *selection_root);

            if PAINT_DEBUG_LOGS {
                println!(
                    "paint_debug bridge SelectionChanged is_tool={is_tool} selection_root={selection_root:?} component={component:?} label={label:?}",
                );
            }
            if is_tool {
                let event = PaintEvent::ToolSelectionChanged {
                    tool: paint_tool_from_item(label.clone()),
                    item: label,
                    component,
                };
                handle_paint_event(
                    world,
                    emit,
                    panel_query_root,
                    &grid_system,
                    &tpl,
                    &state,
                    &ctx,
                    None,
                    &event,
                );
            }
        },
    );
}

fn install_editor_scene_handlers(
    rx: &mut RxWorld,
    editor_root: ComponentId,
    panel_query_root: ComponentId,
    grid_system: GridSystem,
    paint_state: Arc<Mutex<PaintState>>,
    editor_context_state: Arc<Mutex<EditorContextState>>,
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
        let editor_context = Arc::clone(&editor_context_state);
        let shared_templates = Arc::clone(&templates);
        let runtime = Arc::clone(&stroke_runtime);
        let grid_system = grid_system.clone();
        rx.add_handler_closure_named(
            signal_kind,
            editor_root,
            Some("paint_system".to_string()),
            move |world, emit, signal| {
                let Some(event) =
                    paint_event_from_editor_signal(world, editor_root, panel_query_root, signal)
                else {
                    return;
                };

                handle_paint_event(
                    world,
                    emit,
                    panel_query_root,
                    &grid_system,
                    &shared_templates,
                    &state,
                    &editor_context,
                    Some(&runtime),
                    &event,
                );
            },
        );
    }
}

fn handle_paint_event(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    grid_system: &GridSystem,
    templates: &Arc<Mutex<Vec<PaintAssetTemplate>>>,
    paint_state: &Arc<Mutex<PaintState>>,
    editor_context_state: &Arc<Mutex<EditorContextState>>,
    stroke_runtime: Option<&Arc<Mutex<PaintStrokeRuntime>>>,
    event: &PaintEvent,
) {
    let total_start = Instant::now();
    if PAINT_DEBUG_LOGS {
        eprintln!("paint_debug handle_paint_event event={event:?}");
    }
    let reduce_start = Instant::now();
    let (old_state, new_state) = {
        let mut state = paint_state.lock().expect("paint state mutex poisoned");
        let old_state = state.clone();
        let new_state = reduce_paint_state(&old_state, event);
        *state = new_state.clone();
        if PAINT_DEBUG_LOGS {
            eprintln!("paint_debug   old={old_state:?} new={new_state:?}");
        }
        (old_state, new_state)
    };
    paint_perf(
        "handle_paint_event.reduce",
        reduce_start,
        format!("event={event:?}"),
    );

    let effects_start = Instant::now();
    apply_paint_side_effects(
        world,
        emit,
        panel_query_root,
        grid_system,
        templates,
        editor_context_state,
        &old_state,
        &new_state,
        event,
        stroke_runtime,
    );
    paint_perf(
        "handle_paint_event.side_effects",
        effects_start,
        format!("event={event:?}"),
    );
    paint_perf(
        "handle_paint_event.total",
        total_start,
        format!("event={event:?}"),
    );
}

fn bootstrap_paint_state(
    world: &World,
    panel_query_root: ComponentId,
    paint_state: &Arc<Mutex<PaintState>>,
) {
    let mut events = Vec::new();

    if let Some(tool_event) = bootstrap_selection_event(
        world,
        panel_query_root,
        PAINT_TOOL_SELECTION_SELECTOR,
        |selection, w| {
            let label = label_from_selected_payload(w, selection);
            PaintEvent::ToolSelectionChanged {
                tool: paint_tool_from_item(label.clone()),
                item: label,
                component: selection.selected_component,
            }
        },
    ) {
        events.push(tool_event);
    }

    let state_str;
    {
        let mut state = paint_state.lock().expect("paint state mutex poisoned");
        for event in &events {
            *state = reduce_paint_state(&state, event);
        }
        // If the tool selection component exists but has no selected item,
        // default to FreeDraw. The MMS template creates the selection
        // component but doesn't set an initial selection.
        if matches!(state.selected_tool, PaintTool::Unknown(None)) {
            *state = reduce_paint_state(
                &state,
                &PaintEvent::ToolSelectionChanged {
                    tool: PaintTool::FreeDraw,
                    item: Some("FreeDraw".to_string()),
                    component: None,
                },
            );
        }
        state_str = format!("{state:?}");
    }
    if PAINT_DEBUG_LOGS {
        eprintln!("paint_debug bootstrap_paint_state done -> {}", state_str);
    }
}

fn label_from_component_id(world: &World, id: ComponentId) -> Option<String> {
    if let Some(data) = world.get_component_by_id_as::<DataComponent>(id) {
        if let Some(DataValue::Text(label)) = data.get("label") {
            return Some(label.clone());
        }
    }
    None
}

fn label_from_selected_payload(world: &World, selection: &SelectionComponent) -> Option<String> {
    let payload_id = selection.selected_payload?;
    label_from_component_id(world, payload_id)
}

fn bootstrap_selection_event<F>(
    world: &World,
    panel_query_root: ComponentId,
    selector: &str,
    event_builder: F,
) -> Option<PaintEvent>
where
    F: FnOnce(&SelectionComponent, &World) -> PaintEvent,
{
    let selection_root = world.find_component(panel_query_root, selector)?;
    let selection = world.get_component_by_id_as::<SelectionComponent>(selection_root)?;
    Some(event_builder(selection, world))
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

    event
}

fn apply_paint_side_effects(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    grid_system: &GridSystem,
    templates: &Arc<Mutex<Vec<PaintAssetTemplate>>>,
    editor_context_state: &Arc<Mutex<EditorContextState>>,
    _old_state: &PaintState,
    new_state: &PaintState,
    event: &PaintEvent,
    stroke_runtime: Option<&Arc<Mutex<PaintStrokeRuntime>>>,
) {
    let total_start = Instant::now();
    let editor_context = current_editor_context(editor_context_state);
    let active_editor = event_active_editor(event).or(editor_context.active_editor);
    paint_perf(
        "apply_paint_side_effects.context",
        total_start,
        format!("event={event:?} active_editor={active_editor:?}"),
    );
    let mut status_override = None;
    let templates_start = Instant::now();
    let templates_lock = templates.lock().expect("paint templates mutex poisoned");
    let templates = &*templates_lock;
    paint_perf(
        "apply_paint_side_effects.clone_templates",
        templates_start,
        format!("count={}", templates.len()),
    );

    let event_specific_start = Instant::now();
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
                grid_system,
                templates,
                new_state,
                &editor_context,
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
                if let Some(context) = resolve_paint_context(
                    world,
                    grid_system,
                    *editor,
                    panel_query_root,
                    new_state,
                    &editor_context,
                    templates,
                ) {
                    *runtime = PaintStrokeRuntime {
                        active: true,
                        captured_renderable: Some(*renderable),
                        non_grid_placed: false,
                        last_grid_step: None,
                        last_placed_position: None,
                        preview_session: start_preview_session_for_tool(
                            world,
                            emit,
                            *editor,
                            *renderable,
                            *hit_point,
                            new_state.selected_tool.clone(),
                            &context,
                            &editor_context,
                        ),
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
                grid_system,
                templates,
                new_state,
                &editor_context,
                stroke_runtime,
                *renderable,
                *hit_point,
            );
        }
        PaintEvent::StrokeEnded { .. } => {
            if let Some(runtime) = stroke_runtime {
                let mut runtime = runtime.lock().expect("paint stroke runtime mutex poisoned");
                if let Some(session) = runtime.preview_session.take() {
                    commit_preview(world, session.preview_root_component_id);
                    if session.placement_kind == PlacementKind::Grid {
                        let _ = grid_system.set_grid_hidden(
                            world,
                            emit,
                            session.preview_root_component_id,
                            false,
                        );
                        grid_system.mark_dirty();
                        emit.push_event(
                            panel_query_root,
                            EventSignal::DataEvent {
                                name: EDITOR_WORKSPACE_GRIDS_CHANGED.to_string(),
                                payload: Some(session.active_editor),
                            },
                        );
                        select_editor_target(
                            world,
                            emit,
                            session.active_editor,
                            session.preview_root_component_id,
                            false,
                        );
                    }
                    status_override = Some("paint placed".to_string());
                }
                *runtime = PaintStrokeRuntime::default();
            }
        }
        _ => {}
    }
    paint_perf(
        "apply_paint_side_effects.event_specific",
        event_specific_start,
        format!("event={event:?} status_override={status_override:?}"),
    );

    let update_status_start = Instant::now();
    update_paint_status(
        world,
        emit,
        active_editor,
        panel_query_root,
        grid_system,
        new_state,
        &editor_context,
        status_override,
    );
    paint_perf(
        "apply_paint_side_effects.update_status",
        update_status_start,
        format!("event={event:?}"),
    );
    paint_perf(
        "apply_paint_side_effects.total",
        total_start,
        format!("event={event:?}"),
    );
}

fn current_editor_context(
    editor_context_state: &Arc<Mutex<EditorContextState>>,
) -> EditorContextState {
    editor_context_state
        .lock()
        .expect("editor context mutex poisoned")
        .clone()
}

fn event_active_editor(event: &PaintEvent) -> Option<ComponentId> {
    match event {
        PaintEvent::ActiveEditorChanged { editor } => *editor,
        PaintEvent::WorldPanelSelectionChanged { editor, .. } => *editor,
        PaintEvent::EditorSelectionChanged { editor, .. }
        | PaintEvent::SceneClick { editor, .. }
        | PaintEvent::StrokeStarted { editor, .. }
        | PaintEvent::StrokeMoved { editor, .. }
        | PaintEvent::StrokeEnded { editor } => Some(*editor),
        PaintEvent::AssetSelectionChanged { .. }
        | PaintEvent::ToolSelectionChanged { .. }
        | PaintEvent::PanelFocusChanged { .. } => None,
    }
}

fn random_offset_xz(max_dist: f32) -> [f32; 3] {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEED: AtomicU32 = AtomicU32::new(987654321);

    let mut x = SEED.load(Ordering::Relaxed);
    if x == 0 {
        x = 987654321;
    }
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    SEED.store(x, Ordering::Relaxed);

    let r1 = ((x & 0xffff) as f32) / 65535.0; // [0, 1]

    let mut y = x.wrapping_mul(1103515245).wrapping_add(12345);
    if y == 0 {
        y = 123456789;
    }
    y ^= y << 13;
    y ^= y >> 17;
    y ^= y << 5;
    let r2 = ((y & 0xffff) as f32) / 65535.0; // [0, 1]

    let r = r1.sqrt() * max_dist;
    let theta = r2 * 2.0 * std::f32::consts::PI;

    [r * theta.cos(), 0.0, r * theta.sin()]
}

fn find_painted_asset_root(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut cur = Some(start);
    while let Some(node) = cur {
        if world.component_label(node) == Some("painted_asset_raycastable") {
            return Some(node);
        }
        cur = world.parent_of(node);
    }
    None
}

fn handle_free_draw_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    context: &PaintContext<'_>,
    stroke_runtime: Option<&Arc<Mutex<PaintStrokeRuntime>>>,
    renderable: ComponentId,
    hit_point: [f32; 3],
) -> Option<String> {
    let _ = (
        world,
        emit,
        editor_root,
        context,
        stroke_runtime,
        renderable,
        hit_point,
    );
    None
}

fn handle_free_draw_stroke_move(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    context: &PaintContext<'_>,
    stroke_runtime: Option<&Arc<Mutex<PaintStrokeRuntime>>>,
    renderable: ComponentId,
    hit_point: [f32; 3],
) -> Option<String> {
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

    let Some(session) = runtime.preview_session.as_mut() else {
        return None;
    };
    let grid_snap = context.grid_snap(world, renderable, hit_point);
    let frame = match resolve_surface_placement_frame(world, renderable, hit_point, grid_snap) {
        Ok(frame) => frame,
        Err(_) => return None,
    };
    let pose = resolve_surface_aligned_pose_from_frame(&frame, session.local_min_z).ok()?;
    update_preview_pose(world, emit, session.preview_root_component_id, pose);
    session.last_valid_placement_frame = Some(frame);
    Some(match session.placement_kind {
        PlacementKind::PaintAsset => format!("paint preview: {}", context.asset.title),
        PlacementKind::Grid => "grid preview".to_string(),
    })
}

fn handle_spray_can_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    context: &PaintContext<'_>,
    stroke_runtime: Option<&Arc<Mutex<PaintStrokeRuntime>>>,
    renderable: ComponentId,
    hit_point: [f32; 3],
) -> Option<String> {
    let runtime = stroke_runtime?;
    let mut runtime = runtime.lock().expect("paint stroke runtime mutex poisoned");
    if runtime.non_grid_placed {
        return None;
    }
    runtime.non_grid_placed = true;
    runtime.last_placed_position = Some(hit_point);

    let offset = random_offset_xz(1.5);
    let offset_hit_point = [
        hit_point[0] + offset[0],
        hit_point[1] + offset[1],
        hit_point[2] + offset[2],
    ];

    Some(place_asset(
        world,
        emit,
        editor_root,
        renderable,
        offset_hit_point,
        &context.asset,
        None,
    ))
}

fn handle_spray_can_stroke_move(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    context: &PaintContext<'_>,
    stroke_runtime: Option<&Arc<Mutex<PaintStrokeRuntime>>>,
    renderable: ComponentId,
    hit_point: [f32; 3],
) -> Option<String> {
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

    let dist = if let Some(last_pos) = runtime.last_placed_position {
        let dx = hit_point[0] - last_pos[0];
        let dy = hit_point[1] - last_pos[1];
        let dz = hit_point[2] - last_pos[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    } else {
        f32::MAX
    };

    if dist >= 0.5 {
        runtime.last_placed_position = Some(hit_point);

        let offset = random_offset_xz(1.5);
        let offset_hit_point = [
            hit_point[0] + offset[0],
            hit_point[1] + offset[1],
            hit_point[2] + offset[2],
        ];

        Some(place_asset(
            world,
            emit,
            editor_root,
            renderable,
            offset_hit_point,
            &context.asset,
            None,
        ))
    } else {
        None
    }
}

fn handle_erase_click(
    world: &mut World,
    _emit: &mut dyn SignalEmitter,
    _editor_root: ComponentId,
    _context: &PaintContext<'_>,
    stroke_runtime: Option<&Arc<Mutex<PaintStrokeRuntime>>>,
    renderable: ComponentId,
    _hit_point: [f32; 3],
) -> Option<String> {
    let runtime = stroke_runtime?;
    let mut runtime = runtime.lock().expect("paint stroke runtime mutex poisoned");
    if runtime.non_grid_placed {
        return None;
    }
    runtime.non_grid_placed = true;

    if let Some(target_root) = find_painted_asset_root(world, renderable) {
        if world.remove_component_subtree(target_root).is_ok() {
            Some("erased asset".to_string())
        } else {
            None
        }
    } else {
        None
    }
}

fn handle_erase_stroke_move(
    world: &mut World,
    _emit: &mut dyn SignalEmitter,
    _editor_root: ComponentId,
    _context: &PaintContext<'_>,
    stroke_runtime: Option<&Arc<Mutex<PaintStrokeRuntime>>>,
    renderable: ComponentId,
    _hit_point: [f32; 3],
) -> Option<String> {
    let runtime = stroke_runtime?;
    let runtime = runtime.lock().expect("paint stroke runtime mutex poisoned");
    if !runtime.active {
        return None;
    }

    if let Some(target_root) = find_painted_asset_root(world, renderable) {
        if world.remove_component_subtree(target_root).is_ok() {
            return Some("erased asset".to_string());
        }
    }
    None
}

fn handle_line_click(
    _world: &mut World,
    _emit: &mut dyn SignalEmitter,
    _editor_root: ComponentId,
    _context: &PaintContext<'_>,
    _stroke_runtime: Option<&Arc<Mutex<PaintStrokeRuntime>>>,
    _renderable: ComponentId,
    _hit_point: [f32; 3],
) -> Option<String> {
    None
}

fn handle_line_stroke_move(
    _world: &mut World,
    _emit: &mut dyn SignalEmitter,
    _editor_root: ComponentId,
    _context: &PaintContext<'_>,
    _stroke_runtime: Option<&Arc<Mutex<PaintStrokeRuntime>>>,
    _renderable: ComponentId,
    _hit_point: [f32; 3],
) -> Option<String> {
    None
}

fn handle_fill_click(
    _world: &mut World,
    _emit: &mut dyn SignalEmitter,
    _editor_root: ComponentId,
    _context: &PaintContext<'_>,
    _stroke_runtime: Option<&Arc<Mutex<PaintStrokeRuntime>>>,
    _renderable: ComponentId,
    _hit_point: [f32; 3],
) -> Option<String> {
    None
}

fn handle_fill_stroke_move(
    _world: &mut World,
    _emit: &mut dyn SignalEmitter,
    _editor_root: ComponentId,
    _context: &PaintContext<'_>,
    _stroke_runtime: Option<&Arc<Mutex<PaintStrokeRuntime>>>,
    _renderable: ComponentId,
    _hit_point: [f32; 3],
) -> Option<String> {
    None
}

fn handle_scene_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    panel_query_root: ComponentId,
    grid_system: &GridSystem,
    templates: &[PaintAssetTemplate],
    paint_state: &PaintState,
    editor_context: &EditorContextState,
    stroke_runtime: Option<&Arc<Mutex<PaintStrokeRuntime>>>,
    renderable: ComponentId,
    hit_point: [f32; 3],
) -> Option<String> {
    let start = Instant::now();
    let context = resolve_paint_context(
        world,
        grid_system,
        editor_root,
        panel_query_root,
        paint_state,
        editor_context,
        templates,
    )?;
    paint_perf(
        "handle_scene_click.resolve_context",
        start,
        format!("tool={:?}", paint_state.selected_tool),
    );

    let apply_tool_start = Instant::now();
    let result = match paint_state.selected_tool {
        PaintTool::FreeDraw => handle_free_draw_click(
            world,
            emit,
            editor_root,
            &context,
            stroke_runtime,
            renderable,
            hit_point,
        ),
        PaintTool::GridTool => None,
        PaintTool::SprayCan => handle_spray_can_click(
            world,
            emit,
            editor_root,
            &context,
            stroke_runtime,
            renderable,
            hit_point,
        ),
        PaintTool::Erase => handle_erase_click(
            world,
            emit,
            editor_root,
            &context,
            stroke_runtime,
            renderable,
            hit_point,
        ),
        PaintTool::Line => handle_line_click(
            world,
            emit,
            editor_root,
            &context,
            stroke_runtime,
            renderable,
            hit_point,
        ),
        PaintTool::Fill => handle_fill_click(
            world,
            emit,
            editor_root,
            &context,
            stroke_runtime,
            renderable,
            hit_point,
        ),
        PaintTool::Unknown(_) => None,
    };
    paint_perf(
        "handle_scene_click.apply_tool",
        apply_tool_start,
        format!("tool={:?} result={result:?}", paint_state.selected_tool),
    );
    paint_perf(
        "handle_scene_click.total",
        start,
        format!("tool={:?} result={result:?}", paint_state.selected_tool),
    );
    result
}

fn handle_stroke_move(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    panel_query_root: ComponentId,
    grid_system: &GridSystem,
    templates: &[PaintAssetTemplate],
    paint_state: &PaintState,
    editor_context: &EditorContextState,
    stroke_runtime: Option<&Arc<Mutex<PaintStrokeRuntime>>>,
    renderable: ComponentId,
    hit_point: [f32; 3],
) -> Option<String> {
    let start = Instant::now();
    let context = resolve_paint_context(
        world,
        grid_system,
        editor_root,
        panel_query_root,
        paint_state,
        editor_context,
        templates,
    )?;
    paint_perf(
        "handle_stroke_move.resolve_context",
        start,
        format!("tool={:?}", paint_state.selected_tool),
    );

    let apply_tool_start = Instant::now();
    let result = match paint_state.selected_tool {
        PaintTool::FreeDraw => handle_free_draw_stroke_move(
            world,
            emit,
            editor_root,
            &context,
            stroke_runtime,
            renderable,
            hit_point,
        ),
        PaintTool::GridTool => handle_free_draw_stroke_move(
            world,
            emit,
            editor_root,
            &context,
            stroke_runtime,
            renderable,
            hit_point,
        ),
        PaintTool::SprayCan => handle_spray_can_stroke_move(
            world,
            emit,
            editor_root,
            &context,
            stroke_runtime,
            renderable,
            hit_point,
        ),
        PaintTool::Erase => handle_erase_stroke_move(
            world,
            emit,
            editor_root,
            &context,
            stroke_runtime,
            renderable,
            hit_point,
        ),
        PaintTool::Line => handle_line_stroke_move(
            world,
            emit,
            editor_root,
            &context,
            stroke_runtime,
            renderable,
            hit_point,
        ),
        PaintTool::Fill => handle_fill_stroke_move(
            world,
            emit,
            editor_root,
            &context,
            stroke_runtime,
            renderable,
            hit_point,
        ),
        PaintTool::Unknown(_) => None,
    };
    paint_perf(
        "handle_stroke_move.apply_tool",
        apply_tool_start,
        format!("tool={:?} result={result:?}", paint_state.selected_tool),
    );
    paint_perf(
        "handle_stroke_move.total",
        start,
        format!("tool={:?} result={result:?}", paint_state.selected_tool),
    );
    result
}

#[derive(Debug, Clone)]
struct PaintContext<'a> {
    asset: &'a PaintAssetTemplate,
    grid_system: GridSystem,
}

impl<'a> PaintContext<'a> {
    fn grid_snap(
        &self,
        world: &World,
        target_renderable: ComponentId,
        hit_point: [f32; 3],
    ) -> Option<GridSnapResult> {
        let grid = self
            .grid_system
            .grid_hit_context_for_renderable(world, target_renderable)?;
        Some(GridSystem::snap_hit(&grid, hit_point))
    }
}

fn resolve_paint_context<'a>(
    world: &World,
    grid_system: &GridSystem,
    editor_root: ComponentId,
    panel_query_root: ComponentId,
    paint_state: &PaintState,
    editor_context: &EditorContextState,
    templates: &'a [PaintAssetTemplate],
) -> Option<PaintContext<'a>> {
    let start = Instant::now();
    let paint_panel_root = world.find_component(panel_query_root, PAINT_PANEL_ROOT_SELECTOR);
    if !is_paint_active(paint_panel_root, paint_state, editor_context) {
        paint_perf(
            "resolve_paint_context.inactive",
            start,
            format!(
                "tool={:?} panel={paint_panel_root:?}",
                paint_state.selected_tool
            ),
        );
        return None;
    }
    let asset = match paint_state.selected_tool {
        PaintTool::GridTool | PaintTool::Erase => templates.first(),
        _ => {
            let selected_asset = paint_state.selected_asset.as_ref()?;
            let payload = selected_asset.component?;
            let asset_key =
                if let Some(data) = world.get_component_by_id_as::<DataComponent>(payload) {
                    match data.get("asset_key") {
                        Some(crate::engine::ecs::component::DataValue::Text(asset_key)) => {
                            asset_key.clone()
                        }
                        _ => return None,
                    }
                } else {
                    return None;
                };
            templates.iter().find(|template| template.key == asset_key)
        }
    }?;
    let context = PaintContext {
        asset,
        grid_system: grid_system.clone(),
    };
    paint_perf(
        "resolve_paint_context.active",
        start,
        format!(
            "tool={:?} asset_key={}",
            paint_state.selected_tool, asset.key,
        ),
    );
    Some(context)
}

struct PaintActivityStatus {
    active: bool,
    reason: String,
}

fn start_preview_session_for_tool(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    target_renderable: ComponentId,
    hit_point: [f32; 3],
    selected_tool: PaintTool,
    context: &PaintContext<'_>,
    editor_context: &EditorContextState,
) -> Option<PlacementPreviewSession> {
    match selected_tool {
        PaintTool::FreeDraw => start_paint_preview_session(
            world,
            emit,
            editor_root,
            target_renderable,
            hit_point,
            context,
        ),
        PaintTool::GridTool => start_grid_preview_session(
            world,
            emit,
            editor_root,
            target_renderable,
            hit_point,
            context,
            editor_context,
        ),
        _ => None,
    }
}

fn start_paint_preview_session(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    target_renderable: ComponentId,
    hit_point: [f32; 3],
    context: &PaintContext<'_>,
) -> Option<PlacementPreviewSession> {
    let scene_parent = resolve_scene_parent(world, editor_root);
    let asset_root = spawn_asset_subtree(world, emit, &context.asset).ok()?;
    let preview_root =
        world.add_component_boxed_named("painted_asset_root", Box::new(TransformComponent::new()));
    let raycastable_root = world.add_component_boxed_named(
        "painted_asset_raycastable",
        Box::new(RaycastableComponent::enabled()),
    );
    let _ = world.add_child(raycastable_root, preview_root);
    let _ = world.add_child(preview_root, asset_root);
    let _ = world.add_child(scene_parent, raycastable_root);
    create_preview_shell(world, preview_root, emit, PlacementPreviewStyle::default());
    world.init_component_tree(raycastable_root, emit);
    let grid_snap = context.grid_snap(world, target_renderable, hit_point);
    let frame =
        resolve_surface_placement_frame(world, target_renderable, hit_point, grid_snap).ok()?;
    let pose =
        resolve_surface_aligned_pose_from_frame(&frame, asset_local_min_z(world, preview_root)?)
            .ok()?;
    update_preview_pose(world, emit, preview_root, pose);
    Some(PlacementPreviewSession {
        active_editor: editor_root,
        placement_kind: PlacementKind::PaintAsset,
        preview_root_component_id: preview_root,
        target_renderable: Some(target_renderable),
        last_valid_placement_frame: Some(frame),
        local_min_z: asset_local_min_z(world, preview_root)?,
    })
}

fn start_grid_preview_session(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
    target_renderable: ComponentId,
    hit_point: [f32; 3],
    context: &PaintContext<'_>,
    editor_context: &EditorContextState,
) -> Option<PlacementPreviewSession> {
    let grid_snap = context.grid_snap(world, target_renderable, hit_point);
    let frame =
        resolve_surface_placement_frame(world, target_renderable, hit_point, grid_snap).ok()?;
    let pose = resolve_surface_aligned_pose_from_frame(&frame, 0.0).ok()?;
    let mut preview_context = editor_context.clone();
    preview_context.cursor_translation = Some(pose.translation);
    preview_context.cursor_rotation = Some(remap_grid_rotation_to_surface_up(pose.rotation));
    let preview_root = GridSystem::new().spawn_grid_for_editor(
        world,
        emit,
        editor_root,
        GridSpawnSpec::from_cursor_pose(
            preview_context.cursor_translation,
            preview_context.cursor_rotation,
            true,
        ),
    );
    create_preview_shell(world, preview_root, emit, PlacementPreviewStyle::default());
    update_preview_pose(world, emit, preview_root, pose);
    Some(PlacementPreviewSession {
        active_editor: editor_root,
        placement_kind: PlacementKind::Grid,
        preview_root_component_id: preview_root,
        target_renderable: Some(target_renderable),
        last_valid_placement_frame: Some(frame),
        local_min_z: 0.0,
    })
}

fn paint_activity_status(
    world: &World,
    grid_system: &GridSystem,
    active_editor: Option<ComponentId>,
    panel_query_root: ComponentId,
    paint_state: &PaintState,
    editor_context: &EditorContextState,
) -> PaintActivityStatus {
    let paint_panel_root = world.find_component(panel_query_root, PAINT_PANEL_ROOT_SELECTOR);
    if !is_paint_panel_focused(paint_panel_root, editor_context) {
        return PaintActivityStatus {
            active: false,
            reason: "focus Paint panel".to_string(),
        };
    }

    let asset_required = !matches!(
        paint_state.selected_tool,
        PaintTool::Erase | PaintTool::GridTool
    );
    if asset_required
        && paint_state
            .selected_asset
            .as_ref()
            .and_then(|selection| selection.component)
            .is_none()
    {
        return PaintActivityStatus {
            active: false,
            reason: "no asset selected".to_string(),
        };
    }

    match paint_state.selected_tool {
        PaintTool::FreeDraw | PaintTool::GridTool | PaintTool::SprayCan | PaintTool::Erase => {}
        _ => {
            return PaintActivityStatus {
                active: false,
                reason: format!("tool is not supported ({:?})", paint_state.selected_tool),
            };
        }
    }

    let Some(_editor_root) = active_editor else {
        return PaintActivityStatus {
            active: false,
            reason: "no active editor".to_string(),
        };
    };

    PaintActivityStatus {
        active: true,
        reason: "snap on shown grid hits only".to_string(),
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

    let asset_root = match spawn_asset_subtree(world, emit, asset) {
        Ok(asset_root) => asset_root,
        Err(error) => return error,
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
        "snapped on grid hit"
    } else {
        "unsnapped"
    };
    format!("paint placed: {} | {}", asset.title, grid_text)
}

fn spawn_asset_subtree(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    asset: &PaintAssetTemplate,
) -> Result<ComponentId, String> {
    let asset_root = MeowMeowRunner::spawn_mms_module_component_uninitialized(
        &asset.module,
        &asset.export_name,
        default_asset_args(asset),
        world,
        emit,
    )
    .map_err(|error| format!("paint failed: asset spawn error: {error}"))?;
    sanitize_painted_asset_subtree(world, asset_root);
    Ok(asset_root)
}

fn asset_local_min_z(world: &World, root: ComponentId) -> Option<f32> {
    crate::engine::ecs::system::paint_placement::measure_subtree_local_bounds(world, root)
        .map(|bounds| bounds.min[2])
}

fn sanitize_painted_asset_subtree(world: &mut World, root: ComponentId) {
    let mut stack = vec![root];
    let mut selection_components = Vec::new();
    let mut option_components = Vec::new();

    while let Some(node) = stack.pop() {
        if world
            .get_component_by_id_as::<SelectionComponent>(node)
            .is_some()
        {
            selection_components.push(node);
        }
        if world
            .get_component_by_id_as::<OptionComponent>(node)
            .is_some()
        {
            option_components.push(node);
        }
        for &child in world.children_of(node) {
            stack.push(child);
        }
    }

    for component in option_components {
        let _ = world.remove_component_leaf(component);
    }
    for component in selection_components {
        let _ = world.remove_component_leaf(component);
    }
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
    grid_system: &GridSystem,
    paint_state: &PaintState,
    editor_context: &EditorContextState,
    override_text: Option<String>,
) {
    let start = Instant::now();
    let Some(paint_panel_root) = world.find_component(panel_query_root, PAINT_PANEL_ROOT_SELECTOR)
    else {
        return;
    };
    let Some(status_wrap) = world.find_component(paint_panel_root, PAINT_STATUS_WRAP_SELECTOR)
    else {
        return;
    };
    let text = override_text.unwrap_or_else(|| {
        base_status_text(
            world,
            active_editor,
            panel_query_root,
            grid_system,
            paint_state,
            editor_context,
        )
    });
    paint_perf(
        "update_paint_status.compute_text",
        start,
        format!("active_editor={active_editor:?} text={text}"),
    );
    let set_start = Instant::now();
    set_status_text(world, emit, status_wrap, &text);
    paint_perf(
        "update_paint_status.set_text",
        set_start,
        format!("text={text}"),
    );
    paint_perf("update_paint_status.total", start, format!("text={text}"));
}

fn base_status_text(
    world: &World,
    active_editor: Option<ComponentId>,
    panel_query_root: ComponentId,
    grid_system: &GridSystem,
    paint_state: &PaintState,
    editor_context: &EditorContextState,
) -> String {
    let paint_panel_root = world.find_component(panel_query_root, PAINT_PANEL_ROOT_SELECTOR);
    if !is_paint_panel_focused(paint_panel_root, editor_context) {
        return "paint inactive: focus Paint panel".to_string();
    }

    let asset_required = !matches!(
        paint_state.selected_tool,
        PaintTool::Erase | PaintTool::GridTool
    );
    if asset_required
        && paint_state
            .selected_asset
            .as_ref()
            .and_then(|selection| selection.component)
            .is_none()
    {
        return "paint inactive: no asset selected".to_string();
    }

    match paint_state.selected_tool {
        PaintTool::FreeDraw | PaintTool::GridTool | PaintTool::SprayCan | PaintTool::Erase => {}
        _ => {
            return format!(
                "paint inactive: tool is not supported ({:?})",
                paint_state.selected_tool
            );
        }
    }

    let Some(_editor_root) = active_editor else {
        return "paint inactive: no active editor".to_string();
    };

    let tool_name = match paint_state.selected_tool {
        PaintTool::FreeDraw => "Free Draw",
        PaintTool::GridTool => "Grid Tool",
        PaintTool::SprayCan => "Spray Can",
        PaintTool::Erase => "Erase",
        _ => unreachable!(),
    };

    format!("{tool_name} active | snap only on shown grid hits")
}

fn set_status_text(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    status_wrap: ComponentId,
    text: &str,
) {
    let Some(status_text) = world.find_component(status_wrap, PANEL_STATUS_VALUE_SELECTOR) else {
        return;
    };
    let Some(text_component) = world
        .get_component_by_id_as_mut::<crate::engine::ecs::component::TextComponent>(status_text)
    else {
        return;
    };
    if text_component.text == text {
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
    use crate::engine::ecs::component::{ColorComponent, GridComponent, RenderableComponent};
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

    fn push_drag_place(systems: &mut SystemWorld, renderable: ComponentId) {
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
                hit_point: [0.2, 0.0, 0.5],
                delta_world: [0.2, 0.0, 0.0],
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
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);

        let runtime_ui_root = find_named_root(&world, RUNTIME_UI_ROOT_NAME);
        let paint_panel_root = world
            .find_component(runtime_ui_root, PAINT_PANEL_ROOT_SELECTOR)
            .expect("paint panel");

        push_asset_and_panel_focus(&world, &mut systems, paint_panel_root);
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );
        systems.rx.begin_frame();
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );
        systems.rx.begin_frame();
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
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
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );
        systems.rx.begin_frame();
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );

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
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);

        push_drag_place(&mut systems, renderable);
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);

        assert_eq!(
            count_named_descendants(&world, editor_root, "painted_asset_root"),
            0
        );
    }

    #[test]
    fn asset_selection_bootstraps_to_data_payload() {
        let (
            world,
            _emit,
            _visuals,
            _systems,
            _render_assets,
            _editor_root,
            _scene_root,
            _renderable,
            _paint_panel_root,
        ) = init_editor_fixture();

        let payload = _systems
            .editor_context
            .shared_state()
            .lock()
            .expect("editor context state")
            .selected_asset_payload
            .expect("selected payload");
        let asset_payload = world
            .get_component_by_id_as::<DataComponent>(payload)
            .expect("asset payload data");

        assert!(matches!(
            asset_payload.get("asset_key"),
            Some(DataValue::Text(asset_key)) if !asset_key.is_empty()
        ));
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

        push_drag_place(&mut systems, renderable);
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);

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

        push_drag_place(&mut systems, renderable);
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);

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
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);

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
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);

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
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);

        let runtime_ui_root = find_named_root(&world, RUNTIME_UI_ROOT_NAME);
        let paint_panel_root = world
            .find_component(runtime_ui_root, PAINT_PANEL_ROOT_SELECTOR)
            .expect("paint panel");

        push_asset_and_panel_focus(&world, &mut systems, paint_panel_root);
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );
        systems.rx.begin_frame();
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );
        systems.rx.begin_frame();
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );
        push_click(&mut systems, paint_panel_root);
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );
        systems.rx.begin_frame();
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );

        push_drag_place(&mut systems, renderable_b);
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);

        assert_eq!(
            count_named_descendants(&world, editor_a, "painted_asset_root"),
            0
        );
        assert_eq!(
            count_named_descendants(&world, editor_b, "painted_asset_root"),
            1
        );
    }

    #[test]
    fn paint_tool_line_and_fill_noop() {
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

        // 1. Line tool
        {
            let mut state = systems.editor_paint.shared_state.lock().unwrap();
            state.selected_tool = PaintTool::Line;
        }

        push_drag_place(&mut systems, renderable);
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);

        assert_eq!(
            count_named_descendants(&world, editor_root, "painted_asset_root"),
            0,
            "Line tool should NOT place assets on click (NOOP)"
        );

        // 2. Fill tool
        {
            let mut state = systems.editor_paint.shared_state.lock().unwrap();
            state.selected_tool = PaintTool::Fill;
        }

        push_drag_place(&mut systems, renderable);
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);

        assert_eq!(
            count_named_descendants(&world, editor_root, "painted_asset_root"),
            0,
            "Fill tool should NOT place assets on click (NOOP)"
        );
    }

    #[test]
    fn paint_tool_spray_can_scatters_assets() {
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

        {
            let mut state = systems.editor_paint.shared_state.lock().unwrap();
            state.selected_tool = PaintTool::SprayCan;
        }

        // Click hit point is [0.0, 0.0, 0.5] in push_click
        push_drag_place(&mut systems, renderable);
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);

        assert_eq!(
            count_named_descendants(&world, editor_root, "painted_asset_root"),
            1,
            "Spray Can tool should place 1 asset"
        );

        let painted_root = world
            .find_component(editor_root, "[name='painted_asset_root']")
            .expect("painted asset root");
        let transform = world
            .get_component_by_id_as::<TransformComponent>(painted_root)
            .expect("transform");
        let position = transform.transform.translation;

        // Since Spray Can places with random offset, it should NOT be exactly at [0.0, 0.0, 0.5].
        let dx = position[0] - 0.0;
        let dz = position[2] - 0.5;
        let dist = (dx * dx + dz * dz).sqrt();
        assert!(
            dist > 0.0,
            "Spray Can should offset the asset placement by a random distance: position={:?}",
            position
        );
        assert!(
            dist <= 1.5,
            "Spray Can offset should be at most 1.5 units: position={:?}",
            position
        );
    }

    #[test]
    fn paint_tool_erase_removes_painted_assets() {
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

        // 1. First, place an asset using Free Draw
        push_drag_place(&mut systems, renderable);
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);

        assert_eq!(
            count_named_descendants(&world, editor_root, "painted_asset_root"),
            1,
            "expected 1 painted asset root initially"
        );

        let painted_root = world
            .find_component(editor_root, "[name='painted_asset_root']")
            .expect("painted asset root");
        let painted_renderable = world
            .find_component(painted_root, "Renderable")
            .expect("painted renderable");

        // 2. Select Erase tool
        {
            let mut state = systems.editor_paint.shared_state.lock().unwrap();
            state.selected_tool = PaintTool::Erase;
        }

        // 3. Click directly on the painted renderable to erase it
        push_click(&mut systems, painted_renderable);
        let _ = systems.process_signals(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut emit,
            100_000,
        );
        systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut emit);

        assert_eq!(
            count_named_descendants(&world, editor_root, "painted_asset_root"),
            0,
            "Erase tool should remove the painted asset subtree"
        );
    }
}
const PAINT_DEBUG_LOGS: bool = false;
