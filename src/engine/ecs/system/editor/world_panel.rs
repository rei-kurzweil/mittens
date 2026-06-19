use std::sync::{Arc, Mutex};

use crate::engine::ecs::component::{
    EditorComponent, SelectionComponent, SelectionEntry, TransformGizmoComponent,
};
use crate::engine::ecs::system::data_renderer_system::{
    DataRendererSystem, ItemRendererSpec, RendererSpec, UiItem, UiItemKind,
};
use crate::engine::ecs::system::editor::context::EditorContextState;
use crate::engine::ecs::system::editor::panel_ui::{PanelUiRowSpec, spawn_panel_ui_row_tree};
use crate::engine::ecs::system::selection_system::{
    apply_selection_set, resolve_semantic_target_from_payload,
};
use crate::engine::ecs::{ComponentId, SignalEmitter, World};
use crate::engine::memory_trace;
use crate::meow_meow::object::Value;
use crate::meow_meow::runner::MeowMeowRunner;
use std::sync::LazyLock;

pub const ITEM_PREFIX: &str = "item_";
pub const WORLD_PANEL_PAYLOAD_NAME: &str = "world_panel_payload";
pub const WORLD_PANEL_SELECTION_NAME: &str = "world_panel_selection";
const PANEL_STATUS_ROOT_SELECTOR: &str = "#panel_status_root";
const PANEL_STATUS_WRAP_SELECTOR: &str = "#save_status_wrap";

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
        | Some("editor_gizmo_anchor")
        | Some("grid_visual")
        | Some("editor_transform_gizmo") => return AuthoredSceneNodePolicy::Skip,
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
    memory_trace::log_line("\n🟧✏️ [editor-memory] editor rebuild_world_panel_scene_model:start");
    memory_trace::sample("editor rebuild_world_panel_scene_model:start", None);
    let editor_roots = effective_editor_roots(world, installed_editor_roots);
    let rebuilt = build_authored_world_panel_scene_model(world, &editor_roots);
    *scene_model
        .lock()
        .expect("world panel scene model mutex poisoned") = rebuilt;
    memory_trace::log_line("\n🟧✏️ [editor-memory] editor rebuild_world_panel_scene_model:end");
    memory_trace::sample("editor rebuild_world_panel_scene_model:end", None);
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
    println!(
        "[WorldPanel] rerender_panel_status took {:?}",
        start.elapsed()
    );
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
        memory_trace::log_line(format!(
            "[WorldPanel][audit] content rebuild rows={} selected_index={:?}",
            rows.len(),
            selected_index
        ));
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
    memory_trace::sample("after world panel content rebuild", None);
    println!(
        "[WorldPanel] rerender_world_panel_content took {:?}",
        start.elapsed()
    );
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
