use std::sync::LazyLock;

use crate::engine::ecs::component::{
    DataComponent, DataValue, PoseCaptureComponent, PoseCaptureLibraryComponent,
    PoseCapturePoseComponent,
};
use crate::engine::ecs::system::data_renderer_system::{
    DataRendererSystem, ItemRendererSpec, RendererSpec, UiItem, UiItemKind,
};
use crate::engine::ecs::system::editor::panel_ui::{
    PanelUiRowSpec, spawn_panel_ui_row_tree, spawn_panel_ui_section_header_tree,
};
use crate::engine::ecs::system::editor::world_panel::PANEL_CONTENT_SLOT_SELECTOR;
use crate::engine::ecs::system::panel_system::{data_text, is_descendant_or_self};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};

pub const POSE_PANEL_ROOT_SELECTOR: &str = "#pose_capture_panel_root";
pub const POSE_PANEL_SELECTION_NAME: &str = "pose_capture_selection";
pub const POSE_PANEL_PAYLOAD_NAME: &str = "pose_panel_payload";
pub const POSE_PANEL_CAPTURE_BUTTON_SELECTOR: &str = "#pose_capture_button";

#[derive(Debug, Clone, Default)]
pub struct PosePanelModel {
    pub sections: Vec<PosePanelSection>,
}

#[derive(Debug, Clone)]
pub struct PosePanelSection {
    pub target: ComponentId,
    pub label: String,
    pub poses: Vec<PosePanelRow>,
}

#[derive(Debug, Clone)]
pub struct PosePanelRow {
    pub target: ComponentId,
    pub pose: ComponentId,
    pub label: String,
}

fn pose_panel_items(model: &PosePanelModel) -> Vec<UiItem> {
    let mut items = Vec::new();
    for section in &model.sections {
        items.push(UiItem {
            key: "pose_section_header".to_string(),
            kind: UiItemKind::Info,
            label: section.label.clone(),
            selected: false,
            target_ref: None,
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

fn pose_target_for_pose(world: &World, pose: ComponentId) -> Option<ComponentId> {
    let library = world.parent_of(pose)?;
    world.get_component_by_id_as::<PoseCaptureLibraryComponent>(library)?;
    let target = world.parent_of(library)?;
    world.get_component_by_id_as::<PoseCaptureComponent>(target)?;
    Some(target)
}

fn pose_panel_item_render_fn(
    world: &mut World,
    _emit: &mut dyn SignalEmitter,
    item: &UiItem,
) -> Result<ComponentId, String> {
    match item.kind {
        UiItemKind::Info => Ok(spawn_panel_ui_section_header_tree(
            world,
            &item.key,
            &item.label,
        )),
        UiItemKind::Component => {
            let pose = item
                .target_ref
                .ok_or_else(|| "pose panel row missing pose component".to_string())?;
            let target = pose_target_for_pose(world, pose)
                .ok_or_else(|| "pose panel row missing pose capture target".to_string())?;
            let row_node = spawn_panel_ui_row_tree(
                world,
                PanelUiRowSpec {
                    row_name: &item.key,
                    payload_name: POSE_PANEL_PAYLOAD_NAME,
                    target_component: Some(pose),
                    label: &item.label,
                    row_kind_label: "PoseRow",
                    interactive: true,
                    background_rgba: [0.92, 0.97, 0.92, 1.0],
                    text_rgba: [0.0, 0.0, 0.0, 1.0],
                    font_size_gu: None,
                    spacer_height_gu: None,
                },
            );

            if let Some(payload_id) =
                world.find_component(row_node, &format!("[name='{POSE_PANEL_PAYLOAD_NAME}']"))
                && let Some(data) = world.get_component_by_id_as_mut::<DataComponent>(payload_id)
            {
                data.insert("pose_target", DataValue::Component(target));
            }

            Ok(row_node)
        }
        kind => Err(format!("unsupported pose panel item kind: {kind:?}")),
    }
}

static POSE_PANEL_ITEM_SPEC: LazyLock<ItemRendererSpec> = LazyLock::new(|| RendererSpec::Rust {
    render_fn: Box::new(pose_panel_item_render_fn),
});

pub fn build_pose_panel_model(world: &World) -> PosePanelModel {
    let mut sections = Vec::new();

    // Find all PoseCaptureComponent targets
    for id in world.all_components() {
        if let Some(pc) = world.get_component_by_id_as::<PoseCaptureComponent>(id) {
            let label = pc.label.clone().unwrap_or_else(|| {
                world
                    .component_label(id)
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| format!("Target {:?}", id))
            });

            let mut poses = Vec::new();
            // Find library and its poses
            for &child in world.children_of(id) {
                if world
                    .get_component_by_id_as::<PoseCaptureLibraryComponent>(child)
                    .is_some()
                {
                    for &pose_id in world.children_of(child) {
                        if let Some(pose_comp) =
                            world.get_component_by_id_as::<PoseCapturePoseComponent>(pose_id)
                        {
                            poses.push(PosePanelRow {
                                target: id,
                                pose: pose_id,
                                label: pose_comp.name.clone(),
                            });
                        }
                    }
                }
            }

            sections.push(PosePanelSection {
                target: id,
                label,
                poses,
            });
        }
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

    let model = build_pose_panel_model(world);
    let items = pose_panel_items(&model);
    if let Err(error) =
        data_renderer.render_list(world, emit, content_slot, &POSE_PANEL_ITEM_SPEC, &items)
    {
        eprintln!("[InspectorSystem] pose panel content render error: {error}");
    }
}

pub fn handle_pose_panel_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    clicked_node: ComponentId,
    _data_renderer: &mut DataRendererSystem,
) -> bool {
    let Some(panel_root) = world.find_component(panel_query_root, POSE_PANEL_ROOT_SELECTOR) else {
        return false;
    };

    if !is_descendant_or_self(world, panel_root, clicked_node) {
        return false;
    }

    if let Some(capture_button) =
        world.find_component(panel_root, POSE_PANEL_CAPTURE_BUTTON_SELECTOR)
        && is_descendant_or_self(world, capture_button, clicked_node)
    {
        emit.push_intent_now(
            panel_root,
            IntentValue::PoseCapture {
                target: panel_root,
                pose_name: None,
            },
        );
        return true;
    }

    // Search up for a payload
    let mut current = Some(clicked_node);
    while let Some(curr_id) = current {
        if let Some(payload_id) = world
            .children_of(curr_id)
            .iter()
            .find(|&&child| world.component_label(child) == Some(POSE_PANEL_PAYLOAD_NAME))
        {
            if let Some(data) = world.get_component_by_id_as::<DataComponent>(*payload_id) {
                let row_kind = data_text(data, "row_kind").unwrap_or_default();
                match row_kind.as_str() {
                    "PoseRow" => {
                        let pose_id = data.get_component("target_component");
                        let target_id = data.get_component("pose_target");
                        if let (Some(pose), Some(target)) = (pose_id, target_id) {
                            emit.push_intent_now(target, IntentValue::PoseApply { target, pose });
                            return true;
                        }
                    }
                    "PoseAdd" => {
                        let target_id = data.get_component("target_component");
                        if let Some(target) = target_id {
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
                    _ => {}
                }
            }
        }
        current = world.parent_of(curr_id);
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{PoseTargetRef, SizeDimension, StyleComponent};
    use crate::engine::ecs::{EventSignal, IntentSignal};

    struct TestEmitter;

    impl SignalEmitter for TestEmitter {
        fn push_event(&mut self, _scope: ComponentId, _event: EventSignal) {}

        fn push_intent(&mut self, _scope: ComponentId, _intent: IntentSignal) {}
    }

    #[test]
    fn pose_panel_renderer_preserves_payload_and_explicit_text_size() {
        let mut world = World::default();
        let target = world.add_component_boxed_named(
            "pose_target",
            Box::new(PoseCaptureComponent::new().with_label("Avatar")),
        );
        let library = world.add_component_boxed_named(
            "pose_library",
            Box::new(PoseCaptureLibraryComponent::new(PoseTargetRef::Query(
                "#avatar".to_string(),
            ))),
        );
        let pose = world.add_component_boxed_named(
            "captured_pose",
            Box::new(PoseCapturePoseComponent::new(
                "Neutral",
                PoseTargetRef::Query("#avatar".to_string()),
                Vec::new(),
            )),
        );
        let _ = world.add_child(target, library);
        let _ = world.add_child(library, pose);

        let item = UiItem {
            key: "pose_row".to_string(),
            kind: UiItemKind::Component,
            label: "Neutral".to_string(),
            selected: false,
            target_ref: Some(pose),
        };
        let mut emit = TestEmitter;
        let row = pose_panel_item_render_fn(&mut world, &mut emit, &item).unwrap();
        let payload = world
            .find_component(row, &format!("[name='{POSE_PANEL_PAYLOAD_NAME}']"))
            .and_then(|id| world.get_component_by_id_as::<DataComponent>(id))
            .expect("rendered pose row payload");
        assert_eq!(payload.get_component("target_component"), Some(pose));
        assert_eq!(payload.get_component("pose_target"), Some(target));

        let row_style = world
            .find_component(row, "[name='pose_row_style']")
            .and_then(|id| world.get_component_by_id_as::<StyleComponent>(id))
            .expect("rendered pose row style");
        assert_eq!(
            row_style.font_size,
            SizeDimension::GlyphUnits(1.0),
            "pose rows should not inherit an oversized font"
        );

        let header =
            spawn_panel_ui_section_header_tree(&mut world, "pose_section_header", "Avatar");
        let header_style = world
            .find_component(header, "[name='pose_section_header_style']")
            .and_then(|id| world.get_component_by_id_as::<StyleComponent>(id))
            .expect("rendered pose header style");
        assert_eq!(
            header_style.font_size,
            SizeDimension::GlyphUnits(1.0),
            "pose headers should not inherit an oversized font"
        );
    }
}
