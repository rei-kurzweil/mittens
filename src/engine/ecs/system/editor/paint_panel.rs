use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::EditorInteractionMode;
use crate::engine::ecs::system::editor::context::EditorContextState;

pub const COLOR_PANEL_ROOT_SELECTOR: &str = "#color_panel_root";
pub const COLOR_PANEL_SELECTION_SELECTOR: &str = "#color_panel_selection";
pub const ACTIVE_COLOR_WELL_SELECTOR: &str = "#active_color_well";
pub const COLOR_SWATCH_PAYLOAD_NAME: &str = "color_swatch_payload";

const FREE_DRAW_LABEL: &str = "Free Draw";
const LINE_LABEL: &str = "Line";
const SPRAY_CAN_LABEL: &str = "Spray Can";
const COLOR_LABEL: &str = "Color";
const FILL_LABEL: &str = "Fill";
const ERASE_LABEL: &str = "Erase";
const GRID_TOOL_LABEL: &str = "Grid Tool";

#[derive(Debug, Clone, PartialEq)]
pub struct PaintState {
    pub selected_asset: Option<PaintSelection>,
    pub selected_tool: PaintTool,
    pub selected_color: Option<[f32; 4]>,
    /// Enables snapping when the current raycast target is itself a grid.
    /// This is deliberately a permission flag: selecting a grid elsewhere in
    /// the editor must not make ordinary scene hits snap to that grid.
    pub snap_enabled: bool,
    pub stroke: PaintStrokeMode,
}

impl Default for PaintState {
    fn default() -> Self {
        Self {
            selected_asset: None,
            selected_tool: PaintTool::Unknown(None),
            selected_color: None,
            snap_enabled: true,
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
    GridTool,
    Line,
    SprayCan,
    Color,
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
    ColorSelectionChanged {
        item: Option<String>,
        component: Option<ComponentId>,
        rgba: Option<[f32; 4]>,
    },
    SnapSelectionChanged {
        enabled: bool,
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
        PaintEvent::ColorSelectionChanged {
            item,
            component,
            rgba,
        } => {
            let _ = (item, component);
            new.selected_color = *rgba;
        }
        PaintEvent::SnapSelectionChanged { enabled } => {
            new.snap_enabled = *enabled;
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
        Some(GRID_TOOL_LABEL) => PaintTool::GridTool,
        Some(LINE_LABEL) => PaintTool::Line,
        Some(SPRAY_CAN_LABEL) => PaintTool::SprayCan,
        Some(COLOR_LABEL) => PaintTool::Color,
        Some(FILL_LABEL) => PaintTool::Fill,
        Some(ERASE_LABEL) => PaintTool::Erase,
        _ => PaintTool::Unknown(item),
    }
}

pub fn is_paint_workspace_focused(
    paint_panel_root: Option<ComponentId>,
    color_panel_root: Option<ComponentId>,
    editor_context: &EditorContextState,
) -> bool {
    let focused = paint_panel_root
        .is_some_and(|panel_root| editor_context.focused_panel == Some(panel_root))
        || color_panel_root
            .is_some_and(|panel_root| editor_context.focused_panel == Some(panel_root));
    eprintln!(
        "🎨🖌️ paint_debug is_paint_workspace_focused paint_panel_root={paint_panel_root:?} color_panel_root={color_panel_root:?} focused_panel={:?} → {focused}",
        editor_context.focused_panel
    );
    focused
}

pub fn is_paint_panel_focused(
    paint_panel_root: Option<ComponentId>,
    editor_context: &EditorContextState,
) -> bool {
    let focused =
        paint_panel_root.is_some_and(|panel_root| editor_context.focused_panel == Some(panel_root));
    eprintln!(
        "🎨🖌️ paint_debug is_paint_panel_focused paint_panel_root={paint_panel_root:?} focused_panel={:?} → {focused}",
        editor_context.focused_panel
    );
    focused
}

pub fn is_paint_active(
    paint_panel_root: Option<ComponentId>,
    color_panel_root: Option<ComponentId>,
    paint_state: &PaintState,
    editor_context: &EditorContextState,
) -> bool {
    let _ = (paint_panel_root, color_panel_root);
    let mode_ok = editor_context.interaction_mode == EditorInteractionMode::Paint;
    let tool_ok = !matches!(paint_state.selected_tool, PaintTool::Unknown(_));
    let asset_ok = if matches!(
        paint_state.selected_tool,
        PaintTool::Erase | PaintTool::GridTool | PaintTool::Color
    ) {
        true
    } else {
        paint_state
            .selected_asset
            .as_ref()
            .and_then(|selection| selection.component)
            .is_some()
    };
    let result = mode_ok && tool_ok && asset_ok;
    eprintln!(
        "🎨🖌️ paint_debug is_paint_active result={result} mode_ok={mode_ok} tool_ok={tool_ok} asset_ok={asset_ok} tool={:?} asset={:?} color={:?} panel={paint_panel_root:?} focused_panel={:?}",
        paint_state.selected_tool,
        paint_state.selected_asset,
        paint_state.selected_color,
        editor_context.focused_panel
    );
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_and_fill_labels_select_distinct_tools() {
        assert_eq!(
            paint_tool_from_item(Some("Color".to_string())),
            PaintTool::Color
        );
        assert_eq!(
            paint_tool_from_item(Some("Fill".to_string())),
            PaintTool::Fill
        );
    }

    #[test]
    fn snap_selection_updates_only_the_snap_permission() {
        let initial = PaintState::default();
        let disabled = reduce_paint_state(
            &initial,
            &PaintEvent::SnapSelectionChanged { enabled: false },
        );
        assert!(!disabled.snap_enabled);
        assert_eq!(disabled.selected_tool, initial.selected_tool);

        let enabled = reduce_paint_state(
            &disabled,
            &PaintEvent::SnapSelectionChanged { enabled: true },
        );
        assert!(enabled.snap_enabled);
    }

    #[test]
    fn paint_activity_is_owned_by_mode_not_panel_focus() {
        let paint_state = PaintState {
            selected_tool: PaintTool::Erase,
            ..PaintState::default()
        };
        let context = EditorContextState {
            interaction_mode: EditorInteractionMode::Paint,
            focused_panel: None,
            ..EditorContextState::default()
        };

        assert!(is_paint_active(None, None, &paint_state, &context));
    }

    #[test]
    fn authored_paint_panel_exposes_color_but_not_fill() {
        let panel = include_str!("../../../../../assets/components/internal/panels.mms");
        let icons = include_str!("../../../../../assets/components/icons.mms");

        assert!(panel.contains(
            "let tool_names = [\"Free Draw\", \"Grid Tool\", \"Line\", \"Spray Can\", \"Color\", \"Erase\"]"
        ));
        assert!(!panel.contains("\"Fill\""));
        assert!(panel.contains("name = \"paint_snap_selection\""));
        assert!(panel.contains("Text { \"Snap?\" }"));
        assert!(panel.contains("return color_icon()"));
        let color_icon = icons
            .split_once("export fn color_icon()")
            .and_then(|(_, rest)| rest.split_once("export fn erase_icon()"))
            .map(|(body, _)| body)
            .expect("color icon factory before erase icon");
        assert_eq!(color_icon.matches("R.circle_2d()").count(), 3);
        for primary in [
            "C.rgba(1.0, 0.12, 0.12, 0.90)",
            "C.rgba(0.12, 0.95, 0.20, 0.90)",
            "C.rgba(0.12, 0.35, 1.0, 0.90)",
        ] {
            assert!(color_icon.contains(primary));
        }
    }
}
