use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::EditorInteractionMode;

pub(crate) const EDITOR_SETTINGS_PANEL_ROOT_SELECTOR: &str = "#editor_settings_panel_root";
pub(crate) const EDITOR_SETTINGS_SELECTION_NAME: &str = "editor_settings_selection";
pub(crate) const EDITOR_SETTINGS_SELECTION_SELECTOR: &str = "#editor_settings_selection";
pub(crate) const EDITOR_SETTINGS_PAYLOAD_NAME: &str = "editor_settings_payload";
pub(crate) const EDITOR_SETTINGS_SELECT_ROW_NAME: &str = "editor_settings_mode_select";
pub(crate) const EDITOR_SETTINGS_CURSOR_ROW_NAME: &str = "editor_settings_mode_cursor_3d";
pub(crate) const EDITOR_SETTINGS_SELECT_CURSOR_ROW_NAME: &str =
    "editor_settings_mode_select_cursor";
pub(crate) const EDITOR_SETTINGS_ARMATURE_ROW_NAME: &str = "editor_settings_armature_visibility";
pub(crate) const EDITOR_SETTINGS_ARMATURE_CHECKMARK_SLOT_NAME: &str = "checkmark_slot";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorSettingsOption {
    Select,
    Cursor3d,
    SelectAndCursor,
}

impl EditorSettingsOption {
    pub(crate) fn interaction_mode(self) -> EditorInteractionMode {
        match self {
            Self::Select => EditorInteractionMode::Select,
            Self::Cursor3d => EditorInteractionMode::Cursor3d,
            Self::SelectAndCursor => EditorInteractionMode::SelectAndCursor,
        }
    }

    pub(crate) fn row_name(self) -> &'static str {
        match self {
            Self::Select => EDITOR_SETTINGS_SELECT_ROW_NAME,
            Self::Cursor3d => EDITOR_SETTINGS_CURSOR_ROW_NAME,
            Self::SelectAndCursor => EDITOR_SETTINGS_SELECT_CURSOR_ROW_NAME,
        }
    }

    pub(crate) fn from_mode_value(mode_value: &str) -> Option<Self> {
        match mode_value {
            "select" => Some(Self::Select),
            "cursor_3d" => Some(Self::Cursor3d),
            "select_cursor" => Some(Self::SelectAndCursor),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorSettingsPanelState {
    pub(crate) active_editor: Option<ComponentId>,
    pub(crate) interaction_mode: EditorInteractionMode,
    pub(crate) armature_visible: bool,
}

impl Default for EditorSettingsPanelState {
    fn default() -> Self {
        Self {
            active_editor: None,
            interaction_mode: EditorInteractionMode::Select,
            armature_visible: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EditorSettingsPanelEvent {
    ActiveEditorChanged {
        editor: Option<ComponentId>,
        interaction_mode: EditorInteractionMode,
    },
    InteractionModeChanged {
        editor: Option<ComponentId>,
        interaction_mode: EditorInteractionMode,
    },
    ArmatureVisibilityChanged {
        visible: bool,
    },
}

pub(crate) fn reduce_editor_settings_panel_state(
    old: &EditorSettingsPanelState,
    event: &EditorSettingsPanelEvent,
) -> EditorSettingsPanelState {
    let mut new = old.clone();
    match event {
        EditorSettingsPanelEvent::ActiveEditorChanged {
            editor,
            interaction_mode,
        }
        | EditorSettingsPanelEvent::InteractionModeChanged {
            editor,
            interaction_mode,
        } => {
            new.active_editor = *editor;
            new.interaction_mode = *interaction_mode;
        }
        EditorSettingsPanelEvent::ArmatureVisibilityChanged { visible } => {
            new.armature_visible = *visible;
        }
    }
    new
}
