use std::sync::LazyLock;

use crate::engine::ecs::component::{DataComponent, DataValue};
use crate::engine::ecs::system::data_renderer_system::{
    DetailRendererSpec, ItemRendererSpec, RendererSpec, UiDetailItem, UiItem, UiItemKind,
};
use crate::engine::ecs::system::editor::panel_ui::{PanelUiRowSpec, spawn_panel_ui_row_tree};
use crate::engine::ecs::system::editor::world_panel::{
    AuthoredSceneNodePolicy, AuthoredWorldPanelSceneModel, authored_scene_node_policy,
    component_id_short, editor_chunk_label, world_panel_item_label,
};
use crate::engine::ecs::{ComponentId, SignalEmitter, World};
use crate::meow_meow::object::Value;

pub(crate) type InspectorPanelId = u64;

pub(crate) const INSPECTOR_PANEL_PAYLOAD_NAME: &str = "inspector_panel_payload";
pub(crate) const INSPECTOR_PANEL_INSTANCE_ID_KEY: &str = "inspector_panel_id";
pub(crate) const INSPECTOR_ITEM_PREFIX: &str = "inspector_item_";
const MAX_INSPECTOR_PANEL_ROWS: usize = 256;

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
