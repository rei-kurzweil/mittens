use crate::engine::ecs::ComponentId;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{PoseCaptureComponent, PoseCapturePoseComponent, PoseCaptureLibraryComponent};

pub const POSE_PANEL_ROOT_SELECTOR: &str = "#pose_capture_panel_root";
pub const POSE_PANEL_SELECTION_NAME: &str = "pose_capture_selection";
pub const POSE_PANEL_PAYLOAD_NAME: &str = "pose_panel_payload";

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
                world.component_label(id)
                    .map(|s| s.to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| format!("Target {:?}", id))
            });
            
            let mut poses = Vec::new();
            // Find library and its poses
            for &child in world.children_of(id) {
                if world.get_component_by_id_as::<PoseCaptureLibraryComponent>(child).is_some() {
                    for &pose_id in world.children_of(child) {
                        if let Some(pose_comp) = world.get_component_by_id_as::<PoseCapturePoseComponent>(pose_id) {
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
