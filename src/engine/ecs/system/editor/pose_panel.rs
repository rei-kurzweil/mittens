use std::sync::{Arc, LazyLock, Mutex};

use crate::engine::ecs::component::{
    AlignItems, ColorComponent, DataComponent, DataValue, Display, EdgeInsets, OptionComponent,
    PoseCaptureComponent, PoseCaptureLibraryComponent, PoseCapturePoseComponent,
    PoseCaptureReconciliationState, RaycastableComponent, SizeDimension, StyleComponent,
    TextComponent, TextInputComponent, TransformComponent, is_valid_pose_asset_name,
    save_pose_library_asset,
};
use crate::engine::ecs::system::data_renderer_system::{
    DataRendererSystem, ItemRendererSpec, RendererSpec, UiItem, UiItemKind,
};
use crate::engine::ecs::system::editor::context::EditorContextState;
use crate::engine::ecs::system::editor::world_panel::PANEL_CONTENT_SLOT_SELECTOR;
use crate::engine::ecs::system::layout::AUTO_TEXT_LIFT_Z;
use crate::engine::ecs::system::panel_system::{data_text, is_descendant_or_self};
use crate::engine::ecs::system::pose_capture_system::{
    ensure_pose_capture_for_gltf_selection, gltf_for_visual_selection, pose_assets_root,
    reconcile_pose_captures, resolve_pose_apply_target, validate_pose_apply,
};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};

pub const POSE_PANEL_ROOT_SELECTOR: &str = "#pose_capture_panel_root";
pub const POSE_PANEL_SELECTION_NAME: &str = "pose_capture_selection";
pub const POSE_PANEL_PAYLOAD_NAME: &str = "pose_panel_payload";
pub const POSE_PANEL_STATUS_VALUE_SELECTOR: &str = "#pose_panel_status_value";
const POSE_PANEL_BASE_FONT_SIZE_WU: f32 = 0.08;
const POSE_PANEL_ACTION_FONT_SIZE_GU: f32 = 0.9;
const POSE_PANEL_LIBRARY_FONT_SIZE_GU: f32 = 1.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PosePanelActionKind {
    NewLibrary,
    Rename,
    RenamePose,
    Capture,
    Reset,
    Save,
    Select,
    Apply,
}

impl PosePanelActionKind {
    fn label(self) -> &'static str {
        match self {
            Self::NewLibrary => "NewLibrary",
            Self::Rename => "Rename",
            Self::RenamePose => "RenamePose",
            Self::Capture => "Capture",
            Self::Reset => "Reset",
            Self::Save => "Save",
            Self::Select => "Select",
            Self::Apply => "Apply",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PosePanelModel {
    pub sections: Vec<PosePanelSection>,
}

#[derive(Debug, Clone)]
pub struct PosePanelSection {
    pub target: ComponentId,
    pub library: ComponentId,
    pub label: String,
    pub asset_name_draft: String,
    pub poses: Vec<PosePanelRow>,
}

#[derive(Debug, Clone)]
pub struct PosePanelRow {
    pub target: ComponentId,
    pub library: ComponentId,
    pub pose: ComponentId,
    pub label: String,
}

fn pose_panel_items(model: &PosePanelModel) -> Vec<UiItem> {
    if model.sections.is_empty() {
        return vec![UiItem {
            key: "pose_new_library".to_string(),
            kind: UiItemKind::Info,
            label: "New Pose Library".to_string(),
            selected: false,
            target_ref: None,
        }];
    }
    let mut items = Vec::new();
    for section in &model.sections {
        items.push(UiItem {
            key: "pose_library_header".to_string(),
            kind: UiItemKind::Info,
            label: section.asset_name_draft.clone(),
            selected: false,
            target_ref: Some(section.library),
        });
        items.extend(section.poses.iter().map(|row| UiItem {
            key: "pose_row".to_string(),
            kind: UiItemKind::Component,
            label: row.label.clone(),
            selected: false,
            target_ref: Some(row.pose),
        }));
    }
    items
}

fn pose_target_for_library(world: &World, library: ComponentId) -> Option<ComponentId> {
    world.get_component_by_id_as::<PoseCaptureLibraryComponent>(library)?;
    let target = world.parent_of(library)?;
    world.get_component_by_id_as::<PoseCaptureComponent>(target)?;
    Some(target)
}

fn pose_library_for_pose(world: &World, pose: ComponentId) -> Option<ComponentId> {
    let library = world.parent_of(pose)?;
    world.get_component_by_id_as::<PoseCaptureLibraryComponent>(library)?;
    Some(library)
}

fn add_payload(
    world: &mut World,
    parent: ComponentId,
    action: PosePanelActionKind,
    target: Option<ComponentId>,
    library: Option<ComponentId>,
    pose: Option<ComponentId>,
) {
    let mut data =
        DataComponent::new().with_entry("action", DataValue::Text(action.label().to_string()));
    if let Some(target) = target {
        data.insert("target_component", DataValue::Component(target));
    }
    if let Some(library) = library {
        data.insert("library", DataValue::Component(library));
    }
    if let Some(pose) = pose {
        data.insert("pose", DataValue::Component(pose));
    }
    let payload = world.add_component_boxed_named(POSE_PANEL_PAYLOAD_NAME, Box::new(data));
    let _ = world.add_child(parent, payload);
}

fn spawn_text(world: &mut World, parent: ComponentId, name: &str, label: &str, color: [f32; 4]) {
    let text_root = world.add_component_boxed_named(
        format!("{name}_root"),
        Box::new(TransformComponent::new().with_position(0.0, 0.0, AUTO_TEXT_LIFT_Z)),
    );
    let text = world.add_component_boxed_named(
        name,
        Box::new(TextComponent::new(label).with_font_size(POSE_PANEL_BASE_FONT_SIZE_WU)),
    );
    let text_color = world.add_component_boxed_named(
        format!("{name}_color"),
        Box::new(ColorComponent::rgba(color[0], color[1], color[2], color[3])),
    );
    let _ = world.add_child(parent, text_root);
    let _ = world.add_child(text_root, text);
    let _ = world.add_child(text, text_color);
}

fn spawn_action_button(
    world: &mut World,
    parent: ComponentId,
    name: &str,
    label: &str,
    width: f32,
    action: PosePanelActionKind,
    target: ComponentId,
    library: ComponentId,
    pose: Option<ComponentId>,
) -> ComponentId {
    let root = world.add_component_boxed_named(name, Box::new(TransformComponent::new()));
    let raycastable = world.add_component_boxed_named(
        format!("{name}_raycastable"),
        Box::new(RaycastableComponent::click_only()),
    );
    let style = world.add_component_boxed_named(
        format!("{name}_style"),
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::InlineBlock);
            style.width = if width > 0.0 {
                SizeDimension::GlyphUnits(width)
            } else {
                SizeDimension::Auto
            };
            style.height = SizeDimension::GlyphUnits(2.2);
            style.margin = EdgeInsets::axes(0.2, 0.15);
            style.padding = EdgeInsets::axes(0.25, 0.35);
            style.background_color = Some([0.10, 0.55, 0.18, 1.0]);
            style.background_z = Some(0.001);
            style.color = Some([0.75, 1.0, 0.45, 1.0]);
            style.font_size = SizeDimension::GlyphUnits(POSE_PANEL_ACTION_FONT_SIZE_GU);
            style
        }),
    );
    let _ = world.add_child(parent, root);
    let _ = world.add_child(root, raycastable);
    let _ = world.add_child(root, style);
    spawn_text(
        world,
        root,
        &format!("{name}_text"),
        label,
        [0.75, 1.0, 0.45, 1.0],
    );
    add_payload(world, root, action, Some(target), Some(library), pose);
    root
}

fn spawn_library_header(
    world: &mut World,
    asset_name_draft: &str,
    target: ComponentId,
    library: ComponentId,
) -> ComponentId {
    let root =
        world.add_component_boxed_named("pose_library_header", Box::new(TransformComponent::new()));
    let style = world.add_component_boxed_named(
        "pose_library_header_style",
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::Flex);
            style.width = SizeDimension::Percent(100.0);
            style.height = SizeDimension::GlyphUnits(4.2);
            style.margin = EdgeInsets::axes(0.0, 0.35);
            style.padding = EdgeInsets::axes(0.25, 0.2);
            style.align_items = AlignItems::Center;
            style.column_gap = 0.35;
            style.background_color = Some([0.16, 0.20, 0.18, 1.0]);
            style.background_z = Some(0.001);
            style.color = Some([0.95, 0.98, 0.92, 1.0]);
            style.font_size = SizeDimension::GlyphUnits(1.0);
            style
        }),
    );
    let name_root = world.add_component_boxed_named(
        "pose_library_name_wrap",
        Box::new(TransformComponent::new()),
    );
    let name_root_style = world.add_component_boxed_named(
        "pose_library_name_wrap_style",
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::InlineBlock);
            style.width = SizeDimension::GlyphUnits(10.5);
            style.height = SizeDimension::GlyphUnits(3.0);
            style.padding = EdgeInsets::axes(0.25, 0.35);
            style.background_color = Some([0.94, 0.98, 0.92, 1.0]);
            style.background_z = Some(0.001);
            style.color = Some([0.02, 0.08, 0.03, 1.0]);
            style.font_size = SizeDimension::GlyphUnits(POSE_PANEL_LIBRARY_FONT_SIZE_GU);
            style
        }),
    );
    let name_input = world.add_component_boxed_named(
        "pose_library_name_input",
        Box::new(TextInputComponent::new(asset_name_draft)),
    );
    let name_style = world.add_component_boxed_named(
        "pose_library_name_input_style",
        Box::new({
            let mut style = StyleComponent::new();
            style.color = Some([0.02, 0.08, 0.03, 1.0]);
            style.font_size = SizeDimension::GlyphUnits(POSE_PANEL_LIBRARY_FONT_SIZE_GU);
            style
        }),
    );
    let _ = world.add_child(root, style);
    let _ = world.add_child(root, name_root);
    let _ = world.add_child(name_root, name_root_style);
    let _ = world.add_child(name_root, name_input);
    let _ = world.add_child(name_input, name_style);
    add_payload(
        world,
        name_input,
        PosePanelActionKind::Rename,
        Some(target),
        Some(library),
        None,
    );
    spawn_action_button(
        world,
        root,
        "pose_capture_action",
        "Capture",
        0.0,
        PosePanelActionKind::Capture,
        target,
        library,
        None,
    );
    spawn_action_button(
        world,
        root,
        "pose_reset_action",
        "Reset",
        0.0,
        PosePanelActionKind::Reset,
        target,
        library,
        None,
    );
    spawn_action_button(
        world,
        root,
        "pose_save_action",
        "Save",
        0.0,
        PosePanelActionKind::Save,
        target,
        library,
        None,
    );
    root
}

fn spawn_pose_row(
    world: &mut World,
    label: &str,
    target: ComponentId,
    library: ComponentId,
    pose: ComponentId,
) -> ComponentId {
    let root = world.add_component_boxed_named("pose_row", Box::new(TransformComponent::new()));
    let root_style = world.add_component_boxed_named(
        "pose_row_container_style",
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::Flex);
            style.width = SizeDimension::Percent(100.0);
            style.height = SizeDimension::GlyphUnits(3.2);
            style.align_items = AlignItems::Center;
            style.column_gap = 0.35;
            style
        }),
    );
    let option =
        world.add_component_boxed_named("pose_row_option", Box::new(OptionComponent::new()));
    let body =
        world.add_component_boxed_named("pose_row_body", Box::new(TransformComponent::new()));
    let body_raycastable = world.add_component_boxed_named(
        "pose_row_body_raycastable",
        Box::new(RaycastableComponent::click_only()),
    );
    let body_style = world.add_component_boxed_named(
        "pose_row_style",
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::InlineBlock);
            style.width = SizeDimension::GlyphUnits(20.0);
            style.height = SizeDimension::GlyphUnits(2.5);
            style.margin = EdgeInsets::axes(0.25, 0.20);
            style.padding = EdgeInsets::axes(0.55, 0.45);
            style.font_size = SizeDimension::GlyphUnits(1.0);
            style.background_color = Some([0.92, 0.97, 0.92, 1.0]);
            style.background_z = Some(0.001);
            style.color = Some([0.0, 0.0, 0.0, 1.0]);
            style
        }),
    );
    let _ = world.add_child(root, root_style);
    let _ = world.add_child(root, body);
    let _ = world.add_child(body, option);
    let _ = world.add_child(body, body_raycastable);
    let _ = world.add_child(body, body_style);
    let name_input = world.add_component_boxed_named(
        "pose_row_name_input",
        Box::new(TextInputComponent::new(label)),
    );
    let name_style = world.add_component_boxed_named(
        "pose_row_name_input_style",
        Box::new({
            let mut style = StyleComponent::new();
            style.color = Some([0.0, 0.0, 0.0, 1.0]);
            style.font_size = SizeDimension::GlyphUnits(1.0);
            style
        }),
    );
    let _ = world.add_child(body, name_input);
    let _ = world.add_child(name_input, name_style);
    add_payload(
        world,
        name_input,
        PosePanelActionKind::RenamePose,
        Some(target),
        Some(library),
        Some(pose),
    );
    add_payload(
        world,
        body,
        PosePanelActionKind::Select,
        Some(target),
        Some(library),
        Some(pose),
    );
    spawn_action_button(
        world,
        root,
        "pose_apply_action",
        "Apply",
        5.0,
        PosePanelActionKind::Apply,
        target,
        library,
        Some(pose),
    );
    root
}

fn spawn_new_library_button(world: &mut World) -> ComponentId {
    let root = world.add_component_boxed_named(
        "pose_new_library_action",
        Box::new(TransformComponent::new()),
    );
    let raycastable = world.add_component_boxed_named(
        "pose_new_library_action_raycastable",
        Box::new(RaycastableComponent::click_only()),
    );
    let style = world.add_component_boxed_named(
        "pose_new_library_action_style",
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::Block);
            style.width = SizeDimension::Percent(100.0);
            style.height = SizeDimension::GlyphUnits(2.8);
            style.margin = EdgeInsets::axes(0.0, 0.35);
            style.padding = EdgeInsets::axes(0.45, 0.45);
            style.background_color = Some([0.10, 0.55, 0.18, 1.0]);
            style.background_z = Some(0.001);
            style.color = Some([0.75, 1.0, 0.45, 1.0]);
            style
        }),
    );
    let _ = world.add_child(root, raycastable);
    let _ = world.add_child(root, style);
    spawn_text(
        world,
        root,
        "pose_new_library_action_text",
        "New Pose Library",
        [0.75, 1.0, 0.45, 1.0],
    );
    add_payload(
        world,
        root,
        PosePanelActionKind::NewLibrary,
        None,
        None,
        None,
    );
    root
}

fn pose_panel_item_render_fn(
    world: &mut World,
    _emit: &mut dyn SignalEmitter,
    item: &UiItem,
) -> Result<ComponentId, String> {
    if item.key == "pose_new_library" {
        return Ok(spawn_new_library_button(world));
    }
    match item.kind {
        UiItemKind::Info => {
            let library = item
                .target_ref
                .ok_or_else(|| "pose library header missing library".to_string())?;
            let target = pose_target_for_library(world, library)
                .ok_or_else(|| "pose library header missing target".to_string())?;
            Ok(spawn_library_header(world, &item.label, target, library))
        }
        UiItemKind::Component => {
            let pose = item
                .target_ref
                .ok_or_else(|| "pose panel row missing pose".to_string())?;
            let library = pose_library_for_pose(world, pose)
                .ok_or_else(|| "pose panel row missing library".to_string())?;
            let target = pose_target_for_library(world, library)
                .ok_or_else(|| "pose panel row missing target".to_string())?;
            Ok(spawn_pose_row(world, &item.label, target, library, pose))
        }
        kind => Err(format!("unsupported pose panel item kind: {kind:?}")),
    }
}

static POSE_PANEL_ITEM_SPEC: LazyLock<ItemRendererSpec> = LazyLock::new(|| RendererSpec::Rust {
    render_fn: Box::new(pose_panel_item_render_fn),
});

pub fn build_pose_panel_model(world: &World) -> PosePanelModel {
    let mut sections = Vec::new();
    for id in world.all_components() {
        let Some(pc) = world.get_component_by_id_as::<PoseCaptureComponent>(id) else {
            continue;
        };
        let Some(library) = world.children_of(id).iter().copied().find(|&child| {
            world
                .get_component_by_id_as::<PoseCaptureLibraryComponent>(child)
                .is_some()
        }) else {
            continue;
        };
        let label = pc.label.clone().unwrap_or_else(|| {
            world
                .component_label(id)
                .map(str::to_string)
                .filter(|label| !label.is_empty())
                .unwrap_or_else(|| format!("Target {id:?}"))
        });
        let poses = world
            .children_of(library)
            .iter()
            .filter_map(|&pose| {
                world
                    .get_component_by_id_as::<PoseCapturePoseComponent>(pose)
                    .map(|component| PosePanelRow {
                        target: id,
                        library,
                        pose,
                        label: component.name.clone(),
                    })
            })
            .collect();
        sections.push(PosePanelSection {
            target: id,
            library,
            label,
            asset_name_draft: pc.asset_name_draft().to_string(),
            poses,
        });
    }
    PosePanelModel { sections }
}

pub fn rerender_pose_panel(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_mount_root: ComponentId,
    data_renderer: &mut DataRendererSystem,
) {
    let Some(panel_root) = world.find_component(panel_mount_root, POSE_PANEL_ROOT_SELECTOR) else {
        return;
    };
    let Some(content_slot) = world.find_component(panel_root, PANEL_CONTENT_SLOT_SELECTOR) else {
        return;
    };
    let _ = reconcile_pose_captures(world, emit);
    let model = build_pose_panel_model(world);
    let items = pose_panel_items(&model);
    if let Err(error) =
        data_renderer.render_list(world, emit, content_slot, &POSE_PANEL_ITEM_SPEC, &items)
    {
        eprintln!("[InspectorSystem] pose panel content render error: {error}");
    }
    let status = pose_panel_runtime_status(world, &model);
    set_pose_panel_status(world, panel_root, status);
}

fn pose_panel_runtime_status(world: &World, model: &PosePanelModel) -> String {
    if model.sections.is_empty() {
        return "select a glTF and create a pose library".to_string();
    }
    for section in &model.sections {
        let Some(capture) = world.get_component_by_id_as::<PoseCaptureComponent>(section.target)
        else {
            continue;
        };
        match &capture.runtime.state {
            PoseCaptureReconciliationState::LoadFailed { error, .. } => {
                return format!("load failed: {error}");
            }
            PoseCaptureReconciliationState::Unsaved => {
                return format!("{} has unsaved changes", capture.asset_name_draft());
            }
            PoseCaptureReconciliationState::Hydrated => {
                return format!("loaded {}", capture.asset_name_draft());
            }
            PoseCaptureReconciliationState::New => {
                return format!("new library {}", capture.asset_name_draft());
            }
            PoseCaptureReconciliationState::Authored
            | PoseCaptureReconciliationState::Unreconciled => {}
        }
    }
    "idle".to_string()
}

fn set_pose_panel_status(world: &mut World, panel_root: ComponentId, status: impl Into<String>) {
    if let Some(text_id) = world.find_component(panel_root, POSE_PANEL_STATUS_VALUE_SELECTOR)
        && let Some(text) = world.get_component_by_id_as_mut::<TextComponent>(text_id)
    {
        text.text = status.into();
    }
}

pub fn activate_pose_panel_for_selection(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    selected: Option<ComponentId>,
    data_renderer: &mut DataRendererSystem,
) -> bool {
    let Some(selected) = selected else {
        return false;
    };
    if gltf_for_visual_selection(world, selected).is_none() {
        return false;
    }

    let result = ensure_pose_capture_for_gltf_selection(world, emit, selected);
    rerender_pose_panel(world, emit, panel_query_root, data_renderer);
    if let Err(error) = result
        && let Some(panel_root) = world.find_component(panel_query_root, POSE_PANEL_ROOT_SELECTOR)
    {
        set_pose_panel_status(world, panel_root, error);
    }
    true
}

pub fn handle_pose_panel_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    clicked_node: ComponentId,
    editor_context_state: &Arc<Mutex<EditorContextState>>,
    data_renderer: &mut DataRendererSystem,
) -> bool {
    let Some(panel_root) = world.find_component(panel_query_root, POSE_PANEL_ROOT_SELECTOR) else {
        return false;
    };
    if !is_descendant_or_self(world, panel_root, clicked_node) {
        return false;
    }

    let mut current = Some(clicked_node);
    while let Some(curr_id) = current {
        let payload = world
            .children_of(curr_id)
            .iter()
            .copied()
            .find(|&child| world.component_label(child) == Some(POSE_PANEL_PAYLOAD_NAME));
        if let Some(payload) = payload
            && let Some(data) = world.get_component_by_id_as::<DataComponent>(payload)
        {
            let action = data_text(data, "action").unwrap_or_default();
            let target = data.get_component("target_component");
            let library = data.get_component("library");
            let pose = data.get_component("pose");
            match action.as_str() {
                "NewLibrary" => {
                    let selected = editor_context_state
                        .lock()
                        .expect("editor context state mutex poisoned")
                        .selected_component;
                    if !activate_pose_panel_for_selection(
                        world,
                        emit,
                        panel_query_root,
                        selected,
                        data_renderer,
                    ) {
                        set_pose_panel_status(
                            world,
                            panel_root,
                            "new library failed: selection is not part of a glTF",
                        );
                    }
                    return true;
                }
                "Select" => return true,
                "Capture" => {
                    if let Some(target) = target {
                        set_pose_panel_status(world, panel_root, "capturing pose...");
                        emit.push_intent_now(
                            panel_root,
                            IntentValue::PoseCapture {
                                target,
                                pose_name: None,
                            },
                        );
                        return true;
                    }
                }
                "Reset" => {
                    if let Some(target) = target {
                        set_pose_panel_status(
                            world,
                            panel_root,
                            "resetting to imported rest pose...",
                        );
                        emit.push_intent_now(target, IntentValue::PoseReset { target });
                        return true;
                    }
                }
                "Save" => {
                    if let (Some(target), Some(library)) = (target, library) {
                        let draft = world
                            .get_component_by_id_as::<PoseCaptureComponent>(target)
                            .map(|capture| capture.asset_name_draft().to_string())
                            .unwrap_or_default();
                        let status = if draft.is_empty() {
                            "save failed: PoseCapture asset_name is required".to_string()
                        } else if !is_valid_pose_asset_name(&draft) {
                            "save failed: asset_name may contain only ASCII letters, digits, '_' or '-'"
                                    .to_string()
                        } else {
                            let needs_warning = world
                                .get_component_by_id_as::<PoseCaptureComponent>(target)
                                .is_some_and(|capture| {
                                    matches!(
                                        &capture.runtime.state,
                                        PoseCaptureReconciliationState::LoadFailed {
                                            asset_name,
                                            overwrite_warning_issued: false,
                                            ..
                                        } if asset_name == &draft
                                    )
                                });
                            if needs_warning {
                                if let Some(capture) =
                                    world.get_component_by_id_as_mut::<PoseCaptureComponent>(target)
                                    && let PoseCaptureReconciliationState::LoadFailed {
                                        overwrite_warning_issued,
                                        ..
                                    } = &mut capture.runtime.state
                                {
                                    *overwrite_warning_issued = true;
                                }
                                format!(
                                    "warning: {} failed to load; Save again to replace it",
                                    draft
                                )
                            } else {
                                let directory = pose_assets_root().join(&draft);
                                let manifest = directory.join("library.mms");
                                match save_pose_library_asset(world, library, &manifest) {
                                    Ok(paths) => {
                                        if let Some(capture) = world
                                            .get_component_by_id_as_mut::<PoseCaptureComponent>(
                                                target,
                                            )
                                        {
                                            capture.asset_name = Some(draft.clone());
                                            capture.runtime.asset_name_draft = Some(draft.clone());
                                            capture.runtime.state =
                                                PoseCaptureReconciliationState::Hydrated;
                                        }
                                        format!(
                                            "saved {} poses to assets/components/poses/{draft}/",
                                            paths.len()
                                        )
                                    }
                                    Err(error) => format!("save failed: {error}"),
                                }
                            }
                        };
                        set_pose_panel_status(world, panel_root, status);
                        return true;
                    }
                }
                "Apply" => {
                    if let Some(pose) = pose {
                        let selected = editor_context_state
                            .lock()
                            .expect("editor context state mutex poisoned")
                            .selected_component;
                        let result =
                            resolve_pose_apply_target(world, selected, pose).and_then(|gltf| {
                                validate_pose_apply(world, gltf, pose)?;
                                Ok(gltf)
                            });
                        match result {
                            Ok(gltf) => {
                                set_pose_panel_status(world, panel_root, "applying pose...");
                                emit.push_intent_now(
                                    gltf,
                                    IntentValue::PoseApply { target: gltf, pose },
                                );
                            }
                            Err(error) => set_pose_panel_status(
                                world,
                                panel_root,
                                format!("apply failed: {error}"),
                            ),
                        }
                        return true;
                    }
                }
                _ => {}
            }
        }
        current = world.parent_of(curr_id);
    }
    false
}

pub fn handle_pose_panel_text_input_changed(
    world: &mut World,
    panel_query_root: ComponentId,
    component_id: ComponentId,
    text: &str,
) -> bool {
    let Some(panel_root) = world.find_component(panel_query_root, POSE_PANEL_ROOT_SELECTOR) else {
        return false;
    };
    if !is_descendant_or_self(world, panel_root, component_id) {
        return false;
    }
    let payload = world
        .children_of(component_id)
        .iter()
        .copied()
        .find(|&child| world.component_label(child) == Some(POSE_PANEL_PAYLOAD_NAME));
    let Some(payload) = payload else {
        return false;
    };
    let Some(data) = world.get_component_by_id_as::<DataComponent>(payload) else {
        return false;
    };
    let action = data_text(data, "action").unwrap_or_default();
    let Some(target) = data.get_component("target_component") else {
        return false;
    };
    match action.as_str() {
        "Rename" => {
            let valid = world
                .get_component_by_id_as_mut::<PoseCaptureComponent>(target)
                .is_some_and(|capture| capture.set_asset_name_draft(text));
            if valid {
                set_pose_panel_status(
                    world,
                    panel_root,
                    format!("renamed in memory; next Save writes to {text}"),
                );
            } else {
                set_pose_panel_status(
                    world,
                    panel_root,
                    "invalid name: use ASCII letters, digits, '_' or '-'",
                );
            }
        }
        "RenamePose" => {
            let Some(pose) = data.get_component("pose") else {
                return false;
            };
            if text.trim().is_empty() {
                set_pose_panel_status(world, panel_root, "pose name cannot be empty");
                return true;
            }
            let Some(pose_component) =
                world.get_component_by_id_as_mut::<PoseCapturePoseComponent>(pose)
            else {
                return false;
            };
            pose_component.name = text.to_string();
            if let Some(capture) = world.get_component_by_id_as_mut::<PoseCaptureComponent>(target)
            {
                capture.mark_unsaved();
            }
            set_pose_panel_status(
                world,
                panel_root,
                "pose renamed; Save renames its asset file",
            );
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{LayoutComponent, PoseBoneEntry, PoseTargetRef};
    use crate::engine::ecs::system::layout::LayoutSystem;
    use crate::engine::ecs::{EventSignal, IntentSignal};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestEmitter {
        intents: Vec<IntentValue>,
        scopes: Vec<ComponentId>,
    }

    impl SignalEmitter for TestEmitter {
        fn push_event(&mut self, _scope: ComponentId, _event: EventSignal) {}

        fn push_intent(&mut self, scope: ComponentId, intent: IntentSignal) {
            self.scopes.push(scope);
            self.intents.push(intent.value);
        }
    }

    fn emitted_translation(emitter: &TestEmitter, component: ComponentId) -> [f32; 3] {
        emitter
            .intents
            .iter()
            .find_map(|intent| match intent {
                IntentValue::UpdateTransform {
                    component_ids,
                    translation,
                    ..
                } if component_ids == &vec![component] => Some(*translation),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn model_and_renderer_include_library_actions_and_explicit_apply() {
        let mut world = World::default();
        let target = world.add_component_boxed_named(
            "pose_target",
            Box::new(
                PoseCaptureComponent::new()
                    .with_label("Avatar")
                    .with_asset_name("avatar"),
            ),
        );
        let library = world.add_component(PoseCaptureLibraryComponent::new(PoseTargetRef::Query(
            "#avatar".into(),
        )));
        let pose = world.add_component(PoseCapturePoseComponent::new(
            "Neutral",
            PoseTargetRef::Query("#avatar".into()),
            Vec::new(),
        ));
        let _ = world.add_child(target, library);
        let _ = world.add_child(library, pose);

        let model = build_pose_panel_model(&world);
        assert_eq!(model.sections[0].library, library);
        let mut emit = TestEmitter {
            intents: Vec::new(),
            scopes: Vec::new(),
        };
        let header =
            pose_panel_item_render_fn(&mut world, &mut emit, &pose_panel_items(&model)[0]).unwrap();
        assert!(
            world
                .find_component(header, "#pose_capture_action")
                .is_some()
        );
        assert!(world.find_component(header, "#pose_reset_action").is_some());
        assert!(world.find_component(header, "#pose_save_action").is_some());
        let name_input = world
            .find_component(header, "#pose_library_name_input")
            .unwrap();
        let name_wrap = world
            .find_component(header, "#pose_library_name_wrap")
            .unwrap();
        assert_eq!(
            world.parent_of(name_input),
            Some(name_wrap),
            "the text input must live inside a styled Transform so flex layout can position it"
        );
        let name_wrap_style = world
            .find_component(name_wrap, "#pose_library_name_wrap_style")
            .and_then(|id| world.get_component_by_id_as::<StyleComponent>(id))
            .unwrap();
        assert_eq!(name_wrap_style.width, SizeDimension::GlyphUnits(10.5));
        assert_eq!(
            world
                .get_component_by_id_as::<TextInputComponent>(name_input)
                .unwrap()
                .text,
            "avatar"
        );
        let name_style = world
            .find_component(name_input, "#pose_library_name_input_style")
            .and_then(|id| world.get_component_by_id_as::<StyleComponent>(id))
            .unwrap();
        assert_eq!(
            name_style.font_size,
            SizeDimension::GlyphUnits(POSE_PANEL_LIBRARY_FONT_SIZE_GU),
            "library names should use the compact 1.2-GU size"
        );
        let capture_style = world
            .find_component(header, "#pose_capture_action_style")
            .and_then(|id| world.get_component_by_id_as::<StyleComponent>(id))
            .unwrap();
        assert_eq!(capture_style.width, SizeDimension::Auto);
        let save_style = world
            .find_component(header, "#pose_save_action_style")
            .and_then(|id| world.get_component_by_id_as::<StyleComponent>(id))
            .unwrap();
        assert_eq!(save_style.width, SizeDimension::Auto);
        let header_style = world
            .find_component(header, "#pose_library_header_style")
            .and_then(|id| world.get_component_by_id_as::<StyleComponent>(id))
            .unwrap();
        assert_eq!(header_style.display, Some(Display::Flex));
        assert_eq!(header_style.height, SizeDimension::GlyphUnits(4.2));
        let layout = world.add_component(LayoutComponent::new(29.5).with_height(10.0));
        world.add_child(layout, header).unwrap();
        LayoutSystem::new().tick(&mut world, &mut emit);
        let capture = world
            .find_component(header, "#pose_capture_action")
            .unwrap();
        let reset = world.find_component(header, "#pose_reset_action").unwrap();
        let save = world.find_component(header, "#pose_save_action").unwrap();
        let name_x = emitted_translation(&emit, name_wrap)[0];
        let capture_x = emitted_translation(&emit, capture)[0];
        let reset_x = emitted_translation(&emit, reset)[0];
        let save_x = emitted_translation(&emit, save)[0];
        assert!(
            name_x < capture_x && capture_x < reset_x && reset_x < save_x,
            "header layout must place name, Capture, Reset, and Save in horizontal order"
        );

        let row =
            pose_panel_item_render_fn(&mut world, &mut emit, &pose_panel_items(&model)[1]).unwrap();
        assert!(
            world
                .get_component_by_id_as::<OptionComponent>(
                    world.find_component(row, "#pose_row_option").unwrap()
                )
                .is_some()
        );
        let body = world.find_component(row, "#pose_row_body").unwrap();
        let row_name_input = world
            .find_component(body, "#pose_row_name_input")
            .and_then(|id| world.get_component_by_id_as::<TextInputComponent>(id))
            .unwrap();
        assert_eq!(
            row_name_input.text, "Neutral",
            "pose rows should expose their names through editable text inputs"
        );
        assert_eq!(
            world.parent_of(world.find_component(row, "#pose_row_option").unwrap()),
            Some(body),
            "only the row body should own the local-selection marker"
        );
        assert!(world.find_component(row, "#pose_apply_action").is_some());
        let row_container_style = world
            .find_component(row, "#pose_row_container_style")
            .and_then(|id| world.get_component_by_id_as::<StyleComponent>(id))
            .unwrap();
        assert_eq!(row_container_style.display, Some(Display::Flex));
        assert_eq!(row_container_style.height, SizeDimension::GlyphUnits(3.2));
        let row_style = world
            .find_component(row, "#pose_row_style")
            .and_then(|id| world.get_component_by_id_as::<StyleComponent>(id))
            .unwrap();
        assert_eq!(row_style.font_size, SizeDimension::GlyphUnits(1.0));
    }

    #[test]
    fn capture_intent_uses_panel_scope_for_completion_rerender() {
        let mut world = World::default();
        let panel_root = world.add_component_boxed_named(
            "pose_capture_panel_root",
            Box::new(TransformComponent::new()),
        );
        let status = world.add_component_boxed_named(
            "pose_panel_status_value",
            Box::new(TextComponent::new("idle")),
        );
        let target = world.add_component(PoseCaptureComponent::new());
        let library = world.add_component(PoseCaptureLibraryComponent::new(PoseTargetRef::Query(
            "TODO".into(),
        )));
        world.add_child(target, library).unwrap();
        let header = spawn_library_header(&mut world, "avatar", target, library);
        world.add_child(panel_root, status).unwrap();
        world.add_child(panel_root, header).unwrap();
        let capture_button = world
            .find_component(header, "#pose_capture_action")
            .unwrap();
        let mut emitter = TestEmitter {
            intents: Vec::new(),
            scopes: Vec::new(),
        };
        let context = Arc::new(Mutex::new(EditorContextState::default()));
        let mut renderer = DataRendererSystem::new();

        assert!(handle_pose_panel_click(
            &mut world,
            &mut emitter,
            panel_root,
            capture_button,
            &context,
            &mut renderer,
        ));
        assert!(matches!(
            emitter.intents.last(),
            Some(IntentValue::PoseCapture {
                target: emitted_target,
                ..
            }) if *emitted_target == target
        ));
        assert_eq!(
            emitter.scopes.last().copied(),
            Some(panel_root),
            "capture completion must remain in the panel's event scope"
        );

        let reset_button = world.find_component(header, "#pose_reset_action").unwrap();
        assert!(handle_pose_panel_click(
            &mut world,
            &mut emitter,
            panel_root,
            reset_button,
            &context,
            &mut renderer,
        ));
        assert!(matches!(
            emitter.intents.last(),
            Some(IntentValue::PoseReset {
                target: emitted_target
            }) if *emitted_target == target
        ));
    }

    #[test]
    fn incompatible_apply_preflight_emits_no_pose_intent() {
        let mut world = World::default();
        let panel_root = world.add_component_boxed_named(
            "pose_capture_panel_root",
            Box::new(TransformComponent::new()),
        );
        let status = world.add_component_boxed_named(
            "pose_panel_status_value",
            Box::new(TextComponent::new("idle")),
        );
        let _ = world.add_child(panel_root, status);
        let target = world.add_component(PoseCaptureComponent::new());
        let gltf = world.add_component(crate::engine::ecs::component::GLTFComponent::new("a.glb"));
        let _ = world.add_child(gltf, target);
        let library = world.add_component(PoseCaptureLibraryComponent::new(PoseTargetRef::Query(
            "#a".into(),
        )));
        let pose = world.add_component(PoseCapturePoseComponent::new(
            "Bad",
            PoseTargetRef::Query("#a".into()),
            vec![PoseBoneEntry {
                query: "#missing".into(),
                translation: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0; 3],
            }],
        ));
        let _ = world.add_child(target, library);
        let _ = world.add_child(library, pose);
        let action = spawn_action_button(
            &mut world,
            panel_root,
            "pose_apply_action",
            "Apply",
            5.0,
            PosePanelActionKind::Apply,
            target,
            library,
            Some(pose),
        );
        let context = Arc::new(Mutex::new(EditorContextState::default()));
        let mut emitter = TestEmitter {
            intents: Vec::new(),
            scopes: Vec::new(),
        };
        let mut renderer = DataRendererSystem::new();
        assert!(handle_pose_panel_click(
            &mut world,
            &mut emitter,
            panel_root,
            action,
            &context,
            &mut renderer,
        ));
        assert!(emitter.intents.is_empty());
        assert!(
            world
                .get_component_by_id_as::<TextComponent>(status)
                .unwrap()
                .text
                .starts_with("apply failed:")
        );
    }

    #[test]
    fn row_select_is_local_and_apply_emits_one_intent_for_selected_gltf() {
        let mut world = World::default();
        let panel_root = world.add_component_boxed_named(
            "pose_capture_panel_root",
            Box::new(TransformComponent::new()),
        );
        let target = world.add_component(PoseCaptureComponent::new());
        let owner_gltf = world.add_component(crate::engine::ecs::component::GLTFComponent::new(
            "owner.glb",
        ));
        let _ = world.add_child(owner_gltf, target);
        let library = world.add_component(PoseCaptureLibraryComponent::new(PoseTargetRef::Query(
            "#owner".into(),
        )));
        let pose = world.add_component(PoseCapturePoseComponent::new(
            "Neutral",
            PoseTargetRef::Query("#owner".into()),
            Vec::new(),
        ));
        let _ = world.add_child(target, library);
        let _ = world.add_child(library, pose);

        let selected_gltf = world.add_component(crate::engine::ecs::component::GLTFComponent::new(
            "selected.glb",
        ));
        let row = spawn_pose_row(&mut world, "Neutral", target, library, pose);
        let _ = world.add_child(panel_root, row);
        let body = world.find_component(row, "#pose_row_body").unwrap();
        let apply = world.find_component(row, "#pose_apply_action").unwrap();

        let context = Arc::new(Mutex::new(EditorContextState {
            selected_component: Some(selected_gltf),
            ..EditorContextState::default()
        }));
        let mut emitter = TestEmitter {
            intents: Vec::new(),
            scopes: Vec::new(),
        };
        let mut renderer = DataRendererSystem::new();
        assert!(handle_pose_panel_click(
            &mut world,
            &mut emitter,
            panel_root,
            body,
            &context,
            &mut renderer,
        ));
        assert!(emitter.intents.is_empty());
        assert_eq!(
            context.lock().unwrap().selected_component,
            Some(selected_gltf),
            "pose-row selection must not replace editor selection"
        );

        assert!(handle_pose_panel_click(
            &mut world,
            &mut emitter,
            panel_root,
            apply,
            &context,
            &mut renderer,
        ));
        assert_eq!(emitter.intents.len(), 1);
        assert!(matches!(
            emitter.intents[0],
            IntentValue::PoseApply {
                target: emitted_target,
                pose: emitted_pose,
            } if emitted_target == selected_gltf && emitted_pose == pose
        ));
    }

    #[test]
    fn save_reports_missing_and_invalid_asset_names() {
        let mut world = World::default();
        let panel_root = world.add_component_boxed_named(
            "pose_capture_panel_root",
            Box::new(TransformComponent::new()),
        );
        let status = world.add_component_boxed_named(
            "pose_panel_status_value",
            Box::new(TextComponent::new("idle")),
        );
        let _ = world.add_child(panel_root, status);
        let target = world.add_component(PoseCaptureComponent::new());
        let library = world.add_component(PoseCaptureLibraryComponent::new(PoseTargetRef::Query(
            "#avatar".into(),
        )));
        let _ = world.add_child(target, library);
        let save = spawn_action_button(
            &mut world,
            panel_root,
            "pose_save_action",
            "Save",
            4.5,
            PosePanelActionKind::Save,
            target,
            library,
            None,
        );
        let context = Arc::new(Mutex::new(EditorContextState::default()));
        let mut emitter = TestEmitter {
            intents: Vec::new(),
            scopes: Vec::new(),
        };
        let mut renderer = DataRendererSystem::new();

        assert!(handle_pose_panel_click(
            &mut world,
            &mut emitter,
            panel_root,
            save,
            &context,
            &mut renderer,
        ));
        assert!(
            world
                .get_component_by_id_as::<TextComponent>(status)
                .unwrap()
                .text
                .contains("asset_name is required")
        );

        world
            .get_component_by_id_as_mut::<PoseCaptureComponent>(target)
            .unwrap()
            .asset_name = Some("../bad".into());
        assert!(handle_pose_panel_click(
            &mut world,
            &mut emitter,
            panel_root,
            save,
            &context,
            &mut renderer,
        ));
        assert!(
            world
                .get_component_by_id_as::<TextComponent>(status)
                .unwrap()
                .text
                .contains("ASCII letters")
        );
    }

    #[test]
    fn editable_name_draft_commits_valid_names_and_retains_invalid_text() {
        let mut world = World::default();
        let panel_root = world.add_component_boxed_named(
            "pose_capture_panel_root",
            Box::new(TransformComponent::new()),
        );
        let status = world.add_component_boxed_named(
            "pose_panel_status_value",
            Box::new(TextComponent::new("idle")),
        );
        world.add_child(panel_root, status).unwrap();
        let target = world.add_component(PoseCaptureComponent::new().with_asset_name("avatar"));
        let library = world.add_component(PoseCaptureLibraryComponent::new(
            crate::engine::ecs::component::PoseTargetRef::Query("TODO".into()),
        ));
        let pose = world.add_component(PoseCapturePoseComponent::new(
            "Idle",
            crate::engine::ecs::component::PoseTargetRef::Query("TODO".into()),
            Vec::new(),
        ));
        world.add_child(target, library).unwrap();
        world.add_child(library, pose).unwrap();
        let header = spawn_library_header(&mut world, "avatar", target, library);
        world.add_child(panel_root, header).unwrap();
        let input = world
            .find_component(header, "#pose_library_name_input")
            .unwrap();
        let row = spawn_pose_row(&mut world, "Idle", target, library, pose);
        world.add_child(panel_root, row).unwrap();
        let pose_input = world.find_component(row, "#pose_row_name_input").unwrap();

        assert!(handle_pose_panel_text_input_changed(
            &mut world,
            panel_root,
            input,
            "avatar_v2",
        ));
        let capture = world
            .get_component_by_id_as::<PoseCaptureComponent>(target)
            .unwrap();
        assert_eq!(capture.asset_name.as_deref(), Some("avatar_v2"));
        assert_eq!(capture.asset_name_draft(), "avatar_v2");
        assert_eq!(world.parent_of(pose), Some(library));

        assert!(handle_pose_panel_text_input_changed(
            &mut world, panel_root, input, "../bad",
        ));
        let capture = world
            .get_component_by_id_as::<PoseCaptureComponent>(target)
            .unwrap();
        assert_eq!(capture.asset_name.as_deref(), Some("avatar_v2"));
        assert_eq!(capture.asset_name_draft(), "../bad");
        assert!(
            world
                .get_component_by_id_as::<TextComponent>(status)
                .unwrap()
                .text
                .contains("invalid name")
        );

        assert!(handle_pose_panel_text_input_changed(
            &mut world,
            panel_root,
            pose_input,
            "Relaxed Idle",
        ));
        assert_eq!(
            world
                .get_component_by_id_as::<PoseCapturePoseComponent>(pose)
                .unwrap()
                .name,
            "Relaxed Idle"
        );
        assert!(matches!(
            world
                .get_component_by_id_as::<PoseCaptureComponent>(target)
                .unwrap()
                .runtime
                .state,
            PoseCaptureReconciliationState::Unsaved
        ));

        assert!(handle_pose_panel_text_input_changed(
            &mut world, panel_root, pose_input, "   ",
        ));
        assert_eq!(
            world
                .get_component_by_id_as::<PoseCapturePoseComponent>(pose)
                .unwrap()
                .name,
            "Relaxed Idle"
        );
    }

    #[test]
    fn new_library_uses_visual_selection_and_unrelated_selection_is_safe() {
        let mut world = World::default();
        let panel_root = world.add_component_boxed_named(
            "pose_capture_panel_root",
            Box::new(TransformComponent::new()),
        );
        let status = world.add_component_boxed_named(
            "pose_panel_status_value",
            Box::new(TextComponent::new("idle")),
        );
        world.add_child(panel_root, status).unwrap();
        let gltf = world.add_component_boxed_named(
            "Cat Avatar",
            Box::new(crate::engine::ecs::component::GLTFComponent::new(
                "models/cat.2.glb",
            )),
        );
        let node = world.add_component_boxed_named("body", Box::new(TransformComponent::new()));
        let primitive =
            world.add_component(crate::engine::ecs::component::RenderableComponent::cube());
        world.add_child(node, primitive).unwrap();
        {
            world
                .get_component_by_id_as_mut::<crate::engine::ecs::component::GLTFComponent>(gltf)
                .unwrap()
                .spawned_node_transforms = vec![node];
        }
        let action = spawn_new_library_button(&mut world);
        world.add_child(panel_root, action).unwrap();
        let context = Arc::new(Mutex::new(EditorContextState {
            selected_component: Some(primitive),
            ..EditorContextState::default()
        }));
        let mut emitter = TestEmitter {
            intents: Vec::new(),
            scopes: Vec::new(),
        };
        let mut renderer = DataRendererSystem::new();

        assert!(handle_pose_panel_click(
            &mut world,
            &mut emitter,
            panel_root,
            action,
            &context,
            &mut renderer,
        ));
        let capture = world
            .children_of(gltf)
            .iter()
            .copied()
            .find(|&id| {
                world
                    .get_component_by_id_as::<PoseCaptureComponent>(id)
                    .is_some()
            })
            .unwrap();
        let capture_component = world
            .get_component_by_id_as::<PoseCaptureComponent>(capture)
            .unwrap();
        assert_eq!(capture_component.label.as_deref(), Some("Cat Avatar"));
        assert_eq!(capture_component.asset_name.as_deref(), Some("cat_2"));
        assert_eq!(
            world
                .children_of(capture)
                .iter()
                .filter(|&&id| world
                    .get_component_by_id_as::<PoseCaptureLibraryComponent>(id)
                    .is_some())
                .count(),
            1
        );

        let mut unrelated_world = World::default();
        let unrelated_panel = unrelated_world.add_component_boxed_named(
            "pose_capture_panel_root",
            Box::new(TransformComponent::new()),
        );
        let unrelated_status = unrelated_world.add_component_boxed_named(
            "pose_panel_status_value",
            Box::new(TextComponent::new("idle")),
        );
        unrelated_world
            .add_child(unrelated_panel, unrelated_status)
            .unwrap();
        let unrelated = unrelated_world.add_component(TransformComponent::new());
        let action = spawn_new_library_button(&mut unrelated_world);
        unrelated_world.add_child(unrelated_panel, action).unwrap();
        let context = Arc::new(Mutex::new(EditorContextState {
            selected_component: Some(unrelated),
            ..EditorContextState::default()
        }));
        assert!(handle_pose_panel_click(
            &mut unrelated_world,
            &mut emitter,
            unrelated_panel,
            action,
            &context,
            &mut renderer,
        ));
        assert_eq!(
            unrelated_world
                .all_components()
                .filter(|&id| unrelated_world
                    .get_component_by_id_as::<PoseCaptureComponent>(id)
                    .is_some())
                .count(),
            0
        );
    }

    #[test]
    fn selection_activation_keeps_all_gltf_libraries_in_the_global_model() {
        let mut world = World::default();
        let panel_root = world.add_component_boxed_named(
            "pose_capture_panel_root",
            Box::new(TransformComponent::new()),
        );
        let content_slot =
            world.add_component_boxed_named("content_slot", Box::new(TransformComponent::new()));
        let status = world.add_component_boxed_named(
            "pose_panel_status_value",
            Box::new(TextComponent::new("idle")),
        );
        world.add_child(panel_root, content_slot).unwrap();
        world.add_child(panel_root, status).unwrap();

        let gltf_a = world.add_component(crate::engine::ecs::component::GLTFComponent::new(
            "selection_global_a.glb",
        ));
        let gltf_b = world.add_component(crate::engine::ecs::component::GLTFComponent::new(
            "selection_global_b.glb",
        ));
        let node_a = world.add_component(TransformComponent::new());
        let node_b = world.add_component(TransformComponent::new());
        let primitive_a =
            world.add_component(crate::engine::ecs::component::RenderableComponent::cube());
        let primitive_b =
            world.add_component(crate::engine::ecs::component::RenderableComponent::cube());
        world.add_child(node_a, primitive_a).unwrap();
        world.add_child(node_b, primitive_b).unwrap();
        world
            .get_component_by_id_as_mut::<crate::engine::ecs::component::GLTFComponent>(gltf_a)
            .unwrap()
            .spawned_node_transforms = vec![node_a];
        world
            .get_component_by_id_as_mut::<crate::engine::ecs::component::GLTFComponent>(gltf_b)
            .unwrap()
            .spawned_node_transforms = vec![node_b];

        let mut emitter = TestEmitter {
            intents: Vec::new(),
            scopes: Vec::new(),
        };
        let mut renderer = DataRendererSystem::new();
        assert!(activate_pose_panel_for_selection(
            &mut world,
            &mut emitter,
            panel_root,
            Some(primitive_a),
            &mut renderer,
        ));
        assert!(activate_pose_panel_for_selection(
            &mut world,
            &mut emitter,
            panel_root,
            Some(primitive_b),
            &mut renderer,
        ));
        let model = build_pose_panel_model(&world);
        assert_eq!(model.sections.len(), 2);
        assert!(
            model
                .sections
                .iter()
                .any(|section| { world.parent_of(section.target) == Some(gltf_a) })
        );
        assert!(
            model
                .sections
                .iter()
                .any(|section| { world.parent_of(section.target) == Some(gltf_b) })
        );

        let unrelated = world.add_component(TransformComponent::new());
        assert!(!activate_pose_panel_for_selection(
            &mut world,
            &mut emitter,
            panel_root,
            Some(unrelated),
            &mut renderer,
        ));
        assert_eq!(build_pose_panel_model(&world).sections.len(), 2);
    }

    #[test]
    fn load_failure_status_remains_visible_with_an_empty_usable_library() {
        let mut world = World::default();
        let target = world.add_component(PoseCaptureComponent::new().with_asset_name("broken"));
        let library = world.add_component(PoseCaptureLibraryComponent::new(PoseTargetRef::Query(
            "TODO".into(),
        )));
        world.add_child(target, library).unwrap();
        world
            .get_component_by_id_as_mut::<PoseCaptureComponent>(target)
            .unwrap()
            .runtime
            .state = PoseCaptureReconciliationState::LoadFailed {
            asset_name: "broken".into(),
            error: "malformed manifest".into(),
            overwrite_warning_issued: false,
        };

        let model = build_pose_panel_model(&world);
        assert_eq!(model.sections.len(), 1);
        assert_eq!(
            pose_panel_runtime_status(&world, &model),
            "load failed: malformed manifest"
        );
        assert!(world.children_of(library).is_empty());
    }

    #[test]
    fn failed_hydration_requires_two_saves_before_replacement() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let asset_name = format!("pose_overwrite_guard_{nonce}");
        let directory = pose_assets_root().join(&asset_name);
        let _ = std::fs::remove_dir_all(&directory);

        let mut world = World::default();
        let panel_root = world.add_component_boxed_named(
            "pose_capture_panel_root",
            Box::new(TransformComponent::new()),
        );
        let status = world.add_component_boxed_named(
            "pose_panel_status_value",
            Box::new(TextComponent::new("idle")),
        );
        world.add_child(panel_root, status).unwrap();
        let target = world.add_component(PoseCaptureComponent::new().with_asset_name(&asset_name));
        let library = world.add_component(PoseCaptureLibraryComponent::new(PoseTargetRef::Query(
            "TODO".into(),
        )));
        world.add_child(target, library).unwrap();
        {
            let capture = world
                .get_component_by_id_as_mut::<PoseCaptureComponent>(target)
                .unwrap();
            capture.runtime.asset_name_draft = Some(asset_name.clone());
            capture.runtime.state = PoseCaptureReconciliationState::LoadFailed {
                asset_name: asset_name.clone(),
                error: "malformed manifest".into(),
                overwrite_warning_issued: false,
            };
        }
        let save = spawn_action_button(
            &mut world,
            panel_root,
            "pose_save_action",
            "Save",
            4.5,
            PosePanelActionKind::Save,
            target,
            library,
            None,
        );
        let context = Arc::new(Mutex::new(EditorContextState::default()));
        let mut emitter = TestEmitter {
            intents: Vec::new(),
            scopes: Vec::new(),
        };
        let mut renderer = DataRendererSystem::new();

        assert!(handle_pose_panel_click(
            &mut world,
            &mut emitter,
            panel_root,
            save,
            &context,
            &mut renderer,
        ));
        assert!(!directory.join("library.mms").exists());
        assert!(
            world
                .get_component_by_id_as::<TextComponent>(status)
                .unwrap()
                .text
                .contains("Save again")
        );

        assert!(handle_pose_panel_click(
            &mut world,
            &mut emitter,
            panel_root,
            save,
            &context,
            &mut renderer,
        ));
        assert!(directory.join("library.mms").exists());
        assert!(matches!(
            world
                .get_component_by_id_as::<PoseCaptureComponent>(target)
                .unwrap()
                .runtime
                .state,
            PoseCaptureReconciliationState::Hydrated
        ));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
