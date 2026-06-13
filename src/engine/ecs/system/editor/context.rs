use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::engine::ecs::component::{
    ColorComponent, EditorComponent, EditorInteractionMode, EmissiveComponent, OpacityComponent,
    RaycastableComponent, RenderableComponent, SelectableComponent, SelectionComponent,
    SerializeComponent, SignalObserverRouterComponent, TransformComponent,
};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::system::editor::settings_panel::{
    EDITOR_SETTINGS_PAYLOAD_NAME, EDITOR_SETTINGS_SELECTION_SELECTOR, EditorSettingsOption,
};
use crate::engine::ecs::system::selection_system::resolve_semantic_target_from_payload;
use crate::engine::ecs::{
    ComponentId, EventSignal, IntentValue, RxWorld, Signal, SignalEmitter, SignalKind, World,
};
use crate::engine::graphics::primitives::{CpuMeshHandle, MaterialHandle};
use crate::utils::math::mat_to_quat;
use std::f32::consts::FRAC_PI_2;

const PANEL_LAYOUT_SELECTION_SELECTOR: &str = "#editor_panel_layout_selection";
const WORLD_PANEL_SELECTION_SELECTOR: &str = "#world_panel_selection";
const ASSETS_SELECTION_SELECTOR: &str = "#assets_selection";
const PAINT_PANEL_ROOT_SELECTOR: &str = "#paint_panel_root";
pub const EDITOR_WORKSPACE_ASSET_SELECTION_CHANGED: &str = "EditorWorkspaceAssetSelectionChanged";
const PAINT_SYSTEM_HANDLER_NAME: &str = "paint_system";
const EDITOR_PANEL_REFRESH_HANDLER_NAME: &str = "editor_panel_refresh";
const DEBUG_BLACKLIST_EDITOR_PANEL_REFRESH: bool = true;
const CURSOR_MARKER_ROOT_NAME: &str = "editor_cursor_marker";
const CURSOR_MARKER_SIZE: f32 = 0.5;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EditorContextState {
    pub active_editor: Option<ComponentId>,
    pub selected_component: Option<ComponentId>,
    pub selected_asset_payload: Option<ComponentId>,
    pub focused_panel: Option<ComponentId>,
    pub interaction_mode: EditorInteractionMode,
    pub cursor_translation: Option<[f32; 3]>,
    pub cursor_rotation: Option<[f32; 4]>,
}

#[derive(Debug, Clone, Default)]
struct EditorContextWorkspaceState {
    panel_query_root: Option<ComponentId>,
    registered_editors: Vec<ComponentId>,
}

impl EditorContextWorkspaceState {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorContextEvent {
    ActiveEditorChanged {
        editor: Option<ComponentId>,
        selected_component: Option<ComponentId>,
        interaction_mode: EditorInteractionMode,
    },
    PanelFocusChanged {
        focused_panel: Option<ComponentId>,
    },
    WorldPanelSelectionChanged {
        component: Option<ComponentId>,
        editor: Option<ComponentId>,
        interaction_mode: EditorInteractionMode,
    },
    AssetPanelSelectionChanged {
        asset_payload: Option<ComponentId>,
    },
    EditorSelectionChanged {
        editor: ComponentId,
        component: Option<ComponentId>,
        interaction_mode: EditorInteractionMode,
    },
    InteractionModeChanged {
        editor: Option<ComponentId>,
        interaction_mode: EditorInteractionMode,
    },
}

pub fn reduce_editor_context_state(
    old: &EditorContextState,
    event: &EditorContextEvent,
) -> EditorContextState {
    let mut new = old.clone();
    match event {
        EditorContextEvent::ActiveEditorChanged {
            editor,
            selected_component,
            interaction_mode,
        } => {
            new.active_editor = *editor;
            new.selected_component = *selected_component;
            new.interaction_mode = *interaction_mode;
        }
        EditorContextEvent::PanelFocusChanged { focused_panel } => {
            new.focused_panel = *focused_panel;
        }
        EditorContextEvent::WorldPanelSelectionChanged {
            component,
            editor,
            interaction_mode,
        } => {
            new.selected_component = *component;
            if editor.is_some() {
                new.active_editor = *editor;
            }
            new.interaction_mode = *interaction_mode;
        }
        EditorContextEvent::AssetPanelSelectionChanged { asset_payload } => {
            new.selected_asset_payload = *asset_payload;
        }
        EditorContextEvent::EditorSelectionChanged {
            editor,
            component,
            interaction_mode,
        } => {
            new.active_editor = Some(*editor);
            new.selected_component = component.or(Some(*editor));
            new.interaction_mode = *interaction_mode;
        }
        EditorContextEvent::InteractionModeChanged {
            editor,
            interaction_mode,
        } => {
            if editor.is_some() {
                new.active_editor = *editor;
            }
            new.interaction_mode = *interaction_mode;
        }
    }
    new
}

#[derive(Debug, Default)]
pub struct EditorContextSystem {
    installed_editor_roots: HashSet<ComponentId>,
    shared_panel_handlers_installed: bool,
    state: Arc<Mutex<EditorContextState>>,
    workspace: Arc<Mutex<EditorContextWorkspaceState>>,
}

impl EditorContextSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shared_state(&self) -> Arc<Mutex<EditorContextState>> {
        Arc::clone(&self.state)
    }

    pub fn install_scoped_handlers_for_editor(
        &mut self,
        rx: &mut RxWorld,
        world: &mut World,
        editor_root: ComponentId,
        panel_query_root: ComponentId,
    ) {
        ensure_editor_observer_router(world, editor_root);
        {
            let mut workspace = self
                .workspace
                .lock()
                .expect("editor context workspace poisoned");
            workspace.panel_query_root = Some(panel_query_root);
            let registered = workspace.register_editor(editor_root);
            drop(workspace);

            if registered {
                ensure_default_active_editor(&self.state, &self.workspace);
            }
        }

        if !self.shared_panel_handlers_installed {
            self.shared_panel_handlers_installed = true;
            install_shared_panel_handlers(
                rx,
                world,
                panel_query_root,
                Arc::clone(&self.state),
                Arc::clone(&self.workspace),
            );
            bootstrap_editor_context(world, panel_query_root, &self.state, &self.workspace);
            sync_editor_observer_routes(world, &self.state, &self.workspace);
        }

        if self.installed_editor_roots.insert(editor_root) {
            install_editor_handlers(rx, editor_root, Arc::clone(&self.state));
        }
        sync_editor_observer_routes(world, &self.state, &self.workspace);
    }
}

fn install_shared_panel_handlers(
    rx: &mut RxWorld,
    world: &World,
    panel_query_root: ComponentId,
    state: Arc<Mutex<EditorContextState>>,
    workspace: Arc<Mutex<EditorContextWorkspaceState>>,
) {
    let _ = world;
    rx.add_handler_closure_named(
        SignalKind::SelectionChanged,
        panel_query_root,
        Some("editor_system".to_string()),
        move |world, emit, signal| {
            let Some(event) =
                editor_context_event_from_shared_signal(world, panel_query_root, signal)
            else {
                return;
            };
            apply_editor_context_event(&state, &event);
            emit_editor_workspace_data_event(world, emit, panel_query_root, &event);
            sync_editor_component_selection(world, &event);
            sync_editor_cursor_pose(world, &state);
            sync_editor_cursor_visual(world, emit, &state);
            sync_editor_observer_routes(world, &state, &workspace);
        },
    );
}

fn install_editor_handlers(
    rx: &mut RxWorld,
    editor_root: ComponentId,
    state: Arc<Mutex<EditorContextState>>,
) {
    rx.add_handler_closure_named(
        SignalKind::SelectionChanged,
        editor_root,
        Some("editor_system".to_string()),
        move |world, _emit, signal| {
            let Some(EventSignal::SelectionChanged {
                selection_root,
                selected_component,
                ..
            }) = signal.event.as_ref()
            else {
                return;
            };
            if *selection_root != editor_root {
                return;
            }

            let event = EditorContextEvent::EditorSelectionChanged {
                editor: editor_root,
                component: *selected_component,
                interaction_mode: editor_interaction_mode(world, Some(editor_root)),
            };
            apply_editor_context_event(&state, &event);
            sync_editor_component_selection(world, &event);
            sync_editor_cursor_pose(world, &state);
            sync_editor_cursor_visual(world, _emit, &state);
        },
    );
}

fn ensure_editor_observer_router(world: &mut World, editor_root: ComponentId) -> ComponentId {
    if let Some(existing) = world
        .children_of(editor_root)
        .iter()
        .copied()
        .find(|&child| {
            world
                .get_component_by_id_as::<SignalObserverRouterComponent>(child)
                .is_some()
        })
    {
        return existing;
    }

    let router = world.add_component_boxed_named(
        "editor_signal_observer_router",
        Box::new(SignalObserverRouterComponent::new()),
    );
    let _ = world.add_child(editor_root, router);
    router
}

fn bootstrap_editor_context(
    world: &World,
    panel_query_root: ComponentId,
    state: &Arc<Mutex<EditorContextState>>,
    workspace: &Arc<Mutex<EditorContextWorkspaceState>>,
) {
    if let Some(selection_root) =
        world.find_component(panel_query_root, PANEL_LAYOUT_SELECTION_SELECTOR)
        && let Some(selection) = world.get_component_by_id_as::<SelectionComponent>(selection_root)
    {
        apply_editor_context_event(
            state,
            &EditorContextEvent::PanelFocusChanged {
                focused_panel: selection.selected_component,
            },
        );
    }

    if let Some(selection_root) =
        world.find_component(panel_query_root, WORLD_PANEL_SELECTION_SELECTOR)
        && let Some(selection) = world.get_component_by_id_as::<SelectionComponent>(selection_root)
    {
        if let Some(event) = world_panel_selection_event(world, selection) {
            apply_editor_context_event(state, &event);
            sync_editor_cursor_pose(world, state);
        }
    } else if let Some(editor_root) = workspace
        .lock()
        .expect("editor context workspace poisoned")
        .default_active_editor()
    {
        apply_editor_context_event(
            state,
            &EditorContextEvent::ActiveEditorChanged {
                editor: Some(editor_root),
                selected_component: Some(editor_root),
                interaction_mode: editor_interaction_mode(world, Some(editor_root)),
            },
        );
        sync_editor_cursor_pose(world, state);
    }

    if let Some(selection_root) = world.find_component(panel_query_root, ASSETS_SELECTION_SELECTOR)
        && let Some(selection) = world.get_component_by_id_as::<SelectionComponent>(selection_root)
    {
        apply_editor_context_event(
            state,
            &EditorContextEvent::AssetPanelSelectionChanged {
                asset_payload: selection.selected_payload.or(selection.selected_component),
            },
        );
    }
}

fn editor_context_event_from_shared_signal(
    world: &World,
    panel_query_root: ComponentId,
    signal: &Signal,
) -> Option<EditorContextEvent> {
    let EventSignal::SelectionChanged {
        selection_root,
        selected_entries,
        selected_component,
        selected_payload,
        ..
    } = signal.event.as_ref()?
    else {
        return None;
    };

    let component =
        selected_component.or_else(|| selected_entries.last().map(|entry| entry.component));
    let is_panel_layout_selection = world.component_label(*selection_root)
        == Some(PANEL_LAYOUT_SELECTION_SELECTOR.trim_start_matches('#'))
        || world.find_component(panel_query_root, PANEL_LAYOUT_SELECTION_SELECTOR)
            == Some(*selection_root);
    let is_world_panel_selection = world.component_label(*selection_root)
        == Some(WORLD_PANEL_SELECTION_SELECTOR.trim_start_matches('#'))
        || world.find_component(panel_query_root, WORLD_PANEL_SELECTION_SELECTOR)
            == Some(*selection_root);
    let is_assets_selection = world.component_label(*selection_root)
        == Some(ASSETS_SELECTION_SELECTOR.trim_start_matches('#'))
        || world.find_component(panel_query_root, ASSETS_SELECTION_SELECTOR)
            == Some(*selection_root);
    let is_editor_settings_selection = world.component_label(*selection_root)
        == Some(EDITOR_SETTINGS_SELECTION_SELECTOR.trim_start_matches('#'))
        || world.find_component(panel_query_root, EDITOR_SETTINGS_SELECTION_SELECTOR)
            == Some(*selection_root);

    if is_panel_layout_selection {
        Some(EditorContextEvent::PanelFocusChanged {
            focused_panel: component,
        })
    } else if is_editor_settings_selection {
        let row_name = selected_payload
            .or(component)
            .and_then(|id| editor_settings_row_name(world, id));
        let option = row_name
            .as_deref()
            .and_then(EditorSettingsOption::from_row_name)?;
        let active_editor = current_or_default_editor_root(world, panel_query_root, component);
        Some(EditorContextEvent::InteractionModeChanged {
            editor: active_editor,
            interaction_mode: option.interaction_mode(),
        })
    } else if is_world_panel_selection {
        let semantic_target =
            resolve_semantic_target_from_payload(world, *selected_payload, component);
        let active_editor = semantic_target.and_then(|target| nearest_editor_ancestor(world, target));
        println!(
            "[EditorContext][trace] world_panel selection_root={selection_root:?} clicked_row={selected_component:?} payload={selected_payload:?} authored_target={semantic_target:?} active_editor={:?}",
            active_editor
        );
        Some(EditorContextEvent::WorldPanelSelectionChanged {
            component: semantic_target,
            editor: active_editor,
            interaction_mode: editor_interaction_mode(world, active_editor),
        })
    } else if is_assets_selection {
        Some(EditorContextEvent::AssetPanelSelectionChanged {
            asset_payload: selected_payload.or(component),
        })
    } else {
        None
    }
}

fn apply_editor_context_event(state: &Arc<Mutex<EditorContextState>>, event: &EditorContextEvent) {
    let mut state = state.lock().expect("editor context state poisoned");
    *state = reduce_editor_context_state(&state, event);
}

fn emit_editor_workspace_data_event(
    world: &World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    event: &EditorContextEvent,
) {
    let Some(runtime_ui_root) = world
        .all_components()
        .find(|&component_id| {
            world.parent_of(component_id).is_none()
                && world.component_label(component_id) == Some("editor_runtime_ui_root")
        })
        .or(Some(panel_query_root))
    else {
        return;
    };

    if let EditorContextEvent::AssetPanelSelectionChanged { asset_payload } = event {
        emit.push_event(
            runtime_ui_root,
            EventSignal::DataEvent {
                name: EDITOR_WORKSPACE_ASSET_SELECTION_CHANGED.to_string(),
                payload: *asset_payload,
            },
        );
    }
}

fn editor_interaction_mode(world: &World, editor_root: Option<ComponentId>) -> EditorInteractionMode {
    editor_root
        .and_then(|editor_root| {
            world
                .get_component_by_id_as::<EditorComponent>(editor_root)
                .map(|editor| editor.interaction_mode)
        })
        .unwrap_or(EditorInteractionMode::Select)
}

fn current_or_default_editor_root(
    world: &World,
    panel_query_root: ComponentId,
    component: Option<ComponentId>,
) -> Option<ComponentId> {
    component
        .and_then(|component| nearest_editor_ancestor(world, component))
        .or_else(|| {
            world
                .find_component(panel_query_root, WORLD_PANEL_SELECTION_SELECTOR)
                .and_then(|selection_root| {
                    world
                        .get_component_by_id_as::<SelectionComponent>(selection_root)
                        .and_then(|selection| world_panel_selection_event(world, selection))
                        .and_then(|event| match event {
                            EditorContextEvent::WorldPanelSelectionChanged { editor, .. } => editor,
                            _ => None,
                        })
                })
        })
}

fn editor_settings_row_name(world: &World, payload_or_row: ComponentId) -> Option<String> {
    if let Some(data) = world.get_component_by_id_as::<crate::engine::ecs::component::DataComponent>(payload_or_row)
        && world.component_label(payload_or_row) == Some(EDITOR_SETTINGS_PAYLOAD_NAME)
        && let Some(crate::engine::ecs::component::DataValue::Text(row_name)) = data.get("row_name")
    {
        return Some(row_name.clone());
    }

    world.children_of(payload_or_row).iter().find_map(|&child| {
        let data = world.get_component_by_id_as::<crate::engine::ecs::component::DataComponent>(child)?;
        if world.component_label(child) != Some(EDITOR_SETTINGS_PAYLOAD_NAME) {
            return None;
        }
        match data.get("row_name") {
            Some(crate::engine::ecs::component::DataValue::Text(row_name)) => Some(row_name.clone()),
            _ => None,
        }
    })
}

fn sync_editor_cursor_pose(world: &World, state: &Arc<Mutex<EditorContextState>>) {
    let mut state = state.lock().expect("editor context state poisoned");
    let Some(selected_component) = state.selected_component else {
        state.cursor_translation = None;
        state.cursor_rotation = None;
        return;
    };
    let Some(world_model) = TransformSystem::world_model(world, selected_component) else {
        state.cursor_translation = None;
        state.cursor_rotation = None;
        return;
    };

    state.cursor_translation = Some([world_model[3][0], world_model[3][1], world_model[3][2]]);
    state.cursor_rotation = Some(mat_to_quat(world_model));
}

fn sync_editor_cursor_visual(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    state: &Arc<Mutex<EditorContextState>>,
) {
    let state = state.lock().expect("editor context state poisoned").clone();
    let Some(editor_root) = state.active_editor else {
        return;
    };
    let marker = ensure_cursor_marker(world, emit, editor_root);
    let Some(marker_transform) = world.get_component_by_id_as_mut::<TransformComponent>(marker)
    else {
        return;
    };

    let translation = state.cursor_translation.unwrap_or([0.0, 0.0, 0.0]);
    let rotation = state.cursor_rotation.unwrap_or([0.0, 0.0, 0.0, 1.0]);
    marker_transform.transform.translation = translation;
    marker_transform.transform.rotation = rotation;
    marker_transform.transform.scale = [CURSOR_MARKER_SIZE, CURSOR_MARKER_SIZE, CURSOR_MARKER_SIZE];
    marker_transform.transform.recompute_model();

    emit.push_intent_now(
        marker,
        IntentValue::UpdateTransform {
            component_ids: vec![marker],
            translation,
            rotation_quat_xyzw: rotation,
            scale: [CURSOR_MARKER_SIZE, CURSOR_MARKER_SIZE, CURSOR_MARKER_SIZE],
        },
    );

    let opacity_ids = cursor_marker_opacities(world, marker);
    let target_opacity = if state.selected_component.is_some() {
        0.35
    } else {
        0.0
    };
    for opacity_id in &opacity_ids {
        if let Some(opacity) = world.get_component_by_id_as_mut::<OpacityComponent>(*opacity_id) {
            opacity.opacity = target_opacity;
        }
    }
    if !opacity_ids.is_empty() {
        emit.push_intent_now(
            marker,
            IntentValue::RegisterOpacity {
                component_ids: opacity_ids,
            },
        );
    }
}

fn ensure_cursor_marker(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    editor_root: ComponentId,
) -> ComponentId {
    if let Some(existing) = world.children_of(editor_root).iter().copied().find(|&child| {
        world.component_label(child) == Some(CURSOR_MARKER_ROOT_NAME)
    }) {
        return existing;
    }

    let marker_root = world.add_component_boxed_named(
        CURSOR_MARKER_ROOT_NAME,
        Box::new(TransformComponent::new().with_scale(
            CURSOR_MARKER_SIZE,
            CURSOR_MARKER_SIZE,
            CURSOR_MARKER_SIZE,
        )),
    );
    let marker_selectable = world.add_component_boxed_named(
        "editor_cursor_marker_selectable",
        Box::new(SelectableComponent::off()),
    );
    let marker_serialize = world.add_component_boxed_named(
        "editor_cursor_marker_serialize",
        Box::new(SerializeComponent::off()),
    );
    let _ = world.add_child(editor_root, marker_root);
    let _ = world.add_child(marker_root, marker_selectable);
    let _ = world.add_child(marker_root, marker_serialize);

    let half_extent = CURSOR_MARKER_SIZE * 0.5;
    let plane_scale = CURSOR_MARKER_SIZE;
    let plane_roots = [
        (
            "editor_cursor_marker_x_plane_root",
            TransformComponent::new()
                .with_position(half_extent, 0.0, 0.0)
                .with_rotation_euler(0.0, FRAC_PI_2, 0.0)
                .with_scale(plane_scale, plane_scale, plane_scale),
            [0.0, 0.0, 1.0, 1.0],
        ),
        (
            "editor_cursor_marker_y_plane_root",
            TransformComponent::new()
                .with_position(0.0, half_extent, 0.0)
                .with_rotation_euler(-FRAC_PI_2, 0.0, 0.0)
                .with_scale(plane_scale, plane_scale, plane_scale),
            [0.0, 1.0, 0.0, 1.0],
        ),
        (
            "editor_cursor_marker_z_plane_root",
            TransformComponent::new()
                .with_position(0.0, 0.0, half_extent)
                .with_scale(plane_scale, plane_scale, plane_scale),
            [1.0, 0.0, 0.0, 1.0],
        ),
    ];

    let mut renderable_ids = Vec::new();
    for (name, transform, color) in plane_roots {
        let plane_root = world.add_component_boxed_named(name, Box::new(transform));
        let plane_renderable = world.add_component_boxed_named(
            &format!("{name}_renderable"),
            Box::new(RenderableComponent::from_cpu_mesh_handle(
                CpuMeshHandle::QUAD_2D,
                MaterialHandle::TOON_MESH,
            )),
        );
        let plane_raycastable = world.add_component_boxed_named(
            &format!("{name}_raycastable"),
            Box::new(RaycastableComponent::disabled()),
        );
        let plane_color = world.add_component_boxed_named(
            &format!("{name}_color"),
            Box::new(ColorComponent::rgba(color[0], color[1], color[2], color[3])),
        );
        let plane_opacity = world.add_component_boxed_named(
            &format!("{name}_opacity"),
            Box::new(OpacityComponent::new().with_opacity(0.0)),
        );
        let plane_emissive = world.add_component_boxed_named(
            &format!("{name}_emissive"),
            Box::new(EmissiveComponent::new(1.0)),
        );

        let _ = world.add_child(marker_root, plane_root);
        let _ = world.add_child(plane_root, plane_renderable);
        let _ = world.add_child(plane_renderable, plane_raycastable);
        let _ = world.add_child(plane_renderable, plane_color);
        let _ = world.add_child(plane_renderable, plane_opacity);
        let _ = world.add_child(plane_renderable, plane_emissive);
        renderable_ids.push(plane_renderable);
    }

    world.init_component_tree(marker_root, emit);
    emit.push_intent_now(
        marker_root,
        IntentValue::RegisterTransform {
            component_ids: vec![marker_root],
        },
    );
    emit.push_intent_now(
        marker_root,
        IntentValue::RegisterRenderable {
            component_ids: renderable_ids,
        },
    );
    marker_root
}

fn cursor_marker_opacities(world: &World, marker_root: ComponentId) -> Vec<ComponentId> {
    let mut opacities = Vec::new();
    for &child in world.children_of(marker_root) {
        for &grandchild in world.children_of(child) {
            if world
                .get_component_by_id_as::<RenderableComponent>(grandchild)
                .is_some()
            {
                for &style_child in world.children_of(grandchild) {
                    if world
                        .get_component_by_id_as::<OpacityComponent>(style_child)
                        .is_some()
                    {
                        opacities.push(style_child);
                    }
                }
            }
        }
    }
    opacities
}

fn sync_editor_observer_routes(
    world: &mut World,
    state: &Arc<Mutex<EditorContextState>>,
    workspace: &Arc<Mutex<EditorContextWorkspaceState>>,
) {
    let editor_context = state.lock().expect("editor context state poisoned").clone();
    let workspace = workspace
        .lock()
        .expect("editor context workspace poisoned")
        .clone();
    let paint_panel_root = workspace.panel_query_root.and_then(|panel_query_root| {
        world.find_component(panel_query_root, PAINT_PANEL_ROOT_SELECTOR)
    });
    let paint_focused =
        paint_panel_root.is_some_and(|panel| editor_context.focused_panel == Some(panel));

    for editor_root in workspace.registered_editors {
        let router_id = ensure_editor_observer_router(world, editor_root);
        let Some(router) =
            world.get_component_by_id_as_mut::<SignalObserverRouterComponent>(router_id)
        else {
            continue;
        };

        if paint_focused {
            router
                .blacklist
                .retain(|name| name != PAINT_SYSTEM_HANDLER_NAME);
        } else if !router
            .blacklist
            .iter()
            .any(|name| name == PAINT_SYSTEM_HANDLER_NAME)
        {
            router.blacklist.push(PAINT_SYSTEM_HANDLER_NAME.to_string());
        }

        if DEBUG_BLACKLIST_EDITOR_PANEL_REFRESH {
            if !router
                .blacklist
                .iter()
                .any(|name| name == EDITOR_PANEL_REFRESH_HANDLER_NAME)
            {
                router
                    .blacklist
                    .push(EDITOR_PANEL_REFRESH_HANDLER_NAME.to_string());
            }
        } else {
            router
                .blacklist
                .retain(|name| name != EDITOR_PANEL_REFRESH_HANDLER_NAME);
        }
    }
}

fn ensure_default_active_editor(
    state: &Arc<Mutex<EditorContextState>>,
    workspace: &Arc<Mutex<EditorContextWorkspaceState>>,
) {
    let default_editor = workspace
        .lock()
        .expect("editor context workspace poisoned")
        .default_active_editor();
    let Some(default_editor) = default_editor else {
        return;
    };

    let mut state = state.lock().expect("editor context state poisoned");
    if state.active_editor.is_none() {
        *state = reduce_editor_context_state(
            &state,
            &EditorContextEvent::ActiveEditorChanged {
                editor: Some(default_editor),
                selected_component: Some(default_editor),
                interaction_mode: EditorInteractionMode::Select,
            },
        );
    }
}

fn nearest_editor_ancestor(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut cur = Some(start);
    while let Some(node) = cur {
        if world
            .get_component_by_id_as::<crate::engine::ecs::component::EditorComponent>(node)
            .is_some()
        {
            return Some(node);
        }
        cur = world.parent_of(node);
    }
    None
}

fn world_panel_selection_event(
    world: &World,
    selection: &SelectionComponent,
) -> Option<EditorContextEvent> {
    let semantic_target = resolve_semantic_target_from_payload(
        world,
        selection.selected_payload,
        selection.selected_component,
    )?;
    Some(EditorContextEvent::WorldPanelSelectionChanged {
        component: Some(semantic_target),
        editor: nearest_editor_ancestor(world, semantic_target),
        interaction_mode: editor_interaction_mode(
            world,
            nearest_editor_ancestor(world, semantic_target),
        ),
    })
}

fn sync_editor_component_selection(world: &mut World, event: &EditorContextEvent) {
    match event {
        EditorContextEvent::WorldPanelSelectionChanged {
            component,
            editor,
            interaction_mode,
        } => {
            let Some(editor_root) = *editor else {
                return;
            };
            if let Some(editor_component) =
                world.get_component_by_id_as_mut::<EditorComponent>(editor_root)
            {
                editor_component.selected = *component;
                editor_component.interaction_mode = *interaction_mode;
            }
        }
        EditorContextEvent::EditorSelectionChanged {
            editor,
            component,
            interaction_mode,
        } => {
            if let Some(editor_component) =
                world.get_component_by_id_as_mut::<EditorComponent>(*editor)
            {
                editor_component.selected = component.or(Some(*editor));
                editor_component.interaction_mode = *interaction_mode;
            }
        }
        EditorContextEvent::ActiveEditorChanged {
            editor,
            selected_component,
            interaction_mode,
        } => {
            let Some(editor_root) = *editor else {
                return;
            };
            if let Some(editor_component) =
                world.get_component_by_id_as_mut::<EditorComponent>(editor_root)
            {
                editor_component.selected = *selected_component;
                editor_component.interaction_mode = *interaction_mode;
            }
        }
        EditorContextEvent::AssetPanelSelectionChanged { .. } => {}
        EditorContextEvent::InteractionModeChanged {
            editor,
            interaction_mode,
        } => {
            let Some(editor_root) = *editor else {
                return;
            };
            if let Some(editor_component) =
                world.get_component_by_id_as_mut::<EditorComponent>(editor_root)
            {
                editor_component.interaction_mode = *interaction_mode;
            }
        }
        EditorContextEvent::PanelFocusChanged { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EditorContextEvent, EditorContextState, reduce_editor_context_state,
        sync_editor_component_selection, world_panel_selection_event,
    };
    use crate::engine::ecs::World;
    use crate::engine::ecs::component::{
        DataComponent, DataValue, EditorComponent, EditorInteractionMode, SelectionComponent,
        TransformComponent,
    };

    fn cid(world: &mut World) -> crate::engine::ecs::ComponentId {
        world.add_component_boxed(Box::new(TransformComponent::new()))
    }

    #[test]
    fn defaults_to_first_editor_root_selection() {
        let mut world = World::default();
        let editor = cid(&mut world);
        let next = reduce_editor_context_state(
            &EditorContextState::default(),
            &EditorContextEvent::ActiveEditorChanged {
                editor: Some(editor),
                selected_component: Some(editor),
                interaction_mode: EditorInteractionMode::Select,
            },
        );

        assert_eq!(next.active_editor, Some(editor));
        assert_eq!(next.selected_component, Some(editor));
    }

    #[test]
    fn world_panel_root_selection_switches_active_editor() {
        let mut world = World::default();
        let selected = cid(&mut world);
        let editor = cid(&mut world);
        let next = reduce_editor_context_state(
            &EditorContextState::default(),
            &EditorContextEvent::WorldPanelSelectionChanged {
                component: Some(selected),
                editor: Some(editor),
                interaction_mode: EditorInteractionMode::Select,
            },
        );

        assert_eq!(next.active_editor, Some(editor));
        assert_eq!(next.selected_component, Some(selected));
    }

    #[test]
    fn scene_selection_switches_editor_and_component_together() {
        let mut world = World::default();
        let editor = cid(&mut world);
        let selected = cid(&mut world);
        let next = reduce_editor_context_state(
            &EditorContextState::default(),
            &EditorContextEvent::EditorSelectionChanged {
                editor,
                component: Some(selected),
                interaction_mode: EditorInteractionMode::Select,
            },
        );

        assert_eq!(next.active_editor, Some(editor));
        assert_eq!(next.selected_component, Some(selected));
    }

    #[test]
    fn panel_focus_updates_independently() {
        let mut world = World::default();
        let panel = cid(&mut world);
        let next = reduce_editor_context_state(
            &EditorContextState::default(),
            &EditorContextEvent::PanelFocusChanged {
                focused_panel: Some(panel),
            },
        );

        assert_eq!(next.focused_panel, Some(panel));
    }

    #[test]
    fn interaction_mode_changes_without_clearing_selection() {
        let mut world = World::default();
        let editor = cid(&mut world);
        let selected = cid(&mut world);
        let next = reduce_editor_context_state(
            &EditorContextState {
                active_editor: Some(editor),
                selected_component: Some(selected),
                selected_asset_payload: None,
                focused_panel: None,
                interaction_mode: EditorInteractionMode::Select,
                cursor_translation: None,
                cursor_rotation: None,
            },
            &EditorContextEvent::InteractionModeChanged {
                editor: Some(editor),
                interaction_mode: EditorInteractionMode::Cursor3d,
            },
        );

        assert_eq!(next.active_editor, Some(editor));
        assert_eq!(next.selected_component, Some(selected));
        assert_eq!(next.interaction_mode, EditorInteractionMode::Cursor3d);
    }

    #[test]
    fn editor_root_selection_is_preserved_when_only_root_is_active() {
        let mut world = World::default();
        let editor = cid(&mut world);
        let next = reduce_editor_context_state(
            &EditorContextState::default(),
            &EditorContextEvent::WorldPanelSelectionChanged {
                component: Some(editor),
                editor: Some(editor),
                interaction_mode: EditorInteractionMode::Select,
            },
        );

        assert_eq!(next.active_editor, Some(editor));
        assert_eq!(next.selected_component, Some(editor));
    }

    #[test]
    fn world_panel_payload_event_prefers_semantic_target_over_clicked_row() {
        let mut world = World::default();
        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_target =
            world.add_component_boxed_named("scene_target", Box::new(TransformComponent::new()));
        let row = world.add_component_boxed_named("item_1", Box::new(TransformComponent::new()));
        let payload = world.add_component_boxed_named(
            "world_panel_payload",
            Box::new(
                DataComponent::new()
                    .with_entry("target_component", DataValue::Component(scene_target)),
            ),
        );
        let _ = world.add_child(editor_root, scene_target);
        let _ = world.add_child(row, payload);

        let mut selection = SelectionComponent::new();
        selection.selected_component = Some(row);
        selection.selected_payload = Some(payload);
        let event = world_panel_selection_event(&world, &selection).expect("event");

        assert_eq!(
            event,
            EditorContextEvent::WorldPanelSelectionChanged {
                component: Some(scene_target),
                editor: Some(editor_root),
                interaction_mode: EditorInteractionMode::Select,
            }
        );
    }

    #[test]
    fn world_panel_payload_sync_updates_editor_selected_to_semantic_target() {
        let mut world = World::default();
        let editor_root =
            world.add_component_boxed_named("editor_root", Box::new(EditorComponent::new()));
        let scene_target =
            world.add_component_boxed_named("scene_target", Box::new(TransformComponent::new()));
        let _ = world.add_child(editor_root, scene_target);

        sync_editor_component_selection(
            &mut world,
            &EditorContextEvent::WorldPanelSelectionChanged {
                component: Some(scene_target),
                editor: Some(editor_root),
                interaction_mode: EditorInteractionMode::Select,
            },
        );

        assert_eq!(
            world
                .get_component_by_id_as::<EditorComponent>(editor_root)
                .expect("editor")
                .selected,
            Some(scene_target)
        );
    }
}
