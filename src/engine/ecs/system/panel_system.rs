use std::collections::HashMap;

use crate::engine::ecs::component::{
    DataComponent, DataValue, SelectableComponent, SelectionComponent, SerializeComponent,
    TransformComponent,
};
use crate::engine::ecs::{ComponentId, SignalEmitter, World};
use crate::meow_meow::component_registry::spawn_tree;
use crate::meow_meow::object::{CeChild, MaterializedCE, Value};
use crate::meow_meow::runner::MeowMeowRunner;

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

pub struct PanelLayoutMountSpec {
    pub anchor_pos: (f32, f32, f32),
    pub total_height_gu: f64,
    pub available_width_gu: f64,
    pub text_scale: f64,
    pub mount_name: String,
    pub layout_name: String,
    pub children: Vec<MaterializedCE>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnedPanelInstance {
    pub mount_root: ComponentId,
    pub instance: PanelInstance,
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

pub fn build_panel_shell_component_expr(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    spec: &PanelShellSpec,
) -> Result<MaterializedCE, String> {
    MeowMeowRunner::materialize_mms_module_component_from_file(
        &spec.asset_path,
        &spec.export_name,
        spec.args.clone(),
        Some(world),
        Some(emit),
    )
}

pub fn decorate_panel_root_ce(
    mut panel_root: MaterializedCE,
    margin_right_gu: f64,
) -> MaterializedCE {
    panel_root.children.insert(
        0,
        CeChild::Spawn(MaterializedCE {
            component_type: "Option".to_string(),
            ctor_method: None,
            ctor_args: Vec::new(),
            calls: Vec::new(),
            named: Vec::new(),
            positionals: Vec::new(),
            children: Vec::new(),
        }),
    );
    panel_root.children.insert(
        1,
        CeChild::Spawn(MaterializedCE {
            component_type: "Raycastable".to_string(),
            ctor_method: Some("enabled".to_string()),
            ctor_args: Vec::new(),
            calls: Vec::new(),
            named: Vec::new(),
            positionals: Vec::new(),
            children: Vec::new(),
        }),
    );

    if let Some(CeChild::Spawn(style_ce)) = panel_root.children.iter_mut().find(|child| {
        matches!(
            child,
            CeChild::Spawn(MaterializedCE {
                component_type,
                ..
            }) if component_type == "Style"
        )
    }) {
        style_ce.calls.push((
            "display".to_string(),
            vec![Value::String("inline-block".to_string())],
        ));
        style_ce.calls.push((
            "margin_right".to_string(),
            vec![Value::Number(margin_right_gu)],
        ));
    }

    panel_root
}

pub fn build_panel_layout_mount_ce(spec: PanelLayoutMountSpec) -> MaterializedCE {
    let shared_layout_root = MaterializedCE {
        component_type: "LayoutRoot".to_string(),
        ctor_method: None,
        ctor_args: Vec::new(),
        calls: vec![
            (
                "available_width".to_string(),
                vec![Value::Number(spec.available_width_gu)],
            ),
            (
                "available_height".to_string(),
                vec![Value::Number(spec.total_height_gu)],
            ),
            (
                "unit_scale".to_string(),
                vec![Value::Number(spec.text_scale)],
            ),
        ],
        named: vec![("name".to_string(), Value::String(spec.layout_name))],
        positionals: Vec::new(),
        children: spec.children.into_iter().map(CeChild::Spawn).collect(),
    };

    let overlay_ce = MaterializedCE {
        component_type: "Overlay".to_string(),
        ctor_method: None,
        ctor_args: Vec::new(),
        calls: Vec::new(),
        named: Vec::new(),
        positionals: Vec::new(),
        children: vec![CeChild::Spawn(shared_layout_root)],
    };

    MaterializedCE {
        component_type: "T".to_string(),
        ctor_method: Some("position".to_string()),
        ctor_args: vec![
            Value::Number(spec.anchor_pos.0 as f64),
            Value::Number(spec.anchor_pos.1 as f64),
            Value::Number(spec.anchor_pos.2 as f64),
        ],
        calls: Vec::new(),
        named: vec![("name".to_string(), Value::String(spec.mount_name))],
        positionals: Vec::new(),
        children: vec![CeChild::Spawn(overlay_ce)],
    }
}

pub fn spawn_panel_layout_mount(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    spec: PanelLayoutMountSpec,
) -> Result<(ComponentId, ComponentId), String> {
    let mount_name = spec.mount_name.clone();
    let layout_name = spec.layout_name.clone();
    let mount_ce = build_panel_layout_mount_ce(spec);
    let mount_root = spawn_tree(&mount_ce, None, world, emit)?;
    let layout_root = world
        .find_component(mount_root, &format!("#{layout_name}"))
        .ok_or_else(|| format!("missing layout root #{layout_name} under {mount_name}"))?;
    Ok((mount_root, layout_root))
}

pub fn resolve_panel_instance(
    world: &World,
    editor_root: ComponentId,
    spec: &PanelShellSpec,
    root: ComponentId,
    instance_id: Option<u64>,
) -> Option<PanelInstance> {
    let shell_root = world.find_component(root, &spec.root_selector)?;
    let mut slots = HashMap::new();
    for (kind, selector) in &spec.slot_selectors {
        let slot = world.find_component(shell_root, selector)?;
        slots.insert(*kind, slot);
    }
    Some(PanelInstance {
        panel_kind: spec.panel_kind,
        editor_root,
        root: shell_root,
        slots,
        instance_id,
    })
}

pub fn spawn_panel_instance(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    spec: &PanelShellSpec,
    instance_id: Option<u64>,
    margin_right_gu: f64,
) -> Result<SpawnedPanelInstance, String> {
    let panel_ce = build_panel_shell_component_expr(world, emit, spec)?;
    let panel_ce = decorate_panel_root_ce(panel_ce, margin_right_gu);
    let mount_root = spawn_tree(&panel_ce, None, world, emit)?;
    let instance = resolve_panel_instance(world, mount_root, spec, mount_root, instance_id)
        .ok_or_else(|| {
            format!(
                "failed to resolve {:?} panel instance from root selector {}",
                spec.panel_kind, spec.root_selector
            )
        })?;
    Ok(SpawnedPanelInstance {
        mount_root,
        instance,
    })
}

pub fn decode_panel_action_payload(
    world: &World,
    node: ComponentId,
    payload_name: &str,
    panel_kind: PanelKind,
    action_kind: PanelActionKind,
    instance_id: Option<u64>,
    field_key: Option<String>,
) -> Option<PanelActionPayload> {
    let mut current = Some(node);
    while let Some(component_id) = current {
        if let Some(payload) = world.children_of(component_id).iter().find_map(|&child| {
            (world.component_label(child) == Some(payload_name))
                .then_some(child)
                .and_then(|id| world.get_component_by_id_as::<DataComponent>(id))
        }) {
            return Some(PanelActionPayload {
                panel_kind,
                action_kind,
                item_key: data_text(payload, "row_name").or_else(|| data_text(payload, "label")),
                target_component: payload.get_component("target_component"),
                instance_id,
                field_key: field_key.or_else(|| data_text(payload, "field_key")),
            });
        }
        current = world.parent_of(component_id);
    }
    None
}

fn data_text(data: &DataComponent, key: &str) -> Option<String> {
    match data.get(key) {
        Some(DataValue::Text(value)) => Some(value.clone()),
        _ => None,
    }
}
