use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};

use crate::engine::ecs::component::{
    ColorComponent, DataComponent, DataValue, Display, EdgeInsets, OptionComponent,
    PoseCaptureComponent, PoseCaptureLibraryComponent, PoseCapturePoseComponent, PoseTargetRef,
    RaycastableComponent, SizeDimension, StyleComponent, TextComponent, TransformComponent,
    is_valid_pose_asset_name, save_pose_library_asset,
};
use crate::engine::ecs::system::data_renderer_system::{
    DataRendererSystem, ItemRendererSpec, RendererSpec, UiItem, UiItemKind,
};
use crate::engine::ecs::system::editor::context::EditorContextState;
use crate::engine::ecs::system::editor::world_panel::PANEL_CONTENT_SLOT_SELECTOR;
use crate::engine::ecs::system::panel_system::{data_text, is_descendant_or_self};
use crate::engine::ecs::system::pose_capture_system::{
    resolve_pose_apply_target, validate_pose_apply,
};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};

pub const POSE_PANEL_ROOT_SELECTOR: &str = "#pose_capture_panel_root";
pub const POSE_PANEL_SELECTION_NAME: &str = "pose_capture_selection";
pub const POSE_PANEL_PAYLOAD_NAME: &str = "pose_panel_payload";
pub const POSE_PANEL_STATUS_VALUE_SELECTOR: &str = "#pose_panel_status_value";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PosePanelActionKind {
    Capture,
    Save,
    Select,
    Apply,
}

impl PosePanelActionKind {
    fn label(self) -> &'static str {
        match self {
            Self::Capture => "Capture",
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
    let mut items = Vec::new();
    for section in &model.sections {
        items.push(UiItem {
            key: "pose_library_header".to_string(),
            kind: UiItemKind::Info,
            label: section.label.clone(),
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
    target: ComponentId,
    library: ComponentId,
    pose: Option<ComponentId>,
) {
    let mut data = DataComponent::new()
        .with_entry("action", DataValue::Text(action.label().to_string()))
        .with_entry("target_component", DataValue::Component(target))
        .with_entry("library", DataValue::Component(library));
    if let Some(pose) = pose {
        data.insert("pose", DataValue::Component(pose));
    }
    let payload = world.add_component_boxed_named(POSE_PANEL_PAYLOAD_NAME, Box::new(data));
    let _ = world.add_child(parent, payload);
}

fn spawn_text(world: &mut World, parent: ComponentId, name: &str, label: &str, color: [f32; 4]) {
    let text = world.add_component_boxed_named(name, Box::new(TextComponent::new(label)));
    let text_color = world.add_component_boxed_named(
        format!("{name}_color"),
        Box::new(ColorComponent::rgba(color[0], color[1], color[2], color[3])),
    );
    let _ = world.add_child(parent, text);
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
            style.width = SizeDimension::GlyphUnits(width);
            style.height = SizeDimension::GlyphUnits(2.2);
            style.margin = EdgeInsets::axes(0.2, 0.15);
            style.padding = EdgeInsets::axes(0.25, 0.35);
            style.background_color = Some([0.10, 0.55, 0.18, 1.0]);
            style.background_z = Some(0.001);
            style.color = Some([0.75, 1.0, 0.45, 1.0]);
            style.font_size = SizeDimension::GlyphUnits(0.9);
            style
        }),
    );
    let text_root = world.add_component_boxed_named(
        format!("{name}_text_root"),
        Box::new(TransformComponent::new().with_position(0.0, 0.0, 0.005)),
    );
    let _ = world.add_child(parent, root);
    let _ = world.add_child(root, raycastable);
    let _ = world.add_child(root, style);
    let _ = world.add_child(root, text_root);
    spawn_text(
        world,
        text_root,
        &format!("{name}_text"),
        label,
        [0.75, 1.0, 0.45, 1.0],
    );
    add_payload(world, root, action, target, library, pose);
    root
}

fn spawn_library_header(
    world: &mut World,
    label: &str,
    target: ComponentId,
    library: ComponentId,
) -> ComponentId {
    let root =
        world.add_component_boxed_named("pose_library_header", Box::new(TransformComponent::new()));
    let style = world.add_component_boxed_named(
        "pose_library_header_style",
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::Block);
            style.width = SizeDimension::Percent(100.0);
            style.margin = EdgeInsets::axes(0.0, 0.35);
            style.padding = EdgeInsets::axes(0.25, 0.2);
            style.background_color = Some([0.16, 0.20, 0.18, 1.0]);
            style.background_z = Some(0.001);
            style.color = Some([0.95, 0.98, 0.92, 1.0]);
            style.font_size = SizeDimension::GlyphUnits(1.0);
            style
        }),
    );
    let label_root = world.add_component_boxed_named(
        "pose_library_header_label",
        Box::new(TransformComponent::new()),
    );
    let label_style = world.add_component_boxed_named(
        "pose_library_header_label_style",
        Box::new({
            let mut style = StyleComponent::new();
            style.display = Some(Display::InlineBlock);
            style.width = SizeDimension::GlyphUnits(15.0);
            style.height = SizeDimension::GlyphUnits(2.2);
            style.padding = EdgeInsets::axes(0.25, 0.35);
            style
        }),
    );
    let _ = world.add_child(root, style);
    let _ = world.add_child(root, label_root);
    let _ = world.add_child(label_root, label_style);
    spawn_text(
        world,
        label_root,
        "pose_library_header_text",
        label,
        [0.95, 0.98, 0.92, 1.0],
    );
    spawn_action_button(
        world,
        root,
        "pose_capture_action",
        "Capture",
        5.5,
        PosePanelActionKind::Capture,
        target,
        library,
        None,
    );
    spawn_action_button(
        world,
        root,
        "pose_save_action",
        "Save",
        4.5,
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
    let _ = world.add_child(root, body);
    let _ = world.add_child(body, option);
    let _ = world.add_child(body, body_raycastable);
    let _ = world.add_child(body, body_style);
    spawn_text(world, body, "pose_row_text", label, [0.0, 0.0, 0.0, 1.0]);
    add_payload(
        world,
        body,
        PosePanelActionKind::Select,
        target,
        library,
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

fn pose_panel_item_render_fn(
    world: &mut World,
    _emit: &mut dyn SignalEmitter,
    item: &UiItem,
) -> Result<ComponentId, String> {
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

fn ensure_pose_libraries(world: &mut World, emit: &mut dyn SignalEmitter) {
    let targets: Vec<_> = world
        .all_components()
        .filter(|&id| {
            world
                .get_component_by_id_as::<PoseCaptureComponent>(id)
                .is_some()
        })
        .collect();
    for target in targets {
        let has_library = world.children_of(target).iter().any(|&child| {
            world
                .get_component_by_id_as::<PoseCaptureLibraryComponent>(child)
                .is_some()
        });
        if !has_library {
            let library = world.add_component(PoseCaptureLibraryComponent::new(
                PoseTargetRef::Query("TODO".to_string()),
            ));
            let _ = world.add_child(target, library);
            world.init_component_tree(library, emit);
        }
    }
}

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
    ensure_pose_libraries(world, emit);
    let items = pose_panel_items(&build_pose_panel_model(world));
    if let Err(error) =
        data_renderer.render_list(world, emit, content_slot, &POSE_PANEL_ITEM_SPEC, &items)
    {
        eprintln!("[InspectorSystem] pose panel content render error: {error}");
    }
}

fn set_pose_panel_status(world: &mut World, panel_root: ComponentId, status: impl Into<String>) {
    if let Some(text_id) = world.find_component(panel_root, POSE_PANEL_STATUS_VALUE_SELECTOR)
        && let Some(text) = world.get_component_by_id_as_mut::<TextComponent>(text_id)
    {
        text.text = status.into();
    }
}

fn pose_assets_root() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/components/poses"
    ))
}

pub fn handle_pose_panel_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    clicked_node: ComponentId,
    editor_context_state: &Arc<Mutex<EditorContextState>>,
    _data_renderer: &mut DataRendererSystem,
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
                "Select" => return true,
                "Capture" => {
                    if let Some(target) = target {
                        set_pose_panel_status(world, panel_root, "capturing pose...");
                        emit.push_intent_now(
                            target,
                            IntentValue::PoseCapture {
                                target,
                                pose_name: None,
                            },
                        );
                        return true;
                    }
                }
                "Save" => {
                    if let (Some(target), Some(library)) = (target, library) {
                        let asset_name = world
                            .get_component_by_id_as::<PoseCaptureComponent>(target)
                            .and_then(|capture| capture.asset_name.clone());
                        let status = match asset_name {
                            None => "save failed: PoseCapture asset_name is required".to_string(),
                            Some(asset_name) if !is_valid_pose_asset_name(&asset_name) => {
                                "save failed: asset_name may contain only ASCII letters, digits, '_' or '-'"
                                    .to_string()
                            }
                            Some(asset_name) => {
                                let directory = pose_assets_root().join(&asset_name);
                                let manifest = directory.join("library.mms");
                                match save_pose_library_asset(world, library, &manifest) {
                                    Ok(paths) => format!(
                                        "saved {} poses to assets/components/poses/{asset_name}/",
                                        paths.len()
                                    ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::PoseBoneEntry;
    use crate::engine::ecs::{EventSignal, IntentSignal};

    struct TestEmitter {
        intents: Vec<IntentValue>,
    }

    impl SignalEmitter for TestEmitter {
        fn push_event(&mut self, _scope: ComponentId, _event: EventSignal) {}

        fn push_intent(&mut self, _scope: ComponentId, intent: IntentSignal) {
            self.intents.push(intent.value);
        }
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
        };
        let header =
            pose_panel_item_render_fn(&mut world, &mut emit, &pose_panel_items(&model)[0]).unwrap();
        assert!(
            world
                .find_component(header, "#pose_capture_action")
                .is_some()
        );
        assert!(world.find_component(header, "#pose_save_action").is_some());

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
        assert_eq!(
            world.parent_of(world.find_component(row, "#pose_row_option").unwrap()),
            Some(body),
            "only the row body should own the local-selection marker"
        );
        assert!(world.find_component(row, "#pose_apply_action").is_some());
        let row_style = world
            .find_component(row, "#pose_row_style")
            .and_then(|id| world.get_component_by_id_as::<StyleComponent>(id))
            .unwrap();
        assert_eq!(row_style.font_size, SizeDimension::GlyphUnits(1.0));
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
}
