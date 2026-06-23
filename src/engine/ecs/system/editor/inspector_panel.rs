use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use crate::engine::ecs::component::{
    DataComponent, DataValue, SelectionComponent, SelectionEntry, StyleComponent, TextComponent,
    TransformComponent,
};
use crate::engine::ecs::system::data_renderer_system::{
    DataRendererSystem, DetailRendererSpec, ItemRendererSpec, RendererSpec, UiDetailItem, UiItem,
    UiItemKind,
};
use crate::engine::ecs::system::editor::context::EditorContextState;
use crate::engine::ecs::system::editor::panel_ui::{PanelUiRowSpec, spawn_panel_ui_row_tree};
use crate::engine::ecs::system::editor::grid_panel::GRID_PANEL_ROOT_SELECTOR;
use crate::engine::ecs::system::editor::workspace::PAINT_PANEL_ROOT_SELECTOR;
use crate::engine::ecs::system::editor::world_panel::{
    AuthoredSceneNodePolicy, AuthoredWorldPanelSceneModel, authored_scene_node_policy,
    component_id_short, editor_chunk_label, mark_nearest_layout_dirty, rerender_world_panel_status,
    world_panel_item_label, PANEL_STATUS_WRAP_SELECTOR, WORLD_PANEL_ROOT_SELECTOR,
};
use crate::engine::ecs::system::panel_system::{
    decode_panel_action_payload, is_descendant_or_self, panel_layout_root_id, PanelActionKind,
    PanelControlKind, PanelKind, PanelShellSpec, PanelSlotKind, spawn_panel_instance,
    PANEL_LAYOUT_ROOT_NAME, PANEL_LAYOUT_SELECTION_NAME,
};
use crate::engine::ecs::system::selection_system::{
    apply_selection_set, resolve_semantic_target_from_payload,
};
use crate::engine::ecs::{ComponentId, EventSignal, IntentValue, SignalEmitter, SignalKind, World};
use crate::meow_meow::object::Value;

pub(crate) type InspectorPanelId = u64;

pub(crate) const INSPECTOR_PANEL_PAYLOAD_NAME: &str = "inspector_panel_payload";
pub(crate) const INSPECTOR_PANEL_INSTANCE_ID_KEY: &str = "inspector_panel_id";
pub(crate) const INSPECTOR_ITEM_PREFIX: &str = "inspector_item_";
const MAX_INSPECTOR_PANEL_ROWS: usize = 256;
pub(crate) const INSPECTOR_PANEL_ROOT_SELECTOR: &str = "#inspector_panel_root";
pub(crate) const INSPECTOR_PANEL_SIDEBAR_SLOT_SELECTOR: &str = "#sidebar_slot";
pub(crate) const INSPECTOR_PANEL_DETAIL_SLOT_SELECTOR: &str = "#detail_slot";
pub(crate) const INSPECTOR_PANEL_PIN_SLOT_SELECTOR: &str = "#pin_slot";
pub(crate) const INSPECTOR_PANEL_CONTENT_ROOT_SELECTOR: &str = "#inspector_panel_content_root";
pub(crate) const INSPECTOR_PANEL_DETAIL_ROOT_SELECTOR: &str = "#inspector_details_root";
pub(crate) const INSPECTOR_PANEL_SELECTION_SELECTOR: &str = "#inspector_panel_selection";
pub(crate) const INSPECTOR_PANEL_INSTANCE_PREFIX: &str = "inspector_panel_instance_";
pub(crate) const INSPECTOR_PANEL_INSTANCE_DATA_NAME: &str = "inspector_panel_instance_data";
pub(crate) const INSPECTOR_PANEL_PIN_BUTTON_NAME: &str = "pin_button";
pub(crate) const INSPECTOR_PANEL_PIN_BUTTON_SELECTOR: &str = "#pin_button";
pub(crate) const INSPECTOR_PANEL_SELECTION_NAME: &str = "inspector_panel_selection";
const PANEL_LAYOUT_GAP_GU: f64 = 2.0;
const DISABLE_INSPECTOR_MOUNT_WRITES: bool = false;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct InspectorSubtreeSelection {
    pub(crate) focused_row: Option<ComponentId>,
    pub(crate) expanded: Vec<ComponentId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct InspectorScrollState {
    pub(crate) row_offset: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectorPanelState {
    pub(crate) panel_id: InspectorPanelId,
    pub(crate) editor_root: ComponentId,
    pub(crate) inspected: Option<ComponentId>,
    pub(crate) pinned: bool,
    pub(crate) subtree_selection: InspectorSubtreeSelection,
    pub(crate) scroll_offset: InspectorScrollState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct InspectorWorkspaceState {
    pub(crate) panels: Vec<InspectorPanelState>,
    pub(crate) active_panel: Option<InspectorPanelId>,
    pub(crate) pending_spawn_target: Option<ComponentId>,
    pub(crate) next_panel_id: InspectorPanelId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectorPanelModel {
    pub(crate) panel_id: InspectorPanelId,
    pub(crate) title: String,
    pub(crate) rows: Vec<InspectorPanelRow>,
    pub(crate) detail: InspectorPanelDetailModel,
    pub(crate) pinned: bool,
    pub(crate) active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectorPanelRow {
    pub(crate) target_component: Option<ComponentId>,
    pub(crate) display_label: String,
    pub(crate) kind: InspectorPanelRowKind,
    pub(crate) selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InspectorPanelRowKind {
    Info,
    Component,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InspectorPanelDetailModel {
    pub(crate) name: String,
    pub(crate) id: String,
    pub(crate) guid: String,
}

impl InspectorWorkspaceState {
    pub(crate) fn next_panel_id(&mut self) -> InspectorPanelId {
        let next = self.next_panel_id.max(1);
        self.next_panel_id = next + 1;
        next
    }

    pub(crate) fn active_panel_index(&self) -> Option<usize> {
        let active_panel = self.active_panel?;
        self.panels
            .iter()
            .position(|panel| panel.panel_id == active_panel)
    }

    pub(crate) fn ensure_default_panel(
        &mut self,
        editor_root: ComponentId,
        inspected: Option<ComponentId>,
    ) -> InspectorPanelId {
        if let Some(panel) = self.panels.first() {
            return panel.panel_id;
        }

        let panel_id = self.next_panel_id();
        self.panels.push(InspectorPanelState {
            panel_id,
            editor_root,
            inspected,
            pinned: false,
            subtree_selection: InspectorSubtreeSelection::default(),
            scroll_offset: InspectorScrollState::default(),
        });
        self.active_panel = Some(panel_id);
        panel_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InspectorWorkspaceEvent {
    SelectionChanged {
        editor_root: ComponentId,
        selected_target: Option<ComponentId>,
    },
    SidebarRowFocused {
        panel_id: InspectorPanelId,
        component: ComponentId,
    },
    PanelFocused {
        panel_id: InspectorPanelId,
    },
    PanelPinToggled {
        panel_id: InspectorPanelId,
    },
}

pub(crate) fn clear_missing_inspector_targets(
    workspace: &mut InspectorWorkspaceState,
    component_exists: impl Fn(ComponentId) -> bool,
) {
    for panel in &mut workspace.panels {
        if panel
            .inspected
            .is_some_and(|component_id| !component_exists(component_id))
        {
            panel.inspected = None;
        }
    }
}

pub(crate) fn reduce_inspector_workspace_state(
    old: &InspectorWorkspaceState,
    event: &InspectorWorkspaceEvent,
) -> InspectorWorkspaceState {
    let mut new = old.clone();

    match event {
        InspectorWorkspaceEvent::SelectionChanged {
            editor_root,
            selected_target,
        } => {
            if new.panels.is_empty() {
                new.ensure_default_panel(*editor_root, *selected_target);
                return new;
            }

            let active_index = new.active_panel_index().unwrap_or(0);
            let active_panel = &new.panels[active_index];
            let should_spawn = active_panel.pinned
                && selected_target.is_some()
                && active_panel.inspected != *selected_target;

            if should_spawn {
                let panel_id = new.next_panel_id();
                new.panels.insert(
                    active_index + 1,
                    InspectorPanelState {
                        panel_id,
                        editor_root: *editor_root,
                        inspected: *selected_target,
                        pinned: false,
                        subtree_selection: InspectorSubtreeSelection::default(),
                        scroll_offset: InspectorScrollState::default(),
                    },
                );
                new.active_panel = Some(panel_id);
                new.pending_spawn_target = None;
                return new;
            }

            let active_panel = &mut new.panels[active_index];
            active_panel.editor_root = *editor_root;
            active_panel.inspected = *selected_target;
            active_panel.subtree_selection.focused_row = *selected_target;
            new.active_panel = Some(active_panel.panel_id);
            new.pending_spawn_target = None;
        }
        InspectorWorkspaceEvent::SidebarRowFocused {
            panel_id,
            component,
        } => {
            if let Some(panel) = new
                .panels
                .iter_mut()
                .find(|panel| panel.panel_id == *panel_id)
            {
                if panel.subtree_selection.focused_row == Some(*component) {
                    return new;
                }
                panel.subtree_selection.focused_row = Some(*component);
                new.active_panel = Some(*panel_id);
            }
        }
        InspectorWorkspaceEvent::PanelFocused { panel_id } => {
            new.active_panel = Some(*panel_id);
        }
        InspectorWorkspaceEvent::PanelPinToggled { panel_id } => {
            new.active_panel = Some(*panel_id);
            if let Some(panel) = new
                .panels
                .iter_mut()
                .find(|panel| panel.panel_id == *panel_id)
            {
                panel.pinned = !panel.pinned;
            }
        }
    }

    new
}

// ── Render model building ──

pub(crate) fn build_inspector_panel_models(
    world: &World,
    scene_model: &AuthoredWorldPanelSceneModel,
    workspace: &InspectorWorkspaceState,
) -> Vec<InspectorPanelModel> {
    if workspace.panels.is_empty() {
        return vec![InspectorPanelModel {
            panel_id: 0,
            title: "Inspector".to_string(),
            rows: vec![InspectorPanelRow {
                target_component: None,
                display_label: "<nothing selected>".to_string(),
                kind: InspectorPanelRowKind::Info,
                selected: false,
            }],
            detail: InspectorPanelDetailModel {
                name: String::new(),
                id: String::new(),
                guid: String::new(),
            },
            pinned: false,
            active: true,
        }];
    }

    workspace
        .panels
        .iter()
        .map(|panel| {
            let rows = panel
                .inspected
                .filter(|&component_id| world.get_component_record(component_id).is_some())
                .map(|component_id| {
                    build_inspector_panel_rows(world, scene_model, panel, component_id)
                })
                .unwrap_or_else(|| {
                    vec![InspectorPanelRow {
                        target_component: None,
                        display_label: "<nothing selected>".to_string(),
                        kind: InspectorPanelRowKind::Info,
                        selected: false,
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
                detail: build_inspector_panel_detail_model(world, panel),
                rows,
                pinned: panel.pinned,
                active: workspace.active_panel == Some(panel.panel_id),
            }
        })
        .collect()
}

pub(crate) fn build_inspector_panel_rows(
    world: &World,
    scene_model: &AuthoredWorldPanelSceneModel,
    panel: &InspectorPanelState,
    root: ComponentId,
) -> Vec<InspectorPanelRow> {
    if let Some(rows) = build_authored_inspector_panel_rows(world, scene_model, panel, root) {
        return rows;
    }

    if matches!(
        authored_scene_node_policy(world, root),
        AuthoredSceneNodePolicy::Skip
    ) {
        return vec![InspectorPanelRow {
            target_component: None,
            display_label: "<selection hidden>".to_string(),
            kind: InspectorPanelRowKind::Info,
            selected: false,
        }];
    }

    let mut rows = Vec::new();
    push_inspector_panel_rows(world, panel, root, 0, &mut rows);
    rows
}

fn build_authored_inspector_panel_rows(
    world: &World,
    scene_model: &AuthoredWorldPanelSceneModel,
    panel: &InspectorPanelState,
    root: ComponentId,
) -> Option<Vec<InspectorPanelRow>> {
    for section in &scene_model.sections {
        if section.editor_root == root {
            let mut rows = Vec::with_capacity(section.rows.len() + 1);
            rows.push(InspectorPanelRow {
                target_component: Some(section.editor_root),
                display_label: editor_chunk_label(world, section.editor_root),
                kind: InspectorPanelRowKind::Component,
                selected: inspector_row_selected(panel, section.editor_root),
            });
            rows.extend(section.rows.iter().map(|row| InspectorPanelRow {
                target_component: Some(row.target_component),
                display_label: format!("{}{}", "  ".repeat(row.depth + 1), row.label),
                kind: InspectorPanelRowKind::Component,
                selected: inspector_row_selected(panel, row.target_component),
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
            target_component: Some(root),
            display_label: root_row.label.clone(),
            kind: InspectorPanelRowKind::Component,
            selected: inspector_row_selected(panel, root),
        });

        for row in section.rows.iter().skip(root_index + 1) {
            if row.depth <= root_row.depth {
                break;
            }

            rows.push(InspectorPanelRow {
                target_component: Some(row.target_component),
                display_label: format!("{}{}", "  ".repeat(row.depth - root_row.depth), row.label),
                kind: InspectorPanelRowKind::Component,
                selected: inspector_row_selected(panel, row.target_component),
            });
        }

        return Some(rows);
    }

    None
}

fn push_inspector_panel_rows(
    world: &World,
    panel: &InspectorPanelState,
    component_id: ComponentId,
    depth: usize,
    out: &mut Vec<InspectorPanelRow>,
) {
    match authored_scene_node_policy(world, component_id) {
        AuthoredSceneNodePolicy::Skip => return,
        AuthoredSceneNodePolicy::Flatten => {
            for &child in world.children_of(component_id) {
                push_inspector_panel_rows(world, panel, child, depth, out);
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
        target_component: Some(component_id),
        display_label: format!(
            "{}{}",
            "  ".repeat(depth),
            world_panel_item_label(world, component_id)
        ),
        kind: InspectorPanelRowKind::Component,
        selected: inspector_row_selected(panel, component_id),
    });

    for &child in world.children_of(component_id) {
        if out.len() >= MAX_INSPECTOR_PANEL_ROWS {
            out.push(InspectorPanelRow {
                target_component: None,
                display_label: "… inspector truncated …".to_string(),
                kind: InspectorPanelRowKind::Info,
                selected: false,
            });
            return;
        }
        push_inspector_panel_rows(world, panel, child, depth + 1, out);
    }
}

fn inspector_row_selected(panel: &InspectorPanelState, component_id: ComponentId) -> bool {
    panel.subtree_selection.focused_row.or(panel.inspected) == Some(component_id)
}

fn build_inspector_panel_detail_model(
    world: &World,
    panel: &InspectorPanelState,
) -> InspectorPanelDetailModel {
    let selected_component = panel
        .subtree_selection
        .focused_row
        .filter(|&component_id| world.get_component_record(component_id).is_some())
        .or(panel
            .inspected
            .filter(|&component_id| world.get_component_record(component_id).is_some()));

    let Some(component_id) = selected_component else {
        return InspectorPanelDetailModel {
            name: "<nothing selected>".to_string(),
            id: String::new(),
            guid: String::new(),
        };
    };

    let guid = world
        .get_component_record(component_id)
        .map(|record| record.guid.to_string())
        .unwrap_or_default();

    InspectorPanelDetailModel {
        name: world_panel_item_label(world, component_id),
        id: component_id_short(component_id),
        guid,
    }
}

pub(crate) fn parse_inspector_item_index(row_name: &str) -> Option<usize> {
    row_name.strip_prefix(INSPECTOR_ITEM_PREFIX)?.parse().ok()
}

pub(crate) fn inspector_panel_instance_id_on_root(
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

fn inspector_details_asset_path() -> &'static str {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/components/inspector_details.mms"
    )
}

fn inspector_panel_ui_row_render_fn(
    world: &mut World,
    _emit: &mut dyn SignalEmitter,
    item: &UiItem,
) -> Result<ComponentId, String> {
    let row = InspectorPanelRow {
        kind: match item.kind {
            UiItemKind::Component => InspectorPanelRowKind::Component,
            UiItemKind::Info | UiItemKind::EditorRoot | UiItemKind::Spacer => {
                InspectorPanelRowKind::Info
            }
        },
        display_label: item.label.clone(),
        selected: item.selected,
        target_component: item.target_ref,
    };
    Ok(spawn_inspector_panel_row_tree(world, &item.key, &row))
}

pub(crate) static INSPECTOR_ROW_SPEC: LazyLock<ItemRendererSpec> =
    LazyLock::new(|| RendererSpec::Rust {
        render_fn: Box::new(inspector_panel_ui_row_render_fn),
    });

pub(crate) static INSPECTOR_DETAIL_SPEC: LazyLock<DetailRendererSpec> =
    LazyLock::new(|| RendererSpec::Mms {
        asset_path: inspector_details_asset_path(),
        export_name: "inspector_details",
        to_args: |detail: &UiDetailItem| {
            vec![
                Value::String(detail.name.clone()),
                Value::String(detail.id.clone()),
                Value::String(detail.guid.clone()),
            ]
        },
    });

pub(crate) fn rerender_inspector_panels(
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
        .collect::<HashMap<_, _>>();

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
    let _ = start;
}

pub(crate) fn rerender_single_inspector_panel_sidebar(
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
    let _ = start;
}

pub(crate) fn rerender_single_inspector_panel_detail(
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
    let _ = start;
}

pub(crate) fn update_inspector_panel_instance_tree(
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

pub(crate) fn clicked_named_ancestor(world: &World, node: ComponentId, prefix: &str) -> Option<String> {
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

pub(crate) fn clicked_inspector_panel_instance_id(
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

pub(crate) fn find_inspector_panel_instance_root(
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

pub(crate) fn sync_inspector_workspace_to_selection(
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

pub(crate) fn handle_inspector_panel_workspace_click(
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

pub(crate) fn sync_and_refresh_inspector_panels(
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
        let _ = sync_start;
    }

    let Some(bottom_row_root) = panel_layout_root_id(world, panel_query_root) else {
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
    let _ = build_models_start;
    let rerender_start = std::time::Instant::now();
    rerender_inspector_panels(
        world,
        emit,
        bottom_row_root,
        &inspector_models,
        rendered_inspector_models,
        data_renderer,
    );
    let _ = rerender_start;
    let _ = total_start;
}

pub(crate) fn refresh_inspector_panels_from_workspace(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    world_panel_scene_model: &Arc<Mutex<AuthoredWorldPanelSceneModel>>,
    inspector_workspace_state: &Arc<Mutex<InspectorWorkspaceState>>,
    rendered_inspector_models: &Arc<Mutex<Vec<InspectorPanelModel>>>,
    data_renderer: &mut DataRendererSystem,
) {
    let Some(bottom_row_root) = panel_layout_root_id(world, panel_query_root) else {
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

pub(crate) fn world_panel_selection_matches_editor_context(
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

pub(crate) fn trace_suspicious_inspector_target(world: &World, target: Option<ComponentId>) {
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

pub(crate) fn spawn_inspector_panel_instance_tree(
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
        asset_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/panels.mms").to_string(),
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

pub(crate) fn attach_inspector_panel_instance_id(
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

pub(crate) fn spawn_inspector_panel_instance_fallback_root(
    world: &mut World,
    panel_id: InspectorPanelId,
) -> ComponentId {
    let root = world
        .add_component_boxed_named("inspector_panel_root", Box::new(TransformComponent::new()));
    attach_inspector_panel_instance_id(world, root, panel_id);
    root
}

pub(crate) fn set_inspector_pin_button_state(
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

pub(crate) fn debug_style_details(world: &World, root: ComponentId, selector: &str, label: &str) {
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

pub(crate) fn debug_panel_root(world: &World, root: ComponentId, kind: &str) {
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

pub(crate) fn nearest_editor_ancestor(world: &World, start: ComponentId) -> Option<ComponentId> {
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

fn spawn_inspector_panel_row_tree(
    world: &mut World,
    row_name: &str,
    row: &InspectorPanelRow,
) -> ComponentId {
    let (background_rgba, text_rgba, interactive, row_kind_label) = match row.kind {
        InspectorPanelRowKind::Info => {
            ([0.85, 0.85, 0.85, 1.0], [0.0, 0.0, 0.0, 1.0], false, "Info")
        }
        InspectorPanelRowKind::Component if row.selected => (
            [1.00, 0.88, 0.20, 0.96],
            [0.08, 0.08, 0.02, 1.0],
            true,
            "Component",
        ),
        InspectorPanelRowKind::Component => (
            [0.92, 0.97, 0.92, 1.0],
            [0.06, 0.09, 0.08, 1.0],
            true,
            "Component",
        ),
    };
    spawn_panel_ui_row_tree(
        world,
        PanelUiRowSpec {
            row_name,
            payload_name: INSPECTOR_PANEL_PAYLOAD_NAME,
            target_component: row.target_component,
            label: &row.display_label,
            row_kind_label,
            interactive,
            background_rgba,
            text_rgba,
            font_size_gu: Some(1.0),
            spacer_height_gu: None,
        },
    )
}

pub(crate) fn focus_panel_from_descendant_click(
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
