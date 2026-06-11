use std::collections::HashMap;

use crate::engine::ecs::component::{
    SelectableComponent, SelectionComponent, SerializeComponent, TransformComponent,
};
use crate::engine::ecs::{ComponentId, World};

pub const EDITOR_RUNTIME_UI_ROOT_NAME: &str = "editor_runtime_ui_root";
pub const PANEL_LAYOUT_MOUNT_NAME: &str = "editor_panel_layout_mount";
pub const PANEL_LAYOUT_ROOT_NAME: &str = "editor_panel_layout_root";
pub const PANEL_LAYOUT_SELECTION_NAME: &str = "editor_panel_layout_selection";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelKind {
    World,
    Inspector,
    Paint,
    Assets,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelSlotKind {
    List,
    Detail,
    Status,
    Sidebar,
    Toolbar,
    Footer,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PanelShellSpec {
    pub panel_kind: PanelKind,
    pub asset_path: String,
    pub export_name: String,
    pub args: Vec<crate::meow_meow::object::Value>,
    pub root_selector: String,
    pub slot_selectors: HashMap<PanelSlotKind, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelInstance {
    pub panel_kind: PanelKind,
    pub editor_root: ComponentId,
    pub root: ComponentId,
    pub slots: HashMap<PanelSlotKind, ComponentId>,
    pub instance_id: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelActionKind {
    Select,
    Toggle,
    Delete,
    Add,
    Focus,
    Pin,
    ActivateField,
    EditField,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelActionPayload {
    pub panel_kind: PanelKind,
    pub action_kind: PanelActionKind,
    pub item_key: Option<String>,
    pub target_component: Option<ComponentId>,
    pub instance_id: Option<u64>,
    pub field_key: Option<String>,
}

pub fn find_named_root(world: &World, name: &str) -> Option<ComponentId> {
    world.all_components().find(|&component_id| {
        world.parent_of(component_id).is_none()
            && world
                .component_label(component_id)
                .is_some_and(|label| label == name)
    })
}

pub fn get_or_create_runtime_ui_root(world: &mut World) -> ComponentId {
    if let Some(runtime_ui_root) = find_named_root(world, EDITOR_RUNTIME_UI_ROOT_NAME) {
        return runtime_ui_root;
    }

    let runtime_ui_root = world.add_component_boxed_named(
        EDITOR_RUNTIME_UI_ROOT_NAME,
        Box::new(TransformComponent::new()),
    );
    let runtime_ui_selectable = world.add_component_boxed_named(
        "editor_runtime_ui_selectable",
        Box::new(SelectableComponent::off()),
    );
    let _ = world.add_child(runtime_ui_root, runtime_ui_selectable);
    let runtime_ui_serialize = world.add_component_boxed_named(
        "editor_runtime_ui_serialize",
        Box::new(SerializeComponent::off()),
    );
    let _ = world.add_child(runtime_ui_root, runtime_ui_serialize);

    runtime_ui_root
}

pub fn panel_layout_root_id(world: &World, panel_query_root: ComponentId) -> Option<ComponentId> {
    world.find_component(panel_query_root, "#editor_panel_layout_root")
}

pub fn panel_layout_selection_id(
    world: &World,
    panel_query_root: ComponentId,
) -> Option<ComponentId> {
    world.find_component(panel_query_root, "#editor_panel_layout_selection")
}

pub fn ensure_panel_layout_selection(
    world: &mut World,
    layout_root_id: ComponentId,
) -> ComponentId {
    if let Some(existing) = world.find_component(layout_root_id, "#editor_panel_layout_selection") {
        return existing;
    }

    let selection = world.add_component_boxed_named(
        PANEL_LAYOUT_SELECTION_NAME,
        Box::new(SelectionComponent::new()),
    );
    let _ = world.add_child(layout_root_id, selection);
    selection
}

pub fn is_descendant_or_self(world: &World, ancestor: ComponentId, node: ComponentId) -> bool {
    let mut current = Some(node);
    while let Some(component_id) = current {
        if component_id == ancestor {
            return true;
        }
        current = world.parent_of(component_id);
    }
    false
}
