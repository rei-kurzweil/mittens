use std::collections::HashMap;
use std::path::Path;

use crate::engine::ecs::component::{
    DataComponent, DataValue, EditorPanel, EditorUIComponent, EditorUIPanelSpec, OptionComponent,
    SelectableComponent, SelectionComponent, SerializeComponent, TransformComponent,
};
use crate::engine::ecs::system::data_renderer_system::{
    DataRendererSystem, DetailRendererSpec, ItemRendererSpec, UiDetailItem, UiItem,
};
use crate::engine::ecs::system::editor::world_panel::WorldPanelModel;
use crate::engine::ecs::{ComponentId, SignalEmitter, World};
use crate::scripting::component_registry::spawn_tree;
use crate::scripting::object::{CeChild, MaterializedCE, Value};
use crate::scripting::runner::MeowMeowRunner;

pub const EDITOR_RUNTIME_UI_ROOT_NAME: &str = "editor_runtime_ui_root";
pub const PANEL_LAYOUT_MOUNT_NAME: &str = "editor_panel_layout_mount";
pub const PANEL_LAYOUT_ROOT_NAME: &str = "editor_panel_layout_root";
pub const PANEL_LAYOUT_SELECTION_NAME: &str = "editor_panel_layout_selection";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelKind {
    Settings,
    World,
    Inspector,
    Paint,
    Color,
    Assets,
    Grid,
    Pose,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanelControlKind {
    Selection,
    TitleLabel,
    PinButton,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PanelShellSpec {
    pub panel_kind: PanelKind,
    pub asset_path: String,
    pub export_name: String,
    pub args: Vec<crate::scripting::object::Value>,
    pub root_selector: String,
    pub slot_selectors: HashMap<PanelSlotKind, String>,
    pub control_selectors: HashMap<PanelControlKind, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelInstance {
    pub panel_kind: PanelKind,
    pub editor_root: ComponentId,
    pub root: ComponentId,
    pub slots: HashMap<PanelSlotKind, ComponentId>,
    pub controls: HashMap<PanelControlKind, ComponentId>,
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

/// Resolve the single authored workspace root, or create the legacy-positioned fallback.
pub fn get_or_create_editor_ui_root(
    world: &mut World,
    legacy_position: (f32, f32, f32),
) -> ComponentId {
    let all_ui: Vec<_> = world
        .all_components()
        .filter(|&id| {
            world
                .get_component_by_id_as::<EditorUIComponent>(id)
                .is_some()
        })
        .collect();
    let authored: Vec<_> = all_ui
        .iter()
        .copied()
        .filter(|&id| world.component_label(id) != Some("editor_ui_fallback"))
        .collect();
    if let Some(&first) = authored.first() {
        if authored.len() > 1 {
            eprintln!(
                "[EditorUI][warning] only one shared EditorUI is supported; ignoring {} additional instance(s)",
                authored.len() - 1
            );
        }
        // An explicit instance authored after an Editor was registered replaces the fallback.
        let fallbacks: Vec<_> = all_ui
            .iter()
            .copied()
            .filter(|&id| world.component_label(id) == Some("editor_ui_fallback"))
            .collect();
        for fallback in fallbacks {
            if let Some(parent) = world.parent_of(fallback)
                && world.component_label(parent) == Some(EDITOR_RUNTIME_UI_ROOT_NAME)
            {
                let _ = world.remove_component_subtree(parent);
            }
        }
        return first;
    }

    if let Some(&fallback) = all_ui.first() {
        return fallback;
    }

    let runtime_root = get_or_create_runtime_ui_root(world);
    if let Some(transform) = world.get_component_by_id_as_mut::<TransformComponent>(runtime_root) {
        *transform = TransformComponent::new().with_position(
            legacy_position.0,
            legacy_position.1,
            legacy_position.2,
        );
    }
    let editor_ui =
        world.add_component_boxed_named("editor_ui_fallback", Box::new(EditorUIComponent::new()));
    let _ = world.add_child(runtime_root, editor_ui);
    editor_ui
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
            component_property_assignment_only: false,
            ctor_method: None,
            ctor_args: Vec::new(),
            calls: Vec::new(),
            named: Vec::new(),
            positionals: Vec::new(),
            deferred_block: None,
            children: Vec::new(),
        }),
    );
    panel_root.children.insert(
        1,
        CeChild::Spawn(MaterializedCE {
            component_type: "Raycastable".to_string(),
            component_property_assignment_only: false,
            ctor_method: Some("enabled".to_string()),
            ctor_args: Vec::new(),
            calls: vec![(
                "interaction_priority".to_string(),
                vec![Value::Number(100.0)],
            )],
            named: Vec::new(),
            positionals: Vec::new(),
            deferred_block: None,
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
        component_property_assignment_only: false,
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
        deferred_block: None,
        children: spec.children.into_iter().map(CeChild::Spawn).collect(),
    };

    MaterializedCE {
        component_type: "T".to_string(),
        component_property_assignment_only: false,
        ctor_method: Some("position".to_string()),
        ctor_args: vec![
            Value::Number(spec.anchor_pos.0 as f64),
            Value::Number(spec.anchor_pos.1 as f64),
            Value::Number(spec.anchor_pos.2 as f64),
        ],
        calls: Vec::new(),
        named: vec![("name".to_string(), Value::String(spec.mount_name))],
        positionals: Vec::new(),
        deferred_block: None,
        children: vec![CeChild::Spawn(shared_layout_root)],
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
    let mut controls = HashMap::new();
    for (kind, selector) in &spec.control_selectors {
        let control = world.find_component(shell_root, selector)?;
        controls.insert(*kind, control);
    }
    Some(PanelInstance {
        panel_kind: spec.panel_kind,
        editor_root,
        root: shell_root,
        slots,
        controls,
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

pub fn render_list_into_slot(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel: &PanelInstance,
    slot_kind: PanelSlotKind,
    spec: &ItemRendererSpec,
    items: &[UiItem],
    renderer: &mut DataRendererSystem,
) -> Result<ComponentId, String> {
    let slot = panel
        .slots
        .get(&slot_kind)
        .ok_or_else(|| format!("{:?} has no slot {:?}", panel.panel_kind, slot_kind))?;
    renderer.render_list(world, emit, *slot, spec, items)
}

pub fn render_detail_into_slot(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel: &PanelInstance,
    slot_kind: PanelSlotKind,
    spec: &DetailRendererSpec,
    detail: &UiDetailItem,
    renderer: &mut DataRendererSystem,
) -> Result<ComponentId, String> {
    let slot = panel
        .slots
        .get(&slot_kind)
        .ok_or_else(|| format!("{:?} has no slot {:?}", panel.panel_kind, slot_kind))?;
    renderer.render_detail(world, emit, *slot, spec, detail)
}

pub fn clear_slot_on_panel(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel: &PanelInstance,
    slot_kind: PanelSlotKind,
    renderer: &mut DataRendererSystem,
) {
    if let Some(slot) = panel.slots.get(&slot_kind) {
        renderer.clear_slot(world, emit, *slot);
    }
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
        let payload = world.children_of(component_id).iter().find_map(|&child| {
            if world.component_label(child) == Some(payload_name) {
                return world.get_component_by_id_as::<DataComponent>(child);
            }
            world
                .get_component_by_id_as::<OptionComponent>(child)
                .and_then(|_| {
                    world.children_of(child).iter().find_map(|&option_child| {
                        (world.component_label(option_child) == Some(payload_name))
                            .then(|| world.get_component_by_id_as::<DataComponent>(option_child))
                            .flatten()
                    })
                })
        });
        if let Some(payload) = payload {
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

pub fn data_text(data: &DataComponent, key: &str) -> Option<String> {
    match data.get(key) {
        Some(DataValue::Text(value)) => Some(value.clone()),
        _ => None,
    }
}

pub fn build_editor_panel_component_expr(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    asset_path: &str,
    export_name: &str,
    args: Vec<Value>,
    panel_kind: PanelKind,
    panel_kind_label: &str,
) -> Option<MaterializedCE> {
    let result = build_panel_shell_component_expr(
        world,
        emit,
        &PanelShellSpec {
            panel_kind,
            asset_path: asset_path.to_string(),
            export_name: export_name.to_string(),
            args,
            root_selector: String::new(),
            slot_selectors: HashMap::new(),
            control_selectors: HashMap::new(),
        },
    )
    .map_err(|error| {
        eprintln!("[InspectorSystemStopgapMmsAdapter] {panel_kind_label} render error: {error}");
    });
    result.ok()
}

pub fn build_placeholder_panel_component_expr(title_name: &str, title: &str) -> MaterializedCE {
    MaterializedCE {
        component_type: "T".to_string(),
        component_property_assignment_only: false,
        ctor_method: None,
        ctor_args: Vec::new(),
        calls: Vec::new(),
        named: vec![("name".to_string(), Value::String(title_name.to_string()))],
        positionals: Vec::new(),
        deferred_block: None,
        children: vec![CeChild::Spawn(MaterializedCE {
            component_type: "T".to_string(),
            component_property_assignment_only: false,
            ctor_method: None,
            ctor_args: Vec::new(),
            calls: Vec::new(),
            named: vec![(
                "name".to_string(),
                Value::String(format!("{title_name}_title")),
            )],
            positionals: Vec::new(),
            deferred_block: None,
            children: vec![CeChild::Spawn(MaterializedCE {
                component_type: "Text".to_string(),
                component_property_assignment_only: false,
                ctor_method: None,
                ctor_args: Vec::new(),
                calls: Vec::new(),
                named: vec![(
                    "name".to_string(),
                    Value::String(format!("{title_name}_label")),
                )],
                positionals: vec![Value::String(title.to_string())],
                deferred_block: None,
                children: Vec::new(),
            })],
        })],
    }
}

pub fn world_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/panels.mms")
}

pub fn icons_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/icons.mms")
}

pub fn world_panel_status_asset_path() -> &'static str {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/components/panel_items.mms"
    )
}

pub fn inspector_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/panels.mms")
}

pub fn inspector_details_asset_path() -> &'static str {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/components/inspector_details.mms"
    )
}

pub fn asset_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/panels.mms")
}

pub fn paint_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/panels.mms")
}

pub fn grid_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/panels.mms")
}

pub fn editor_settings_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/panels.mms")
}

pub fn pose_panel_asset_path() -> &'static str {
    concat!(env!("CARGO_MANIFEST_DIR"), "/assets/components/panels.mms")
}

/// Build the panel layout tree: all panel component expressions + mount.
/// Returns `(panel_mount_root, layout_root_id)` on success.
pub fn spawn_editor_panel_layout_tree(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    model: &WorldPanelModel,
    working_file_path: &Path,
    world_panel_pos: (f32, f32, f32),
    selected_panel_specs: &[EditorUIPanelSpec],
) -> Option<(ComponentId, ComponentId)> {
    let selected_panels: Vec<_> = selected_panel_specs.iter().map(|spec| spec.panel).collect();
    let world_panel_title_color = Value::Array(vec![
        Value::Number(0.90),
        Value::Number(1.00),
        Value::Number(0.92),
        Value::Number(1.0),
    ]);
    let world_panel_bg = Value::Array(vec![
        Value::Number(0.18),
        Value::Number(0.78),
        Value::Number(0.22),
        Value::Number(0.95),
    ]);
    let world_panel_item_bg = Value::Array(vec![
        Value::Number(0.92),
        Value::Number(0.97),
        Value::Number(0.92),
        Value::Number(1.0),
    ]);

    let asset_panel_title_color = world_panel_title_color.clone();
    let asset_panel_bg = world_panel_bg.clone();
    let asset_panel_item_bg = world_panel_item_bg.clone();

    let paint_panel_title_color = world_panel_title_color.clone();
    let paint_panel_bg = world_panel_bg.clone();
    let paint_panel_item_bg = world_panel_item_bg.clone();

    let working_file_path_str = working_file_path.to_string_lossy().to_string();

    let world_panel = if selected_panels.contains(&EditorPanel::World) {
        Some(build_editor_panel_component_expr(
            world,
            emit,
            world_panel_asset_path(),
            "world_panel",
            vec![
                Value::String(model.title.clone()),
                Value::Array(Vec::new()),
                world_panel_title_color.clone(),
                world_panel_bg.clone(),
                world_panel_item_bg.clone(),
                Value::String(working_file_path_str),
            ],
            PanelKind::World,
            "world panel",
        )?)
    } else {
        None
    };

    let asset_items_val = Value::Array(Vec::new());

    let asset_panel = if selected_panels.contains(&EditorPanel::Assets) {
        Some(build_editor_panel_component_expr(
            world,
            emit,
            asset_panel_asset_path(),
            "asset_panel",
            vec![
                Value::String("Assets".to_string()),
                asset_items_val,
                asset_panel_title_color.clone(),
                asset_panel_bg.clone(),
                asset_panel_item_bg.clone(),
            ],
            PanelKind::Assets,
            "asset panel",
        )?)
    } else {
        None
    };

    let paint_panel = if selected_panels.contains(&EditorPanel::Paint) {
        Some(build_editor_panel_component_expr(
            world,
            emit,
            paint_panel_asset_path(),
            "paint_panel",
            vec![
                Value::String("Paint".to_string()),
                paint_panel_title_color.clone(),
                paint_panel_bg.clone(),
                paint_panel_item_bg.clone(),
            ],
            PanelKind::Paint,
            "paint panel",
        )?)
    } else {
        None
    };

    let color_panel = if selected_panels.contains(&EditorPanel::Color) {
        Some(build_editor_panel_component_expr(
            world,
            emit,
            paint_panel_asset_path(),
            "color_panel",
            vec![
                Value::String("Color".to_string()),
                paint_panel_title_color.clone(),
                paint_panel_bg.clone(),
            ],
            PanelKind::Color,
            "color panel",
        )?)
    } else {
        None
    };

    let grid_panel = if selected_panels.contains(&EditorPanel::Grid) {
        Some(build_editor_panel_component_expr(
            world,
            emit,
            grid_panel_asset_path(),
            "grid_panel",
            vec![
                Value::String("Grids".to_string()),
                Value::Array(Vec::new()),
                world_panel_title_color.clone(),
                world_panel_bg.clone(),
                world_panel_item_bg.clone(),
            ],
            PanelKind::Grid,
            "grid panel",
        )?)
    } else {
        None
    };

    let pose_panel = if selected_panels.contains(&EditorPanel::Pose) {
        Some(build_editor_panel_component_expr(
            world,
            emit,
            pose_panel_asset_path(),
            "pose_capture_panel",
            vec![
                Value::String("Poses".to_string()),
                world_panel_title_color.clone(),
                world_panel_bg.clone(),
            ],
            PanelKind::Pose,
            "pose capture panel",
        )?)
    } else {
        None
    };

    let editor_settings_panel = if selected_panels.contains(&EditorPanel::Settings) {
        let config = selected_panel_specs
            .iter()
            .find(|spec| spec.panel == EditorPanel::Settings)
            .and_then(EditorUIPanelSpec::settings_config)
            .unwrap_or_default();
        Some(build_editor_panel_component_expr(
            world,
            emit,
            editor_settings_panel_asset_path(),
            "editor_settings_panel",
            vec![
                Value::String("Editor".to_string()),
                world_panel_title_color.clone(),
                world_panel_bg.clone(),
                Value::Map(std::collections::HashMap::from([
                    ("show_armature".into(), Value::Bool(config.show_armature)),
                    ("show_bounds".into(), Value::Bool(config.show_bounds)),
                    ("show_colliders".into(), Value::Bool(config.show_colliders)),
                    (
                        "show_gltf_colliders".into(),
                        Value::Bool(config.show_gltf_colliders),
                    ),
                ])),
            ],
            PanelKind::Settings,
            "editor settings panel",
        )?)
    } else {
        None
    };

    let anchor_pos = world_panel_pos;

    let total_height_gu = 60.5f64
        .max(60.5)
        .max(60.5)
        .max(32.0)
        .max(60.5)
        .max(60.5)
        .max(11.5)
        * 2.0
        + 2.0
        + (0.5 * 2.0);

    let decorate =
        |panel: Option<MaterializedCE>| panel.map(|panel| decorate_panel_root_ce(panel, 2.0));
    let world_panel = decorate(world_panel);
    let paint_panel = decorate(paint_panel);
    let color_panel = decorate(color_panel);
    let asset_panel = decorate(asset_panel);
    let grid_panel = decorate(grid_panel);
    let pose_panel = decorate(pose_panel);
    let editor_settings_panel = decorate(editor_settings_panel);

    let mut children = Vec::new();
    children.extend(editor_settings_panel);
    children.extend(paint_panel);
    children.extend(color_panel);
    children.extend(grid_panel);
    children.extend(pose_panel);
    children.extend(asset_panel);
    children.extend(world_panel);

    let (panel_mount_root, layout_root_id) = match spawn_panel_layout_mount(
        world,
        emit,
        PanelLayoutMountSpec {
            anchor_pos,
            total_height_gu,
            available_width_gu: 200000.0,
            text_scale: 0.08,
            mount_name: PANEL_LAYOUT_MOUNT_NAME.to_string(),
            layout_name: PANEL_LAYOUT_ROOT_NAME.to_string(),
            children,
        },
    ) {
        Ok(ids) => ids,
        Err(error) => {
            eprintln!("[InspectorSystemStopgapMmsAdapter] panel layout spawn error: {error}");
            return None;
        }
    };

    Some((panel_mount_root, layout_root_id))
}

#[cfg(test)]
mod tests {
    use super::{PanelLayoutMountSpec, build_panel_layout_mount_ce};
    use crate::scripting::object::{CeChild, Value};

    #[test]
    fn build_panel_layout_mount_ce_places_layout_root_directly_under_mount() {
        let mount = build_panel_layout_mount_ce(PanelLayoutMountSpec {
            anchor_pos: (1.0, 2.0, 3.0),
            total_height_gu: 10.0,
            available_width_gu: 20.0,
            text_scale: 0.08,
            mount_name: "panel_mount".to_string(),
            layout_name: "panel_layout".to_string(),
            children: Vec::new(),
        });

        assert_eq!(mount.component_type, "T");
        assert_eq!(mount.children.len(), 1);
        let CeChild::Spawn(layout_root) = &mount.children[0] else {
            panic!("expected spawned layout root");
        };
        assert_eq!(layout_root.component_type, "LayoutRoot");
        assert_eq!(
            layout_root.named,
            vec![(
                "name".to_string(),
                Value::String("panel_layout".to_string())
            )]
        );
    }
}
