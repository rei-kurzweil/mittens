use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use crate::engine::ecs::component::{
    DataComponent, EditorComponent, SelectionComponent, SelectionEntry, TransformGizmoComponent,
};
use crate::engine::ecs::system::data_renderer_system::{
    DataRendererSystem, ItemRendererSpec, RendererSpec, UiItem, UiItemKind,
};
use crate::engine::ecs::system::editor::context::{
    EditorContextState, apply_semantic_target_selection,
};
use crate::engine::ecs::system::editor::panel_ui::{
    PanelUiRowSpec, spawn_block_container, spawn_panel_ui_row_tree,
};
use crate::engine::ecs::system::panel_system::{
    EDITOR_RUNTIME_UI_ROOT_NAME, PanelActionKind, PanelKind, decode_panel_action_payload,
    find_named_root, is_descendant_or_self,
};
use crate::engine::ecs::system::selection_system::{
    apply_selection_set, resolve_semantic_target_from_payload,
};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::scripting::component_registry::{
    filtered_root_ids_for_roots, filtered_roots_to_ce_ast, spawn_tree,
};
use crate::scripting::object::{MaterializedCE, Value};
use crate::scripting::runner::MeowMeowRunner;

pub const ITEM_PREFIX: &str = "item_";
pub const WORLD_PANEL_PAYLOAD_NAME: &str = "world_panel_payload";
pub const WORLD_PANEL_SELECTION_NAME: &str = "world_panel_selection";
pub(crate) const PANEL_STATUS_ROOT_SELECTOR: &str = "#panel_status_root";
pub(crate) const PANEL_STATUS_WRAP_SELECTOR: &str = "#save_status_wrap";

// ── State + Event + Reducer ─────────────────────────────────────────

pub(crate) struct WorldPanelState {
    pub(crate) scene_model: AuthoredWorldPanelSceneModel,
    pub(crate) selected_component: Option<ComponentId>,
    pub(crate) active_editor: Option<ComponentId>,
}

impl Default for WorldPanelState {
    fn default() -> Self {
        Self {
            scene_model: AuthoredWorldPanelSceneModel::default(),
            selected_component: None,
            active_editor: None,
        }
    }
}

pub(crate) enum WorldPanelEvent {
    SelectionChanged {
        component: Option<ComponentId>,
        editor: Option<ComponentId>,
    },
    RebuildSceneModel {
        sections: Vec<AuthoredWorldPanelSection>,
    },
    ClearSelection,
}

pub(crate) fn reduce_world_panel_state(
    old: &WorldPanelState,
    event: &WorldPanelEvent,
) -> WorldPanelState {
    let mut new = WorldPanelState {
        scene_model: old.scene_model.clone(),
        selected_component: old.selected_component,
        active_editor: old.active_editor,
    };

    match event {
        WorldPanelEvent::SelectionChanged { component, editor } => {
            new.selected_component = *component;
            if editor.is_some() {
                new.active_editor = *editor;
            }
        }
        WorldPanelEvent::RebuildSceneModel { sections } => {
            new.scene_model.sections = sections.clone();
        }
        WorldPanelEvent::ClearSelection => {
            new.selected_component = None;
        }
    }

    new
}

// ── Scene model types ───────────────────────────────────────────────

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthoredWorldPanelSceneModel {
    pub sections: Vec<AuthoredWorldPanelSection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredWorldPanelSection {
    pub editor_root: ComponentId,
    pub rows: Vec<AuthoredWorldPanelRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredWorldPanelRow {
    pub target_component: ComponentId,
    pub label: String,
    pub depth: usize,
}

// ── Render model types ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldPanelModel {
    pub title: String,
    pub rows: Vec<WorldPanelRow>,
    pub selected_index: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldPanelRow {
    pub target_component: Option<ComponentId>,
    pub label: String,
    pub display_label: String,
    pub kind: WorldPanelRowKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldPanelRowKind {
    Spacer,
    EditorRoot,
    Info,
    Component,
}

// ── Scene traversal policy ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredSceneNodePolicy {
    Include,
    Skip,
    Flatten,
}

// ── Shared helper functions ─────────────────────────────────────────

pub fn authored_scene_node_policy(
    world: &World,
    component_id: ComponentId,
) -> AuthoredSceneNodePolicy {
    if world
        .get_component_by_id_as::<EditorComponent>(component_id)
        .is_some()
    {
        return AuthoredSceneNodePolicy::Include;
    }

    match world.component_label(component_id) {
        Some("editor_auto_raycastable") => return AuthoredSceneNodePolicy::Flatten,
        Some("selection_highlight")
        | Some("editor_runtime_ui_root")
        | Some("editor_workspace_cursor_root")
        | Some("editor_workspace_gizmo_root")
        | Some("editor_gizmo_anchor")
        | Some("editor_transform_gizmo")
        | Some("grid_live_root") => return AuthoredSceneNodePolicy::Skip,
        _ => {}
    }

    if world
        .get_component_by_id_as::<TransformGizmoComponent>(component_id)
        .is_some()
    {
        return AuthoredSceneNodePolicy::Skip;
    }

    AuthoredSceneNodePolicy::Include
}

pub fn world_panel_item_label(world: &World, component_id: ComponentId) -> String {
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

pub fn editor_chunk_label(world: &World, editor_root: ComponentId) -> String {
    if let Some(label) = world.component_label(editor_root) {
        if !label.is_empty() {
            return format!("Editor#{label}");
        }
    }

    format!("Editor {{ id={} }}", component_id_short(editor_root))
}

pub fn component_id_short(component_id: ComponentId) -> String {
    format!("{:?}", component_id)
        .trim_start_matches("ComponentId(")
        .trim_end_matches(')')
        .to_string()
}

pub fn editor_scene_roots(world: &World) -> Vec<ComponentId> {
    world
        .all_components()
        .filter(|&component_id| {
            world.parent_of(component_id).is_none()
                && world
                    .get_component_by_id_as::<EditorComponent>(component_id)
                    .is_some()
        })
        .collect()
}

// ── Scene model building ────────────────────────────────────────────

pub fn push_authored_world_panel_rows(
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

pub fn build_authored_world_panel_scene_model(
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

pub fn rebuild_world_panel_scene_model(
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

pub fn effective_editor_roots(
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
                .get_component_by_id_as::<EditorComponent>(component_id)
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

pub fn register_editor_root(
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

// ── Build render model ──────────────────────────────────────────────

pub fn build_world_panel_rows(
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

pub fn build_world_panel_model(
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

// ── Render item spec ────────────────────────────────────────────────

fn world_panel_ui_row_render_fn(
    world: &mut World,
    _emit: &mut dyn SignalEmitter,
    item: &UiItem,
) -> Result<ComponentId, String> {
    let height_gu = if matches!(item.kind, UiItemKind::Spacer) {
        Some(0.8)
    } else {
        None
    };

    let (background_rgba, text_rgba, interactive, row_kind_label) = match item.kind {
        UiItemKind::Spacer => ([0.0, 0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 0.0], false, "Spacer"),
        UiItemKind::EditorRoot => (
            [0.30, 0.84, 0.38, 0.98],
            [0.03, 0.08, 0.04, 1.0],
            true,
            "EditorRoot",
        ),
        UiItemKind::Info => ([0.85, 0.85, 0.85, 1.0], [0.0, 0.0, 0.0, 1.0], false, "Info"),
        UiItemKind::Component if item.selected => (
            [1.00, 0.88, 0.20, 0.96],
            [0.06, 0.09, 0.08, 1.0],
            true,
            "Component",
        ),
        UiItemKind::Component => (
            [0.92, 0.97, 0.92, 1.0],
            [0.06, 0.09, 0.08, 1.0],
            true,
            "Component",
        ),
    };

    Ok(spawn_panel_ui_row_tree(
        world,
        PanelUiRowSpec {
            row_name: &item.key,
            payload_name: WORLD_PANEL_PAYLOAD_NAME,
            target_component: item.target_ref,
            label: &item.label,
            row_kind_label,
            interactive,
            background_rgba,
            text_rgba,
            font_size_gu: None,
            spacer_height_gu: height_gu,
        },
    ))
}

pub static WORLD_PANEL_ROW_SPEC: LazyLock<ItemRendererSpec> =
    LazyLock::new(|| RendererSpec::Rust {
        render_fn: Box::new(world_panel_ui_row_render_fn),
    });

// ── Panel status rendering ──────────────────────────────────────────

fn world_panel_status_asset_path() -> &'static str {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/components/panel_items.mms"
    )
}

pub fn mark_nearest_layout_dirty(world: &mut World, start: ComponentId) {
    let mut current = Some(start);
    while let Some(component_id) = current {
        if let Some(layout) = world
            .get_component_by_id_as_mut::<crate::engine::ecs::component::LayoutComponent>(
                component_id,
            )
        {
            layout.mark_dirty();
            return;
        }
        current = world.parent_of(component_id);
    }
}

fn rerender_panel_status(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_root: ComponentId,
    status_wrap: ComponentId,
    label: &str,
) {
    let start = std::time::Instant::now();
    if let Some(existing_status_root) = world.find_component(panel_root, PANEL_STATUS_ROOT_SELECTOR)
    {
        emit.push_intent_now(
            existing_status_root,
            crate::engine::ecs::IntentValue::RemoveSubtree {
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
        crate::engine::ecs::IntentValue::Attach {
            parents: vec![status_wrap],
            child: spawned_status_root,
        },
    );
    mark_nearest_layout_dirty(world, status_wrap);
    let _ = start;
}

pub fn rerender_world_panel_status(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    world_panel_root: ComponentId,
    status_wrap: ComponentId,
    label: &str,
) {
    rerender_panel_status(world, emit, world_panel_root, status_wrap, label);
}

pub(crate) fn world_panel_status_label(rows_len: usize) -> String {
    format!("rows: {rows_len}")
}

// ── Content rendering ───────────────────────────────────────────────

pub fn rerender_world_panel_content(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    content_slot: ComponentId,
    selection_root: ComponentId,
    rows: &[WorldPanelRow],
    selected_index: Option<i64>,
    data_renderer: &mut DataRendererSystem,
) {
    let start = std::time::Instant::now();
    if std::env::var("CAT_DEBUG_WORLD_PANEL_REBUILD")
        .ok()
        .map(|s| {
            let s = s.trim().to_ascii_lowercase();
            s == "1" || s == "true" || s == "on" || s == "yes"
        })
        .unwrap_or(false)
    {
        println!(
            "[WorldPanel][audit] content rebuild rows={} selected_index={:?}",
            rows.len(),
            selected_index
        );
    }
    let items: Vec<UiItem> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| UiItem {
            key: format!("{ITEM_PREFIX}{i}"),
            kind: match row.kind {
                WorldPanelRowKind::Spacer => UiItemKind::Spacer,
                WorldPanelRowKind::EditorRoot => UiItemKind::EditorRoot,
                WorldPanelRowKind::Info => UiItemKind::Info,
                WorldPanelRowKind::Component => UiItemKind::Component,
            },
            label: row.display_label.clone(),
            selected: selected_index == Some(i as i64),
            target_ref: row.target_component,
        })
        .collect();

    let container =
        match data_renderer.render_list(world, emit, content_slot, &WORLD_PANEL_ROW_SPEC, &items) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[InspectorSystem] world panel content render error: {e}");
                return;
            }
        };

    if let Some(index) = selected_index.and_then(|i| usize::try_from(i).ok()) {
        let row_selector = format!("#{ITEM_PREFIX}{index}");
        if let Some(row_root) = world.find_component(container, &row_selector) {
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
    } else {
        apply_selection_set(world, emit, selection_root, Vec::new(), None);
    }
    let _ = start;
}

pub fn parse_item_index(row_name: &str) -> Option<usize> {
    row_name.strip_prefix(ITEM_PREFIX)?.parse().ok()
}

pub fn sync_world_panel_selection(
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
    let Some(world_panel_root) = world.find_component(panel_query_root, "#world_panel_root") else {
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
        apply_selection_set(world, emit, selection_root, Vec::new(), None);
        return;
    };
    let model = build_world_panel_model(
        world,
        &editor_context,
        &world_panel_scene_model
            .lock()
            .expect("world panel scene model mutex poisoned"),
    );
    let Some((selected_index, _)) = model
        .rows
        .iter()
        .enumerate()
        .find(|(_, row)| row.target_component == Some(target_component))
    else {
        apply_selection_set(world, emit, selection_root, Vec::new(), None);
        return;
    };
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

    apply_selection_set(
        world,
        emit,
        selection_root,
        vec![SelectionEntry {
            index: Some(selected_index),
            component: row_root,
        }],
        Some(row_root),
    );
}

pub fn apply_world_panel_semantic_selection(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    selection_root: ComponentId,
    world_panel_root: ComponentId,
    status_wrap: ComponentId,
    editor_context_state: &Arc<Mutex<EditorContextState>>,
) -> bool {
    let Some((selected_component, selected_payload)) = world
        .get_component_by_id_as::<SelectionComponent>(selection_root)
        .map(|selection| (selection.selected_component, selection.selected_payload))
    else {
        return false;
    };
    let Some(target_component) =
        resolve_semantic_target_from_payload(world, selected_payload, selected_component)
    else {
        return false;
    };

    let _selection_result =
        apply_semantic_target_selection(world, emit, editor_context_state, target_component, true);

    let status_text =
        if let Some(rows_mount) = world.find_component(world_panel_root, "#rows_mount") {
            let rows_len = world.children_of(rows_mount).len().saturating_sub(1);
            format!(
                "{} | selected {}",
                world_panel_status_label(rows_len),
                world_panel_item_label(world, target_component)
            )
        } else {
            format!(
                "selected {}",
                world_panel_item_label(world, target_component)
            )
        };
    rerender_world_panel_status(world, emit, world_panel_root, status_wrap, &status_text);
    true
}

/// Handle a click on a selectable world-panel list item.
/// Returns `true` if the click landed on a world-panel row and the semantic
/// selection was applied (caller may want to refresh inspector panels).
pub fn handle_world_panel_item_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    world_panel_root: ComponentId,
    renderable: ComponentId,
    editor_context_state: &Arc<Mutex<EditorContextState>>,
) -> bool {
    let Some(action) = decode_panel_action_payload(
        world,
        renderable,
        WORLD_PANEL_PAYLOAD_NAME,
        PanelKind::World,
        PanelActionKind::Select,
        None,
        None,
    ) else {
        return false;
    };
    let Some(row_name) = action.item_key.as_deref() else {
        return false;
    };
    let Some(row_index) = parse_item_index(row_name) else {
        return false;
    };
    let Some(row_root) = world.find_component(world_panel_root, &format!("#{row_name}")) else {
        return false;
    };
    let Some(selection_root) =
        world.find_component(world_panel_root, &format!("#{WORLD_PANEL_SELECTION_NAME}"))
    else {
        return false;
    };
    let payload_child = world.children_of(row_root).iter().copied().find(|&child| {
        world
            .get_component_by_id_as::<DataComponent>(child)
            .is_some_and(|data| data.get_component("target_component").is_some())
    });
    println!(
        "[WorldPanel][trace] click row_name={row_name} row_root={row_root:?} row_index={row_index} payload_child={payload_child:?} selection_root={selection_root:?}"
    );
    apply_selection_set(
        world,
        emit,
        selection_root,
        vec![SelectionEntry {
            index: Some(row_index),
            component: row_root,
        }],
        Some(row_root),
    );
    let Some(status_wrap) = world.find_component(world_panel_root, PANEL_STATUS_WRAP_SELECTOR)
    else {
        return false;
    };
    apply_world_panel_semantic_selection(
        world,
        emit,
        selection_root,
        world_panel_root,
        status_wrap,
        editor_context_state,
    )
}

// ── Shared consts ────────────────────────────────────────────────────

pub(crate) const PANEL_CONTENT_SLOT_SELECTOR: &str = "#content_slot";
const PANEL_STATUS_VALUE_SELECTOR: &str = "#panel_status_value";
pub(crate) const WORLD_PANEL_ROOT_SELECTOR: &str = "#world_panel_root";
pub(crate) const WORLD_PANEL_SELECTION_SELECTOR: &str = "#world_panel_selection";
pub(crate) const SAVE_BUTTON_SELECTOR: &str = "#save_button";
pub(crate) const LOAD_BUTTON_SELECTOR: &str = "#load_button";

#[cfg(test)]
static WORLD_PANEL_SCENE_PATH_OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);

pub(crate) fn world_panel_scene_path() -> PathBuf {
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

pub(crate) fn world_panel_root_label(world: &World, component_id: ComponentId) -> String {
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

pub(crate) fn save_world_panel_scene_to_path(world: &World, path: &Path) -> Result<usize, String> {
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
        out.push_str(&crate::scripting::unparser::unparse_component(&component));
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

fn should_skip_loaded_root(component: &MaterializedCE) -> bool {
    let Some(name) = materialized_ce_name(component) else {
        return false;
    };

    matches!(
        name,
        EDITOR_RUNTIME_UI_ROOT_NAME
            | "editor_panel_layout_mount"
            | "editor_panel_layout_root"
            | "editor_panel_layout_selection"
            | "world_panel_root"
            | "inspector_panel_root"
            | "assets_root"
            | "paint_panel_root"
            | "editor_settings_panel_root"
            | "world_panel_content_root"
            | "inspector_panel_content_root"
            | "panel_status_root"
            | "rows_mount"
    ) || name.starts_with("inspector_panel_instance_")
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

pub(crate) fn load_world_panel_scene_from_path(
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

pub(crate) fn handle_panel_button_click(
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

pub(crate) fn panel_status_text(world: &World, panel_root: ComponentId) -> Option<String> {
    world
        .find_component(panel_root, PANEL_STATUS_VALUE_SELECTOR)
        .and_then(|status_id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(status_id)
                .map(|text| text.text.clone())
        })
}

pub(crate) const WORLD_PANEL_CONTENT_ROOT_SELECTOR: &str = "#world_panel_content_root";

pub(crate) fn spawn_world_panel_content_tree(
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

pub(crate) fn spawn_world_panel_row_tree(
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

pub(crate) fn rerender_world_panel_for_context(
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
    if let Some(status_wrap) = world.find_component(world_panel_root, PANEL_STATUS_WRAP_SELECTOR) {
        let status_text = world_panel_status_label(world_model.rows.len());
        rerender_world_panel_status(world, emit, world_panel_root, status_wrap, &status_text);
    }
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
