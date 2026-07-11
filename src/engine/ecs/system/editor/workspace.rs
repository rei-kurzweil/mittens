use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::engine::ecs::system::editor::grid_panel::GRID_PANEL_ROOT_SELECTOR;
use crate::engine::ecs::system::editor::pose_panel::POSE_PANEL_ROOT_SELECTOR;
use crate::engine::ecs::system::panel_system::{
    PanelControlKind, PanelInstance, PanelKind, PanelShellSpec, PanelSlotKind,
    get_or_create_runtime_ui_root, resolve_panel_instance,
};
use crate::engine::ecs::{ComponentId, World};

pub(crate) const PANEL_LAYOUT_MOUNT_NAME: &str = "editor_panel_layout_mount";
pub(crate) const WORLD_PANEL_ROOT_SELECTOR: &str = "#world_panel_root";
pub(crate) const PANEL_CONTENT_SLOT_SELECTOR: &str = "#content_slot";
pub(crate) const WORLD_PANEL_SELECTION_SELECTOR: &str = "#world_panel_selection";
pub(crate) const PAINT_PANEL_ROOT_SELECTOR: &str = "#paint_panel_root";
pub(crate) const COLOR_PANEL_ROOT_SELECTOR: &str = "#color_panel_root";

#[derive(Debug, Default)]
pub(crate) struct EditorWorkspaceRuntime {
    panel_handler_installed: bool,
    panel_layout_spawned: bool,
    installed_editor_roots: Arc<Mutex<Vec<ComponentId>>>,
    refresh_handler_editor_roots: Arc<Mutex<Vec<ComponentId>>>,
    runtime_ui_root: Arc<Mutex<Option<ComponentId>>>,
    mounted_panels: HashMap<PanelKind, PanelInstance>,
    panel_layout_mount_root: Option<ComponentId>,
    focused_panel: Option<PanelKind>,
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

    pub(crate) fn panel_instance(&self, kind: PanelKind) -> Option<&PanelInstance> {
        self.mounted_panels.get(&kind)
    }

    pub(crate) fn panel_layout_mount_root(&self) -> Option<ComponentId> {
        self.panel_layout_mount_root
    }

    pub(crate) fn focused_panel(&self) -> Option<PanelKind> {
        self.focused_panel
    }

    pub(crate) fn set_focused_panel(&mut self, kind: PanelKind) {
        self.focused_panel = Some(kind);
    }

    pub(crate) fn find_panel_mount_root(&self, world: &World) -> Option<ComponentId> {
        world.all_components().find(|&id| {
            world
                .component_label(id)
                .is_some_and(|label| label == PANEL_LAYOUT_MOUNT_NAME)
        })
    }

    pub(crate) fn resolve_and_cache_static_panels(
        &mut self,
        world: &World,
        editor_root: ComponentId,
        mount_root: ComponentId,
    ) {
        let mut mounted = HashMap::new();

        let panel_roots: [(PanelKind, &str); 6] = [
            (PanelKind::World, WORLD_PANEL_ROOT_SELECTOR),
            (PanelKind::Grid, GRID_PANEL_ROOT_SELECTOR),
            (PanelKind::Paint, PAINT_PANEL_ROOT_SELECTOR),
            (PanelKind::Color, COLOR_PANEL_ROOT_SELECTOR),
            (PanelKind::Assets, "#assets_root"),
            (PanelKind::Pose, POSE_PANEL_ROOT_SELECTOR),
        ];
        for (kind, root_sel) in &panel_roots {
            let spec = PanelShellSpec {
                panel_kind: *kind,
                asset_path: String::new(),
                export_name: String::new(),
                args: Vec::new(),
                root_selector: root_sel.to_string(),
                slot_selectors: HashMap::new(),
                control_selectors: HashMap::new(),
            };
            if let Some(instance) =
                resolve_panel_instance(world, editor_root, &spec, mount_root, None)
            {
                mounted.insert(*kind, instance);
            }
        }

        // Resolve a richer world panel instance with known slots/controls
        {
            let spec = PanelShellSpec {
                panel_kind: PanelKind::World,
                asset_path: String::new(),
                export_name: String::new(),
                args: Vec::new(),
                root_selector: WORLD_PANEL_ROOT_SELECTOR.to_string(),
                slot_selectors: HashMap::from([(
                    PanelSlotKind::List,
                    PANEL_CONTENT_SLOT_SELECTOR.to_string(),
                )]),
                control_selectors: HashMap::from([(
                    PanelControlKind::Selection,
                    WORLD_PANEL_SELECTION_SELECTOR.to_string(),
                )]),
            };
            if let Some(instance) =
                resolve_panel_instance(world, editor_root, &spec, mount_root, None)
            {
                mounted.insert(PanelKind::World, instance);
            }
        }

        self.mounted_panels = mounted;
        self.panel_layout_mount_root = Some(mount_root);
    }
}
