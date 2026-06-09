use crate::engine::ecs::system::editor::context::EditorContextState;
use crate::engine::ecs::ComponentId;

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
        && !matches!(paint_state.selected_tool, PaintTool::Unknown(_))
        && paint_state
            .selected_asset
            .as_ref()
            .and_then(|selection| selection.component)
            .is_some()
}


