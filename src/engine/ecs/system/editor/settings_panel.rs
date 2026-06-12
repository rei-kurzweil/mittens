use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::EditorInteractionMode;

pub(crate) const EDITOR_SETTINGS_PANEL_ROOT_SELECTOR: &str = "#editor_settings_panel_root";
pub(crate) const EDITOR_SETTINGS_SELECTION_NAME: &str = "editor_settings_selection";
pub(crate) const EDITOR_SETTINGS_SELECTION_SELECTOR: &str = "#editor_settings_selection";
pub(crate) const EDITOR_SETTINGS_PAYLOAD_NAME: &str = "editor_settings_payload";
pub(crate) const EDITOR_SETTINGS_SELECT_ROW_NAME: &str = "editor_settings_mode_select";
pub(crate) const EDITOR_SETTINGS_CURSOR_ROW_NAME: &str = "editor_settings_mode_cursor_3d";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditorSettingsOption {
    Select,
    Cursor3d,
}

impl EditorSettingsOption {
    pub(crate) fn interaction_mode(self) -> EditorInteractionMode {
        match self {
            Self::Select => EditorInteractionMode::Select,
            Self::Cursor3d => EditorInteractionMode::Cursor3d,
        }
    }

    pub(crate) fn row_name(self) -> &'static str {
        match self {
            Self::Select => EDITOR_SETTINGS_SELECT_ROW_NAME,
            Self::Cursor3d => EDITOR_SETTINGS_CURSOR_ROW_NAME,
        }
    }

    pub(crate) fn from_row_name(row_name: &str) -> Option<Self> {
        match row_name {
            EDITOR_SETTINGS_SELECT_ROW_NAME => Some(Self::Select),
            EDITOR_SETTINGS_CURSOR_ROW_NAME => Some(Self::Cursor3d),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorSettingsPanelState {
    pub(crate) active_editor: Option<ComponentId>,
    pub(crate) interaction_mode: EditorInteractionMode,
}

impl Default for EditorSettingsPanelState {
    fn default() -> Self {
        Self {
            active_editor: None,
            interaction_mode: EditorInteractionMode::Select,
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
    }
    new
}
