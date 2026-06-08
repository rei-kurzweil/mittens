use crate::engine::ecs::ComponentId;

pub(crate) type InspectorPanelId = u64;

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
            new.active_panel = Some(active_panel.panel_id);
            new.pending_spawn_target = None;
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
