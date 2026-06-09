use std::sync::LazyLock;

use crate::engine::ecs::component::{SelectionComponent, SelectionEntry};
use crate::engine::ecs::system::data_renderer_system::{
    DataRendererSystem, ItemRendererSpec, RendererSpec, UiItem, UiItemKind,
};
use crate::engine::ecs::system::editor::context::EditorContextState;
use crate::engine::ecs::system::editor::panel_ui::{spawn_panel_ui_row_tree, PanelUiRowSpec};
use crate::engine::ecs::{ComponentId, SignalEmitter, World};

const FREE_DRAW_LABEL: &str = "Free Draw";
const LINE_LABEL: &str = "Line";
const SPRAY_CAN_LABEL: &str = "Spray Can";
const FILL_LABEL: &str = "Fill";
const ERASE_LABEL: &str = "Erase";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaintState {
    pub selected_asset: Option<PaintSelection>,
    pub selected_tool: PaintTool,
    pub stroke: PaintStrokeMode,
}

impl Default for PaintState {
    fn default() -> Self {
        Self {
            selected_asset: None,
            selected_tool: PaintTool::Unknown(None),
            stroke: PaintStrokeMode::Idle,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaintSelection {
    pub item: Option<String>,
    pub component: Option<ComponentId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaintTool {
    FreeDraw,
    Line,
    SprayCan,
    Fill,
    Erase,
    Unknown(Option<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintStrokeMode {
    Idle,
    Dragging,
}

#[derive(Debug, Clone)]
pub enum PaintEvent {
    ActiveEditorChanged {
        editor: Option<ComponentId>,
    },
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
    WorldPanelSelectionChanged {
        component: Option<ComponentId>,
        editor: Option<ComponentId>,
    },
    EditorSelectionChanged {
        editor: ComponentId,
        component: Option<ComponentId>,
    },
    SceneClick {
        editor: ComponentId,
        renderable: ComponentId,
        hit_point: [f32; 3],
    },
    StrokeStarted {
        editor: ComponentId,
        renderable: ComponentId,
        hit_point: [f32; 3],
    },
    StrokeMoved {
        editor: ComponentId,
        renderable: ComponentId,
        hit_point: [f32; 3],
    },
    StrokeEnded {
        editor: ComponentId,
    },
}

pub fn reduce_paint_state(old: &PaintState, event: &PaintEvent) -> PaintState {
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
        PaintEvent::ActiveEditorChanged { editor }
        | PaintEvent::PanelFocusChanged {
            focused_panel: editor,
        } => {
            let _ = editor;
        }
        PaintEvent::WorldPanelSelectionChanged { component, editor } => {
            let _ = (component, editor);
        }
        PaintEvent::EditorSelectionChanged { editor, component } => {
            let _ = (editor, component);
        }
        PaintEvent::SceneClick { editor, .. }
        | PaintEvent::StrokeStarted { editor, .. }
        | PaintEvent::StrokeMoved { editor, .. }
        | PaintEvent::StrokeEnded { editor } => {
            let _ = editor;
        }
    }

    match event {
        PaintEvent::StrokeStarted { .. } => {
            new.stroke = PaintStrokeMode::Dragging;
        }
        PaintEvent::StrokeEnded { .. } => {
            new.stroke = PaintStrokeMode::Idle;
        }
        _ => {}
    }

    new
}

pub fn paint_tool_from_item(item: Option<String>) -> PaintTool {
    match item.as_deref() {
        Some(FREE_DRAW_LABEL) => PaintTool::FreeDraw,
        Some(LINE_LABEL) => PaintTool::Line,
        Some(SPRAY_CAN_LABEL) => PaintTool::SprayCan,
        Some(FILL_LABEL) => PaintTool::Fill,
        Some(ERASE_LABEL) => PaintTool::Erase,
        _ => PaintTool::Unknown(item),
    }
}

pub fn is_paint_panel_focused(
    paint_panel_root: Option<ComponentId>,
    editor_context: &EditorContextState,
) -> bool {
    paint_panel_root.is_some_and(|panel_root| editor_context.focused_panel == Some(panel_root))
}

pub fn is_paint_active(
    paint_panel_root: Option<ComponentId>,
    paint_state: &PaintState,
    editor_context: &EditorContextState,
) -> bool {
    is_paint_panel_focused(paint_panel_root, editor_context)
        && paint_state.selected_tool == PaintTool::FreeDraw
        && paint_state
            .selected_asset
            .as_ref()
            .and_then(|selection| selection.component)
            .is_some()
}

// ── DataRendererSystem infrastructure ──

pub(crate) const PAINT_PANEL_PAYLOAD_NAME: &str = "paint_panel_payload";
pub(crate) const PAINT_ITEM_PREFIX: &str = "paint_item_";

fn build_paint_tool_items() -> Vec<UiItem> {
    let tool_labels = [FREE_DRAW_LABEL, LINE_LABEL, SPRAY_CAN_LABEL, FILL_LABEL, ERASE_LABEL];
    tool_labels
        .into_iter()
        .enumerate()
        .map(|(index, label)| UiItem {
            key: format!("{PAINT_ITEM_PREFIX}{index}"),
            kind: UiItemKind::Component,
            label: label.to_string(),
            selected: index == 0,
            target_ref: None,
        })
        .collect()
}

fn paint_tool_row_render_fn(
    world: &mut World,
    _emit: &mut dyn SignalEmitter,
    item: &UiItem,
) -> Result<ComponentId, String> {
    Ok(spawn_panel_ui_row_tree(
        world,
        PanelUiRowSpec {
            row_name: &item.key,
            payload_name: PAINT_PANEL_PAYLOAD_NAME,
            target_component: None,
            label: &item.label,
            row_kind_label: "PaintTool",
            interactive: true,
            background_rgba: if item.selected {
                [1.00, 0.88, 0.20, 0.96]
            } else {
                [0.85, 0.90, 0.95, 1.0]
            },
            text_rgba: if item.selected {
                [0.08, 0.08, 0.02, 1.0]
            } else {
                [0.06, 0.09, 0.08, 1.0]
            },
            font_size_gu: Some(1.0),
            spacer_height_gu: None,
        },
    ))
}

pub(crate) static PAINT_TOOL_ROW_SPEC: LazyLock<ItemRendererSpec> = LazyLock::new(|| {
    RendererSpec::Rust {
        render_fn: Box::new(paint_tool_row_render_fn),
    }
});

pub(crate) fn rerender_paint_panel_content(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    content_slot: ComponentId,
    data_renderer: &mut DataRendererSystem,
) {
    let items = build_paint_tool_items();
    let container = match data_renderer.render_list(world, emit, content_slot, &PAINT_TOOL_ROW_SPEC, &items) {
        Ok(container) => container,
        Err(error) => {
            eprintln!("[PaintPanel] render_list error: {error}");
            return;
        }
    };

    let first_item = world.find_component(container, &format!("#{}0", PAINT_ITEM_PREFIX));
    let selection = world.add_component_boxed_named(
        "paint_tool_selection",
        Box::new(SelectionComponent::new()),
    );
    if let Some(selection_component) =
        world.get_component_by_id_as_mut::<SelectionComponent>(selection)
    {
        selection_component.payload_selector =
            Some(format!("[name='{PAINT_PANEL_PAYLOAD_NAME}']"));
        if let Some(item_component) = first_item {
            selection_component.select_entry(SelectionEntry {
                index: Some(0),
                component: item_component,
            });
        }
    }
    let _ = world.add_child(container, selection);
}
