//! XR eye projection with the selected camera's authored clip distances.
use super::OpenXRSystem;
use crate::engine::ecs::{World, component::CameraXRComponent};
use crate::engine::graphics::{CameraData, VisualWorld};

pub(super) fn eye_cameras(
    world: &World,
    visuals: &VisualWorld,
    raw_head: openxr::Posef,
    views: &[openxr::View],
) -> Option<Vec<CameraData>> {
    let world_from_stage = OpenXRSystem::tracking_world_frame(world, visuals, raw_head)?;
    let camera_id = visuals
        .active_xr_camera()
        .or_else(|| OpenXRSystem::first_enabled_camera_xr(world))?;
    let camera = world.get_component_by_id_as::<CameraXRComponent>(camera_id)?;
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
