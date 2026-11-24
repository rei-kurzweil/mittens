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
        let z_far = view_far(camera, &world_from_stage);
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
                        proj: OpenXRSystem::proj_from_fov_rh_zo(v.fov, camera.z_near, z_far),
                        transform: OpenXRSystem::transform_from_matrix_world(world_from_eye),
                    }
                })
                .collect(),
        )
    }
}

/// A scaled rig (a tiny player) scales view space with it. Far is a world
/// distance, so it reaches as far into the world at any scale; near stays in
/// view space so it stays close to a shrunken viewer.
fn view_far(camera: &CameraXRComponent, world_from_stage: &[[f32; 4]; 4]) -> f32 {
    let column = world_from_stage[1];
    let rig_scale = (column[0] * column[0] + column[1] * column[1] + column[2] * column[2]).sqrt();
    if !rig_scale.is_finite() || rig_scale <= 0.0 {
        return camera.z_far;
    }
    (camera.z_far / rig_scale).max(camera.z_near * 2.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::TransformComponent;

    #[test]
    fn far_plane_is_a_world_distance_under_a_scaled_rig() {
        let camera = CameraXRComponent::new(true);
        let rig = |s: f32| TransformComponent::new().with_scale(s, s, s).transform.model;
        assert_eq!(view_far(&camera, &rig(1.0)), camera.z_far);
        // A rig at a tenth of the size sees the same 100 m of world.
        assert!((view_far(&camera, &rig(0.1)) - camera.z_far / 0.1).abs() < 1e-2);
        // A degenerate rig keeps the authored far plane.
        assert_eq!(view_far(&camera, &rig(0.0)), camera.z_far);
    }
}
