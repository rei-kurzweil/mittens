use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use crate::engine::ecs::component::SelectionComponent;
use crate::engine::ecs::{ComponentId, EventSignal, RxWorld, Signal, SignalKind, World};

const PANEL_LAYOUT_SELECTION_SELECTOR: &str = "#editor_panel_layout_selection";
const WORLD_PANEL_SELECTION_SELECTOR: &str = "#world_panel_selection";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditorContextState {
    pub active_editor: Option<ComponentId>,
    pub selected_component: Option<ComponentId>,
    pub focused_panel: Option<ComponentId>,
}

#[derive(Debug, Clone, Default)]
struct EditorContextWorkspaceState {
    panel_query_root: Option<ComponentId>,
    registered_editors: Vec<ComponentId>,
}

impl EditorContextWorkspaceState {
    fn register_editor(&mut self, editor_root: ComponentId) -> bool {
        if self.registered_editors.contains(&editor_root) {
            return false;
        }
        self.registered_editors.push(editor_root);
        true
    }

    fn default_active_editor(&self) -> Option<ComponentId> {
        self.registered_editors.first().copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorContextEvent {
    ActiveEditorChanged {
        editor: Option<ComponentId>,
        selected_component: Option<ComponentId>,
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
}

pub fn reduce_editor_context_state(
    old: &EditorContextState,
    event: &EditorContextEvent,
) -> EditorContextState {
    let mut new = old.clone();
    match event {
        EditorContextEvent::ActiveEditorChanged {
            editor,
            selected_component,
        } => {
            new.active_editor = *editor;
            new.selected_component = *selected_component;
        }
        EditorContextEvent::PanelFocusChanged { focused_panel } => {
            new.focused_panel = *focused_panel;
        }
        EditorContextEvent::WorldPanelSelectionChanged { component, editor } => {
            new.selected_component = *component;
            if editor.is_some() {
                new.active_editor = *editor;
            }
        }
        EditorContextEvent::EditorSelectionChanged { editor, component } => {
            new.active_editor = Some(*editor);
            new.selected_component = component.or(Some(*editor));
        }
    }
    new
}

#[derive(Debug, Default)]
pub struct EditorContextSystem {
    installed_editor_roots: HashSet<ComponentId>,
    shared_panel_handlers_installed: bool,
    state: Arc<Mutex<EditorContextState>>,
    workspace: Arc<Mutex<EditorContextWorkspaceState>>,
}

impl EditorContextSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shared_state(&self) -> Arc<Mutex<EditorContextState>> {
        Arc::clone(&self.state)
    }

    pub fn install_scoped_handlers_for_editor(
        &mut self,
        rx: &mut RxWorld,
        world: &World,
        editor_root: ComponentId,
        panel_query_root: ComponentId,
    ) {
        {
            let mut workspace = self
                .workspace
                .lock()
                .expect("editor context workspace poisoned");
            workspace.panel_query_root = Some(panel_query_root);
            let registered = workspace.register_editor(editor_root);
            drop(workspace);

            if registered {
                ensure_default_active_editor(&self.state, &self.workspace);
            }
        }

        if !self.shared_panel_handlers_installed {
            self.shared_panel_handlers_installed = true;
            install_shared_panel_handlers(rx, world, panel_query_root, Arc::clone(&self.state));
            bootstrap_editor_context(world, panel_query_root, &self.state, &self.workspace);
        }

        if self.installed_editor_roots.insert(editor_root) {
            install_editor_handlers(rx, editor_root, Arc::clone(&self.state));
        }
    }
}

fn install_shared_panel_handlers(
    rx: &mut RxWorld,
    world: &World,
    panel_query_root: ComponentId,
    state: Arc<Mutex<EditorContextState>>,
) {
    let _ = world;
    rx.add_handler_closure(
        SignalKind::SelectionChanged,
        panel_query_root,
        move |world, _emit, signal| {
            let Some(event) =
                editor_context_event_from_shared_signal(world, panel_query_root, signal)
            else {
                return;
            };
            apply_editor_context_event(&state, &event);
        },
    );
}

fn install_editor_handlers(
    rx: &mut RxWorld,
    editor_root: ComponentId,
    state: Arc<Mutex<EditorContextState>>,
) {
    rx.add_handler_closure(
        SignalKind::SelectionChanged,
        editor_root,
        move |_world, _emit, signal| {
            let Some(EventSignal::SelectionChanged {
                selection_root,
                selected_component,
                ..
            }) = signal.event.as_ref()
            else {
                return;
            };
            if *selection_root != editor_root {
                return;
            }

            apply_editor_context_event(
                &state,
                &EditorContextEvent::EditorSelectionChanged {
                    editor: editor_root,
                    component: *selected_component,
                },
            );
        },
    );
}

fn bootstrap_editor_context(
    world: &World,
    panel_query_root: ComponentId,
    state: &Arc<Mutex<EditorContextState>>,
    workspace: &Arc<Mutex<EditorContextWorkspaceState>>,
) {
    if let Some(selection_root) =
        world.find_component(panel_query_root, PANEL_LAYOUT_SELECTION_SELECTOR)
        && let Some(selection) = world.get_component_by_id_as::<SelectionComponent>(selection_root)
    {
        apply_editor_context_event(
            state,
            &EditorContextEvent::PanelFocusChanged {
                focused_panel: selection.selected_component,
            },
        );
    }

    if let Some(selection_root) =
        world.find_component(panel_query_root, WORLD_PANEL_SELECTION_SELECTOR)
        && let Some(selection) = world.get_component_by_id_as::<SelectionComponent>(selection_root)
    {
        let component = selection.selected_component;
        apply_editor_context_event(
            state,
            &EditorContextEvent::WorldPanelSelectionChanged {
                component,
                editor: component.and_then(|target| nearest_editor_ancestor(world, target)),
            },
        );
    } else if let Some(editor_root) = workspace
        .lock()
        .expect("editor context workspace poisoned")
        .default_active_editor()
    {
        apply_editor_context_event(
            state,
            &EditorContextEvent::ActiveEditorChanged {
                editor: Some(editor_root),
                selected_component: Some(editor_root),
            },
        );
    }
}

fn editor_context_event_from_shared_signal(
    world: &World,
    panel_query_root: ComponentId,
    signal: &Signal,
) -> Option<EditorContextEvent> {
    let EventSignal::SelectionChanged {
        selection_root,
        selected_entries,
        selected_component,
        ..
    } = signal.event.as_ref()?
    else {
        return None;
    };

    let component =
        selected_component.or_else(|| selected_entries.last().map(|entry| entry.component));
    let panel_layout_selection_root =
        world.find_component(panel_query_root, PANEL_LAYOUT_SELECTION_SELECTOR);
    let world_panel_selection_root =
        world.find_component(panel_query_root, WORLD_PANEL_SELECTION_SELECTOR);

    if panel_layout_selection_root == Some(*selection_root) {
        Some(EditorContextEvent::PanelFocusChanged {
            focused_panel: component,
        })
    } else if world_panel_selection_root == Some(*selection_root) {
        Some(EditorContextEvent::WorldPanelSelectionChanged {
            component,
            editor: component.and_then(|target| nearest_editor_ancestor(world, target)),
        })
    } else {
        None
    }
}

fn apply_editor_context_event(state: &Arc<Mutex<EditorContextState>>, event: &EditorContextEvent) {
    let mut state = state.lock().expect("editor context state poisoned");
    *state = reduce_editor_context_state(&state, event);
}

fn ensure_default_active_editor(
    state: &Arc<Mutex<EditorContextState>>,
    workspace: &Arc<Mutex<EditorContextWorkspaceState>>,
) {
    let default_editor = workspace
        .lock()
        .expect("editor context workspace poisoned")
        .default_active_editor();
    let Some(default_editor) = default_editor else {
        return;
    };

    let mut state = state.lock().expect("editor context state poisoned");
    if state.active_editor.is_none() {
        *state = reduce_editor_context_state(
            &state,
            &EditorContextEvent::ActiveEditorChanged {
                editor: Some(default_editor),
                selected_component: Some(default_editor),
            },
        );
    }
}

fn nearest_editor_ancestor(world: &World, start: ComponentId) -> Option<ComponentId> {
    let mut cur = Some(start);
    while let Some(node) = cur {
        if world
            .get_component_by_id_as::<crate::engine::ecs::component::EditorComponent>(node)
            .is_some()
        {
            return Some(node);
        }
        cur = world.parent_of(node);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{EditorContextEvent, EditorContextState, reduce_editor_context_state};
    use crate::engine::ecs::World;
    use crate::engine::ecs::component::TransformComponent;

    fn cid(world: &mut World) -> crate::engine::ecs::ComponentId {
        world.add_component_boxed(Box::new(TransformComponent::new()))
    }

    #[test]
    fn defaults_to_first_editor_root_selection() {
        let mut world = World::default();
        let editor = cid(&mut world);
        let next = reduce_editor_context_state(
            &EditorContextState::default(),
            &EditorContextEvent::ActiveEditorChanged {
                editor: Some(editor),
                selected_component: Some(editor),
            },
        );

        assert_eq!(next.active_editor, Some(editor));
        assert_eq!(next.selected_component, Some(editor));
    }

    #[test]
    fn world_panel_root_selection_switches_active_editor() {
        let mut world = World::default();
        let selected = cid(&mut world);
        let editor = cid(&mut world);
        let next = reduce_editor_context_state(
            &EditorContextState::default(),
            &EditorContextEvent::WorldPanelSelectionChanged {
                component: Some(selected),
                editor: Some(editor),
            },
        );

        assert_eq!(next.active_editor, Some(editor));
        assert_eq!(next.selected_component, Some(selected));
    }

    #[test]
    fn scene_selection_switches_editor_and_component_together() {
        let mut world = World::default();
        let editor = cid(&mut world);
        let selected = cid(&mut world);
        let next = reduce_editor_context_state(
            &EditorContextState::default(),
            &EditorContextEvent::EditorSelectionChanged {
                editor,
                component: Some(selected),
            },
        );

        assert_eq!(next.active_editor, Some(editor));
        assert_eq!(next.selected_component, Some(selected));
    }

    #[test]
    fn panel_focus_updates_independently() {
        let mut world = World::default();
        let panel = cid(&mut world);
        let next = reduce_editor_context_state(
            &EditorContextState::default(),
            &EditorContextEvent::PanelFocusChanged {
                focused_panel: Some(panel),
            },
        );

        assert_eq!(next.focused_panel, Some(panel));
    }

    #[test]
    fn editor_root_selection_is_preserved_when_only_root_is_active() {
        let mut world = World::default();
        let editor = cid(&mut world);
        let next = reduce_editor_context_state(
            &EditorContextState::default(),
            &EditorContextEvent::WorldPanelSelectionChanged {
                component: Some(editor),
                editor: Some(editor),
            },
        );

        assert_eq!(next.active_editor, Some(editor));
        assert_eq!(next.selected_component, Some(editor));
    }
}
