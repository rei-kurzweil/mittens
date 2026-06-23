use std::sync::LazyLock;

use crate::engine::ecs::component::{
    ColorComponent, DataComponent, DataValue, Display, EdgeInsets, EditorComponent,
    OptionComponent, RaycastableComponent, SizeDimension, StyleComponent, TextAlign, TextComponent,
    TransformComponent, style::VerticalAlign,
};
use crate::engine::ecs::system::GridSystem;
use crate::engine::ecs::system::data_renderer_system::{
    DataRendererSystem, ItemRendererSpec, RendererSpec, UiItem, UiItemKind,
};
use crate::engine::ecs::system::editor::context::EditorContextState;
use crate::engine::ecs::system::editor::world_panel::{
    world_panel_item_label, PANEL_CONTENT_SLOT_SELECTOR,
};
use crate::engine::ecs::{ComponentId, SignalEmitter, World};

pub(crate) const GRID_PANEL_ROOT_SELECTOR: &str = "#grid_panel_root";
pub(crate) const GRID_PANEL_ADD_BUTTON_SELECTOR: &str = "#grid_add_button";
pub(crate) const GRID_PANEL_ITEM_PREFIX: &str = "grid_item_";
pub(crate) const GRID_PANEL_ROW_PAYLOAD_NAME: &str = "grid_panel_row_payload";
pub(crate) const GRID_PANEL_VISIBILITY_PAYLOAD_NAME: &str = "grid_panel_visibility_payload";
pub(crate) const GRID_PANEL_ENABLED_PAYLOAD_NAME: &str = "grid_panel_enabled_payload";
pub(crate) const GRID_PANEL_DELETE_PAYLOAD_NAME: &str = "grid_panel_delete_payload";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GridPanelState {
    pub(crate) active_editor: Option<ComponentId>,
    pub(crate) selected_grid_transform: Option<ComponentId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GridPanelEvent {
    SelectionChanged {
        editor_root: Option<ComponentId>,
        selected_component: Option<ComponentId>,
    },
    GridDeleted {
        owner_transform: ComponentId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GridPanelEntry {
    pub(crate) grid_component: ComponentId,
    pub(crate) owner_transform: ComponentId,
    pub(crate) label: String,
    pub(crate) shown: bool,
    pub(crate) enabled: bool,
    pub(crate) selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GridPanelModel {
    pub(crate) title: String,
    pub(crate) rows: Vec<GridPanelEntry>,
    pub(crate) active_editor: Option<ComponentId>,
}

pub(crate) fn reduce_grid_panel_state(
    old: &GridPanelState,
    event: &GridPanelEvent,
) -> GridPanelState {
    let mut new = old.clone();
    match event {
        GridPanelEvent::SelectionChanged {
            editor_root,
            selected_component,
        } => {
            new.active_editor = *editor_root;
            new.selected_grid_transform = *selected_component;
        }
        GridPanelEvent::GridDeleted { owner_transform } => {
            if new.selected_grid_transform == Some(*owner_transform) {
                new.selected_grid_transform = None;
            }
        }
    }
    new
}

pub(crate) fn build_grid_panel_model(
    world: &World,
    grids: &GridSystem,
    editor_root: ComponentId,
) -> GridPanelModel {
    let selected_component = world
        .get_component_by_id_as::<crate::engine::ecs::component::EditorComponent>(editor_root)
        .and_then(|editor| editor.selected);

    let rows = grids
        .enumerate_grids_for_editor(world, editor_root)
        .into_iter()
        .map(|entry| GridPanelEntry {
            grid_component: entry.grid_component,
            owner_transform: entry.owner_transform,
            label: world_panel_item_label(world, entry.owner_transform),
            shown: !entry.hidden,
            enabled: entry.enabled,
            selected: selected_component == Some(entry.owner_transform)
                || selected_component == Some(entry.grid_component),
        })
        .collect();

    GridPanelModel {
        title: "Grids".to_string(),
        rows,
        active_editor: Some(editor_root),
    }
}

pub(crate) fn grid_panel_items(model: &GridPanelModel) -> Vec<UiItem> {
    model
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| UiItem {
            key: format!("{GRID_PANEL_ITEM_PREFIX}{index}"),
            kind: UiItemKind::Component,
            label: row.label.clone(),
            selected: row.selected,
            target_ref: Some(row.owner_transform),
        })
        .collect()
}

fn grid_panel_row_render_fn(
    world: &mut World,
    _emit: &mut dyn SignalEmitter,
    item: &UiItem,
) -> Result<ComponentId, String> {
    let owner_transform = item
        .target_ref
        .ok_or_else(|| "grid row missing owner transform".to_string())?;
    let grids = GridSystem::new();
    let (shown, enabled) = grids
        .grid_owned_by_transform(world, owner_transform)
        .map(|entry| (!entry.hidden, entry.enabled))
        .unwrap_or((true, true));
    Ok(spawn_grid_panel_row_tree(
        world,
        &item.key,
        owner_transform,
        &item.label,
        item.selected,
        shown,
        enabled,
    ))
}

pub(crate) static GRID_PANEL_ROW_SPEC: LazyLock<ItemRendererSpec> =
    LazyLock::new(|| RendererSpec::Rust {
        render_fn: Box::new(grid_panel_row_render_fn),
    });

fn spawn_grid_panel_row_tree(
    world: &mut World,
    row_name: &str,
    owner_transform: ComponentId,
    label: &str,
    selected: bool,
    shown: bool,
    enabled: bool,
) -> ComponentId {
    let row_root = world.add_component_boxed_named(row_name, Box::new(TransformComponent::new()));

    let style = world.add_component_boxed_named(
        format!("{row_name}_style"),
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::Block);
            style.width = SizeDimension::Percent(100.0);
            style.margin = EdgeInsets::axes(0.25, 0.20);
            style.padding = EdgeInsets::axes(0.30, 0.35);
            style.background_color = Some(if selected {
                [1.00, 0.88, 0.20, 0.96]
            } else {
                [0.92, 0.97, 0.92, 1.0]
            });
            style.background_z = Some(0.001);
            style.overflow = crate::engine::ecs::component::Overflow::Visible;
            style
        }),
    );
    let _ = world.add_child(row_root, style);

    let body = world.add_component_boxed_named(
        format!("{row_name}_body"),
        Box::new(TransformComponent::new()),
    );
    let body_option = world.add_component_boxed_named(
        format!("{row_name}_body_option"),
        Box::new(OptionComponent::new()),
    );
    let body_raycastable = world.add_component_boxed_named(
        format!("{row_name}_body_raycastable"),
        Box::new(RaycastableComponent::click_only()),
    );
    let body_payload = world.add_component_boxed_named(
        GRID_PANEL_ROW_PAYLOAD_NAME,
        Box::new(
            DataComponent::new()
                .with_entry("row_name", DataValue::Text(row_name.to_string()))
                .with_entry("label", DataValue::Text(label.to_string()))
                .with_entry("target_component", DataValue::Component(owner_transform)),
        ),
    );
    let body_style = world.add_component_boxed_named(
        format!("{row_name}_body_style"),
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::InlineBlock);
            style.width = SizeDimension::GlyphUnits(20.5);
            style.height = SizeDimension::GlyphUnits(2.3);
            style.padding = EdgeInsets::axes(0.15, 0.10);
            style.vertical_align = VerticalAlign::Middle;
            style.color = Some(if selected {
                [0.08, 0.08, 0.02, 1.0]
            } else {
                [0.06, 0.09, 0.08, 1.0]
            });
            style
        }),
    );
    let body_text_root = world.add_component_boxed_named(
        format!("{row_name}_body_text_root"),
        Box::new(TransformComponent::new().with_position(0.0, 0.0, 0.005)),
    );
    let body_text = world.add_component_boxed_named(
        format!("{row_name}_body_text"),
        Box::new(TextComponent::new(label.to_string())),
    );
    let body_text_color = world.add_component_boxed_named(
        format!("{row_name}_body_text_color"),
        Box::new(ColorComponent::rgba(
            if selected { 0.08 } else { 0.06 },
            if selected { 0.08 } else { 0.09 },
            if selected { 0.02 } else { 0.08 },
            1.0,
        )),
    );
    let _ = world.add_child(row_root, body);
    let _ = world.add_child(body, body_option);
    let _ = world.add_child(body, body_raycastable);
    let _ = world.add_child(body, body_payload);
    let _ = world.add_child(body, body_style);
    let _ = world.add_child(body, body_text_root);
    let _ = world.add_child(body_text_root, body_text);
    let _ = world.add_child(body_text, body_text_color);

    let visibility = spawn_grid_icon_button(
        world,
        row_name,
        "visibility",
        GRID_PANEL_VISIBILITY_PAYLOAD_NAME,
        owner_transform,
        row_name,
        if shown { "Hide" } else { "Show" },
        if shown {
            [0.10, 0.55, 0.18, 1.0]
        } else {
            [0.42, 0.42, 0.42, 1.0]
        },
    );
    let enabled_toggle = spawn_grid_icon_button(
        world,
        row_name,
        "enabled",
        GRID_PANEL_ENABLED_PAYLOAD_NAME,
        owner_transform,
        row_name,
        if enabled { "Off" } else { "On" },
        if enabled {
            [0.12, 0.36, 0.72, 1.0]
        } else {
            [0.45, 0.30, 0.08, 1.0]
        },
    );
    let delete = spawn_grid_icon_button(
        world,
        row_name,
        "delete",
        GRID_PANEL_DELETE_PAYLOAD_NAME,
        owner_transform,
        row_name,
        "Delete",
        [0.72, 0.15, 0.15, 1.0],
    );
    let _ = world.add_child(row_root, visibility);
    let _ = world.add_child(row_root, enabled_toggle);
    let _ = world.add_child(row_root, delete);

    row_root
}

fn spawn_grid_icon_button(
    world: &mut World,
    row_name: &str,
    suffix: &str,
    payload_name: &str,
    owner_transform: ComponentId,
    item_key: &str,
    text: &str,
    background_color: [f32; 4],
) -> ComponentId {
    let root = world.add_component_boxed_named(
        format!("{row_name}_{suffix}_button"),
        Box::new(TransformComponent::new()),
    );
    let option = world.add_component_boxed_named(
        format!("{row_name}_{suffix}_option"),
        Box::new(OptionComponent::new()),
    );
    let raycastable = world.add_component_boxed_named(
        format!("{row_name}_{suffix}_raycastable"),
        Box::new(RaycastableComponent::click_only()),
    );
    let payload = world.add_component_boxed_named(
        payload_name,
        Box::new(
            DataComponent::new()
                .with_entry("row_name", DataValue::Text(item_key.to_string()))
                .with_entry("target_component", DataValue::Component(owner_transform))
                .with_entry("label", DataValue::Text(text.to_string())),
        ),
    );
    let style = world.add_component_boxed_named(
        format!("{row_name}_{suffix}_style"),
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::InlineBlock);
            style.width = SizeDimension::GlyphUnits(3.3);
            style.height = SizeDimension::GlyphUnits(2.3);
            style.margin = EdgeInsets {
                left: SizeDimension::GlyphUnits(0.20),
                right: SizeDimension::GlyphUnits(0.0),
                top: SizeDimension::GlyphUnits(0.0),
                bottom: SizeDimension::GlyphUnits(0.0),
            };
            style.text_align = TextAlign::Center;
            style.vertical_align = VerticalAlign::Middle;
            style.background_color = Some(background_color);
            style.color = Some([0.96, 0.98, 0.96, 1.0]);
            style
        }),
    );
    let text_root = world.add_component_boxed_named(
        format!("{row_name}_{suffix}_text_root"),
        Box::new(TransformComponent::new().with_position(0.0, 0.0, 0.005)),
    );
    let text_component = world.add_component_boxed_named(
        format!("{row_name}_{suffix}_text"),
        Box::new(TextComponent::new(text.to_string()).with_font_size(0.08)),
    );

    let _ = world.add_child(root, option);
    let _ = world.add_child(root, raycastable);
    let _ = world.add_child(root, payload);
    let _ = world.add_child(root, style);
    let _ = world.add_child(root, text_root);
    let _ = world.add_child(text_root, text_component);
    root
}

pub(crate) const EDITOR_WORKSPACE_GRIDS_CHANGED: &str = "EditorWorkspaceGridsChanged";

pub(crate) fn resolve_grid_panel_editor_root(
    world: &World,
    editor_context: &EditorContextState,
) -> Option<ComponentId> {
    if editor_context.active_editor.is_some() {
        return editor_context.active_editor;
    }

    let grids = GridSystem::new();
    for component_id in world.all_components() {
        if world
            .get_component_by_id_as::<EditorComponent>(component_id)
            .is_some()
            && !grids
                .enumerate_grids_for_editor(world, component_id)
                .is_empty()
        {
            return Some(component_id);
        }
    }

    world.all_components().find(|&component_id| {
        world
            .get_component_by_id_as::<EditorComponent>(component_id)
            .is_some()
    })
}

pub(crate) fn rerender_grid_panel_from_context(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    editor_context: &EditorContextState,
    data_renderer: &mut DataRendererSystem,
) {
    let Some(grid_panel_root) = world.find_component(panel_query_root, GRID_PANEL_ROOT_SELECTOR)
    else {
        return;
    };
    let Some(content_slot) = world.find_component(grid_panel_root, PANEL_CONTENT_SLOT_SELECTOR)
    else {
        return;
    };
    let Some(editor_root) = resolve_grid_panel_editor_root(world, editor_context) else {
        data_renderer.clear_slot(world, emit, content_slot);
        return;
    };

    let grids = GridSystem::new();
    let model = build_grid_panel_model(world, &grids, editor_root);
    let items = grid_panel_items(&model);
    if let Err(error) =
        data_renderer.render_list(world, emit, content_slot, &GRID_PANEL_ROW_SPEC, &items)
    {
        eprintln!("[InspectorSystem] grid panel content render error: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{EditorComponent, GridComponent, TransformComponent};

    #[test]
    fn reduce_grid_panel_state_tracks_selection_and_delete() {
        let mut world = World::default();
        let a = world.add_component(TransformComponent::new());
        let b = world.add_component(TransformComponent::new());
        let state = reduce_grid_panel_state(
            &GridPanelState::default(),
            &GridPanelEvent::SelectionChanged {
                editor_root: Some(a),
                selected_component: Some(b),
            },
        );
        assert_eq!(state.active_editor, Some(a));
        assert_eq!(state.selected_grid_transform, Some(b));

        let cleared =
            reduce_grid_panel_state(&state, &GridPanelEvent::GridDeleted { owner_transform: b });
        assert_eq!(cleared.selected_grid_transform, None);
    }

    #[test]
    fn build_grid_panel_model_marks_selected_transform_grid() {
        let mut world = World::default();
        let grids = GridSystem::new();
        let editor = world.add_component(EditorComponent::new());
        let transform =
            world.add_component_boxed_named("grid_1", Box::new(TransformComponent::new()));
        let grid = world.add_component(GridComponent::new(0.5));
        let _ = world.add_child(editor, transform);
        let _ = world.add_child(transform, grid);
        world
            .get_component_by_id_as_mut::<EditorComponent>(editor)
            .expect("editor")
            .selected = Some(transform);

        let model = build_grid_panel_model(&world, &grids, editor);
        assert_eq!(model.title, "Grids");
        assert_eq!(model.rows.len(), 1);
        assert_eq!(model.rows[0].label, "grid_1");
        assert!(model.rows[0].shown);
        assert!(model.rows[0].enabled);
        assert!(model.rows[0].selected);
    }
}
