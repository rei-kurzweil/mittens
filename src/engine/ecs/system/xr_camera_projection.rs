//! XR eye projection and render-only stage placement.
use super::OpenXRSystem;
use crate::engine::ecs::system::avatar_control_system::camera_view::third_person_model;
use crate::engine::ecs::{ComponentId, World, component::CameraXRComponent};
use crate::engine::graphics::{CameraData, VisualWorld};
use crate::engine::transform::TransformMatrix;

#[derive(Default)]
pub(super) struct EyeView {
    initial_view_from_stage: Option<(ComponentId, TransformMatrix)>,
}

impl EyeView {
    pub(super) fn cameras(
        &mut self,
        world: &World,
        visuals: &VisualWorld,
        raw_head: openxr::Posef,
        views: &[openxr::View],
    ) -> Option<Vec<CameraData>> {
        let tracked_frame = OpenXRSystem::tracking_world_frame(world, visuals, raw_head)?;
        let camera_id = visuals
            .active_xr_camera()
            .or_else(|| OpenXRSystem::first_enabled_camera_xr(world))?;
        let camera = world.get_component_by_id_as::<CameraXRComponent>(camera_id)?;
        let world_from_stage =
            if let Some(model) = third_person_model(world, camera.avatar_view_owner) {
                // Center the view at entry, then preserve headset movement and
                // both eyes' offsets in OpenXR stage space.
                let initial_view_from_stage = match self.initial_view_from_stage {
                    Some((id, reference)) if id == camera_id => reference,
                    _ => {
                        let inverse_head = OpenXRSystem::invert_affine_transform(
                            &OpenXRSystem::mat4_from_pose(raw_head),
                        );
                        self.initial_view_from_stage = Some((camera_id, inverse_head));
                        inverse_head
                    }
                };
                OpenXRSystem::mul_mat4(&model, &initial_view_from_stage)
            } else {
                self.initial_view_from_stage = None;
                tracked_frame
            };
        Some(
            views
                .iter()
                .map(|v| {
                    let world_from_eye = OpenXRSystem::mul_mat4(
                        &world_from_stage,
                        &OpenXRSystem::mat4_from_pose(v.pose),
                    );
                    CameraData {
                        view: OpenXRSystem::invert_affine_transform(&world_from_eye),
                        proj: OpenXRSystem::proj_from_fov_rh_zo(v.fov, camera.z_near, camera.z_far),
                        transform: OpenXRSystem::transform_from_matrix_world(world_from_eye),
                    }
                })
                .collect(),
        )
    }
}
