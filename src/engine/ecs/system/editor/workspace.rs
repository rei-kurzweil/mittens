use std::sync::{Arc, Mutex};

use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::system::panel_system::get_or_create_runtime_ui_root;

#[derive(Debug, Default)]
pub(crate) struct EditorWorkspaceRuntime {
    panel_handler_installed: bool,
    panel_layout_spawned: bool,
    installed_editor_roots: Arc<Mutex<Vec<ComponentId>>>,
    refresh_handler_editor_roots: Arc<Mutex<Vec<ComponentId>>>,
    runtime_ui_root: Arc<Mutex<Option<ComponentId>>>,
}

impl EditorWorkspaceRuntime {
    pub(crate) fn panel_handler_installed(&self) -> bool {
        self.panel_handler_installed
    }

    pub(crate) fn mark_panel_handler_installed(&mut self) {
        self.panel_handler_installed = true;
    }

    pub(crate) fn panel_layout_spawned_mut(&mut self) -> &mut bool {
        &mut self.panel_layout_spawned
    }

    pub(crate) fn installed_editor_roots(&self) -> &Arc<Mutex<Vec<ComponentId>>> {
        &self.installed_editor_roots
    }

    pub(crate) fn refresh_handler_editor_roots(&self) -> &Arc<Mutex<Vec<ComponentId>>> {
        &self.refresh_handler_editor_roots
    }

    pub(crate) fn runtime_ui_root_handle(&self) -> Arc<Mutex<Option<ComponentId>>> {
        Arc::clone(&self.runtime_ui_root)
    }

    pub(crate) fn current_runtime_ui_root(&self) -> Option<ComponentId> {
        *self
            .runtime_ui_root
            .lock()
            .expect("runtime ui root mutex poisoned")
    }

    pub(crate) fn get_or_create_runtime_ui_root(&self, world: &mut World) -> ComponentId {
        let runtime_ui_root = get_or_create_runtime_ui_root(world);
        *self
            .runtime_ui_root
            .lock()
            .expect("runtime ui root mutex poisoned") = Some(runtime_ui_root);
        runtime_ui_root
    }
}
