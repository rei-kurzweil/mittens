use crate::engine::ecs::component::{
    DataComponent, DataValue, PoseCaptureComponent, PoseCaptureLibraryComponent,
    PoseCapturePoseComponent,
};
use crate::engine::ecs::system::data_renderer_system::DataRendererSystem;
use crate::engine::ecs::system::editor::panel_ui::{
    PanelUiRowSpec, spawn_panel_ui_row_tree, spawn_panel_ui_section_header_tree,
};
use crate::engine::ecs::system::editor::world_panel::mark_nearest_layout_dirty;
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

    let Some(content_slot) = world.find_component(panel_root, "#content_area") else {
        return;
    };

    // Clear content
    let children = world.children_of(content_slot).to_vec();
    for ch in children {
        let _ = world.remove_component_subtree(ch);
    }

    let model = build_pose_panel_model(world);

    for section in model.sections {
        let header =
            spawn_panel_ui_section_header_tree(world, "pose_section_header", &section.label);
        let _ = world.add_child(content_slot, header);

        for row in section.poses {
            let row_spec = PanelUiRowSpec {
                row_name: "pose_row",
                payload_name: POSE_PANEL_PAYLOAD_NAME,
                target_component: Some(row.pose),
                label: &row.label,
                row_kind_label: "PoseRow",
                interactive: true,
                background_rgba: [0.92, 0.97, 0.92, 1.0],
                text_rgba: [0.0, 0.0, 0.0, 1.0],
                font_size_gu: None,
                spacer_height_gu: None,
            };
            let row_node = spawn_panel_ui_row_tree(world, row_spec);

            // Add extra payload for target
            if let Some(payload_id) =
                world.find_component(row_node, &format!("[name='{POSE_PANEL_PAYLOAD_NAME}']"))
            {
                if let Some(data) = world.get_component_by_id_as_mut::<DataComponent>(payload_id) {
                    data.insert("pose_target", DataValue::Component(row.target));
                }
            }

            let _ = world.add_child(content_slot, row_node);
        }
    }

    let _ = data_renderer;
    world.init_component_tree(content_slot, emit);
    mark_nearest_layout_dirty(world, content_slot);
}

pub fn handle_pose_panel_click(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    panel_query_root: ComponentId,
    clicked_node: ComponentId,
    data_renderer: &mut DataRendererSystem,
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
