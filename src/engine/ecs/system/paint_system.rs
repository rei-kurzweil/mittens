use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::engine::ecs::component::{
    EditorComponent, SelectableComponent, SelectionComponent, TransformComponent,
    TransformGizmoComponent,
};
use crate::engine::ecs::system::grid_system::{GridSnapResult, GridStep, GridSystem};
use crate::engine::ecs::system::paint_placement::{PlacementError, resolve_placement_pose};
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, RxWorld, SignalEmitter, SignalKind, World};
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

#[derive(Debug, Clone)]
pub struct PaintAssetTemplate {
    pub title: String,
    pub module: LoadedMmsModule,
    pub export_name: String,
    pub param_names: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct StrokeState {
    active: bool,
    non_grid_placed: bool,
    last_grid_step: Option<GridStep>,
}

impl StrokeState {
    fn inactive() -> Self {
        Self {
            active: false,
            non_grid_placed: false,
            last_grid_step: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct PaintUiState {
    selected_asset_title: Option<String>,
    selected_tool_label: Option<String>,
    paint_panel_focused: bool,
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

        let stroke_state = Arc::new(Mutex::new(StrokeState::inactive()));
        let ui_state = Arc::new(Mutex::new(read_ui_state(world, panel_query_root)));

        {
            let ui_state = Arc::clone(&ui_state);
            rx.add_handler_closure(
                SignalKind::SelectionChanged,
                panel_query_root,
                move |world, emit, env| {
                    let Some(EventSignal::SelectionChanged { selection_root, .. }) = env.event.as_ref() else {
                        return;
                    };

                    let matches_assets = world
                        .find_component(panel_query_root, ASSETS_SELECTION_SELECTOR)
                        .is_some_and(|root| root == *selection_root);
                    let matches_tools = world
                        .find_component(panel_query_root, PAINT_TOOL_SELECTION_SELECTOR)
                        .is_some_and(|root| root == *selection_root);
                    let matches_panels = world
                        .find_component(panel_query_root, PANEL_LAYOUT_SELECTION_SELECTOR)
                        .is_some_and(|root| root == *selection_root);

                    if !(matches_assets || matches_tools || matches_panels) {
                        return;
                    }

                    *ui_state.lock().expect("paint ui state mutex poisoned") =
                        read_ui_state(world, panel_query_root);
                    update_paint_status(
                        world,
                        emit,
                        editor_root,
                        panel_query_root,
                        &ui_state,
                        None,
                    );
                },
            );
        }

        for signal_kind in [SignalKind::Click, SignalKind::DragStart, SignalKind::DragMove, SignalKind::DragEnd] {
            let state = Arc::clone(&stroke_state);
            let ui_state = Arc::clone(&ui_state);
            let templates = templates.clone();
            rx.add_handler_closure(signal_kind, editor_root, move |world, emit, env| {
                match env.event.as_ref() {
                    Some(EventSignal::DragStart { renderable, .. }) => {
                        let mut state = state.lock().expect("paint stroke state mutex poisoned");
                        if !eligible_scene_hit(world, editor_root, panel_query_root, *renderable) {
                            *state = StrokeState::inactive();
                            return;
                        }
                        let ui_state_snapshot =
                            ui_state.lock().expect("paint ui state mutex poisoned").clone();
                        if resolve_paint_context(world, editor_root, &ui_state_snapshot, &templates)
                            .is_none()
                        {
                            *state = StrokeState::inactive();
                            update_paint_status(
                                world,
                                emit,
                                editor_root,
                                panel_query_root,
                                &ui_state,
                                None,
                            );
                            return;
                        }
                        *state = StrokeState {
                            active: true,
                            non_grid_placed: false,
                            last_grid_step: None,
                        };
                        update_paint_status(
                            world,
                            emit,
                            editor_root,
                            panel_query_root,
                            &ui_state,
                            None,
                        );
                    }
                    Some(EventSignal::DragMove {
                        renderable,
                        hit_point,
                        ..
                    }) => {
                        let mut state = state.lock().expect("paint stroke state mutex poisoned");
                        if !state.active {
                            return;
                        }
                        let ui_state_snapshot =
                            ui_state.lock().expect("paint ui state mutex poisoned").clone();
                        let Some(context) =
                            resolve_paint_context(world, editor_root, &ui_state_snapshot, &templates)
                        else {
                            return;
                        };
                        if !eligible_scene_hit(world, editor_root, panel_query_root, *renderable) {
                            return;
                        }
                        match context.grid_snap(*hit_point) {
                            Some(grid_snap) => {
                                if GridSystem::same_step(state.last_grid_step, grid_snap.step) {
                                    return;
                                }
                                state.last_grid_step = Some(grid_snap.step);
                                let message = place_asset(
                                    world,
                                    emit,
                                    editor_root,
                                    *renderable,
                                    *hit_point,
                                    &context.asset,
                                    Some(grid_snap),
                                );
                                update_paint_status(
                                    world,
                                    emit,
                                    editor_root,
                                    panel_query_root,
                                    &ui_state,
                                    Some(message),
                                );
                            }
                            None => {
                                if state.non_grid_placed {
                                    return;
                                }
                                state.non_grid_placed = true;
                                let message = place_asset(
                                    world,
                                    emit,
                                    editor_root,
                                    *renderable,
                                    *hit_point,
                                    &context.asset,
                                    None,
                                );
                                update_paint_status(
                                    world,
                                    emit,
                                    editor_root,
                                    panel_query_root,
                                    &ui_state,
                                    Some(message),
                                );
                            }
                        }
                    }
                    Some(EventSignal::Click {
                        renderable,
                        hit_point,
                        ..
                    }) => {
                        if !eligible_scene_hit(world, editor_root, panel_query_root, *renderable) {
                            return;
                        }
                        let ui_state_snapshot =
                            ui_state.lock().expect("paint ui state mutex poisoned").clone();
                        let Some(context) =
                            resolve_paint_context(world, editor_root, &ui_state_snapshot, &templates)
                        else {
                            update_paint_status(
                                world,
                                emit,
                                editor_root,
                                panel_query_root,
                                &ui_state,
                                None,
                            );
                            return;
                        };
                        let mut state = state.lock().expect("paint stroke state mutex poisoned");
                        if state.non_grid_placed {
                            return;
                        }
                        let grid_snap = context.grid_snap(*hit_point);
                        if let Some(snap) = grid_snap {
                            state.last_grid_step = Some(snap.step);
                        }
                        state.non_grid_placed = true;
                        let message = place_asset(
                            world,
                            emit,
                            editor_root,
                            *renderable,
                            *hit_point,
                            &context.asset,
                            grid_snap,
                        );
                        update_paint_status(
                            world,
                            emit,
                            editor_root,
                            panel_query_root,
                            &ui_state,
                            Some(message),
                        );
                    }
                    Some(EventSignal::DragEnd { .. }) => {
                        *state.lock().expect("paint stroke state mutex poisoned") = StrokeState::inactive();
                        update_paint_status(
                            world,
                            emit,
                            editor_root,
                            panel_query_root,
                            &ui_state,
                            None,
                        );
                    }
                    _ => {}
                }
            });
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
    ui_state: &PaintUiState,
    templates: &[PaintAssetTemplate],
) -> Option<PaintContext> {
    if !ui_state.paint_panel_focused {
        return None;
    }
    if ui_state.selected_tool_label.as_deref() != Some(FREE_DRAW_LABEL) {
        return None;
    }
    let asset_title = ui_state.selected_asset_title.clone()?;
    let asset = templates.iter().find(|template| template.title == asset_title)?.clone();
    Some(PaintContext {
        asset,
        active_grid: GridSystem::active_grid_for_editor(world, editor_root),
    })
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

    let pose = match resolve_placement_pose(world, target_renderable, hit_point, asset_root, grid_snap) {
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
                .with_position(pose.translation[0], pose.translation[1], pose.translation[2])
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
    ui_state: &Arc<Mutex<PaintUiState>>,
    override_text: Option<String>,
) {
    let Some(paint_panel_root) = world.find_component(panel_query_root, PAINT_PANEL_ROOT_SELECTOR) else {
        return;
    };
    let Some(status_wrap) = world.find_component(paint_panel_root, PAINT_STATUS_WRAP_SELECTOR) else {
        return;
    };
    let ui_state_snapshot = ui_state.lock().expect("paint ui state mutex poisoned").clone();
    let text = override_text
        .unwrap_or_else(|| base_status_text(world, editor_root, &ui_state_snapshot));
    set_status_text(world, emit, status_wrap, &text);
}

fn base_status_text(world: &World, editor_root: ComponentId, ui_state: &PaintUiState) -> String {
    if ui_state.selected_asset_title.is_none() {
        return "paint inactive: no asset selected".to_string();
    }
    if !ui_state.paint_panel_focused {
        return "paint inactive: focus Paint panel".to_string();
    }
    if ui_state.selected_tool_label.as_deref() != Some(FREE_DRAW_LABEL) {
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
        return;
    };
    let Some(text_component) = world.get_component_by_id_as_mut::<crate::engine::ecs::component::TextComponent>(status_text) else {
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

fn read_ui_state(world: &World, panel_query_root: ComponentId) -> PaintUiState {
    let selected_asset_title = world
        .find_component(panel_query_root, ASSETS_SELECTION_SELECTOR)
        .and_then(|selection| {
            world
                .get_component_by_id_as::<SelectionComponent>(selection)
                .and_then(|selection| selection.selected_item.clone())
        });
    let selected_tool_label = world
        .find_component(panel_query_root, PAINT_TOOL_SELECTION_SELECTOR)
        .and_then(|selection| {
            world
                .get_component_by_id_as::<SelectionComponent>(selection)
                .and_then(|selection| selection.selected_item.clone())
        });
    let paint_panel_focused = world
        .find_component(panel_query_root, PANEL_LAYOUT_SELECTION_SELECTOR)
        .and_then(|selection| {
            world
                .get_component_by_id_as::<SelectionComponent>(selection)
                .and_then(|selection| selection.selected_component)
        })
        .zip(world.find_component(panel_query_root, PAINT_PANEL_ROOT_SELECTOR))
        .is_some_and(|(selected, paint_panel_root)| selected == paint_panel_root);

    PaintUiState {
        selected_asset_title,
        selected_tool_label,
        paint_panel_focused,
    }
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
        if world.get_component_by_id_as::<EditorComponent>(node).is_some() {
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
        if world.get_component_by_id_as::<TransformGizmoComponent>(node).is_some() {
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
    use crate::engine::ecs::component::{ColorComponent, GridComponent};
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

    fn init_editor_fixture() -> (World, CommandQueue, VisualWorld, SystemWorld, RenderAssets, ComponentId, ComponentId, ComponentId, ComponentId) {
        let tmp_dir = temp_asset_directory();
        write_test_asset(&tmp_dir);

        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let render_assets = RenderAssets::new();

        systems.asset_system.scan_assets_dir(&tmp_dir).expect("scan");

        let editor_root = world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root = world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
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
        let paint_panel_root = world.find_component(runtime_ui_root, PAINT_PANEL_ROOT_SELECTOR).expect("paint panel");
        let assets_selection = world.find_component(runtime_ui_root, ASSETS_SELECTION_SELECTOR).expect("assets selection");
        let asset_item = world.find_component(runtime_ui_root, "[name='asset_item']").expect("asset item");

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
        let _ = systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

        assert!(
            world
                .get_component_by_id_as::<SelectionComponent>(assets_selection)
                .expect("selection")
                .selected_item
                .is_some()
        );

        (world, emit, visuals, systems, render_assets, editor_root, scene_root, renderable, paint_panel_root)
    }

    #[test]
    fn paint_inactive_without_asset_selection() {
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let mut visuals = VisualWorld::default();
        let mut systems = SystemWorld::default();
        let render_assets = RenderAssets::new();

        let editor_root = world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_root = world.add_component_boxed_named("scene_root", Box::new(TransformComponent::new()));
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
        let _ = systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

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
        let (mut world, mut emit, mut visuals, mut systems, render_assets, _editor_root, scene_root, renderable, _paint_panel_root) =
            init_editor_fixture();

        push_click(&mut systems, renderable);
        let _ = systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

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
        let (mut world, mut emit, mut visuals, mut systems, render_assets, _editor_root, scene_root, renderable, _paint_panel_root) =
            init_editor_fixture();

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
        let _ = systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

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
        let (mut world, mut emit, mut visuals, mut systems, render_assets, editor_root, scene_root, renderable, _paint_panel_root) =
            init_editor_fixture();

        let grid = world.add_component_boxed_named(
            "grid",
            Box::new(GridComponent::new(0.5)),
        );
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
        let _ = systems.process_signals(&mut world, &mut visuals, &render_assets, &mut emit, 100_000);

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
