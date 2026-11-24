//! Render camera framing for AvatarControl.
#[cfg(test)]
#[path = "camera_view_tests.rs"]
mod tests;

use super::automatic_avc_movement_target;
use crate::engine::ecs::component::{AvatarControlComponent, Camera3DComponent, CameraXRComponent};
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::transform::TransformMatrix;
use crate::utils::math::{vec3_add, vec3_cross, vec3_len, vec3_normalize, vec3_scale};

pub(super) fn bind_camera_views(
    world: &mut World,
    owner: ComponentId,
    cameras: &[(ComponentId, [f32; 3], bool)],
) {
    if cameras.is_empty() {
        return;
    }
    let root = automatic_avc_movement_target(world, owner);
    if let Some(avatar) = world.get_component_by_id_as_mut::<AvatarControlComponent>(owner) {
        avatar.camera_follow_root = root;
    }
    for &(mount, _, _) in cameras {
        if bind_camera(world, owner, mount) {
            continue;
        }
        for camera in world.children_of(mount).to_vec() {
            bind_camera(world, owner, camera);
        }
    }
}

fn bind_camera(world: &mut World, owner: ComponentId, camera: ComponentId) -> bool {
    if let Some(camera) = world.get_component_by_id_as_mut::<Camera3DComponent>(camera) {
        camera.avatar_view_owner = Some(owner);
    } else if let Some(camera) = world.get_component_by_id_as_mut::<CameraXRComponent>(camera) {
        camera.avatar_view_owner = Some(owner);
    } else {
        return false;
    }
    true
}

pub(in crate::engine::ecs::system) fn third_person_model(
    world: &World,
    owner: Option<ComponentId>,
) -> Option<TransformMatrix> {
    let owner = owner?;
    let avatar = world.get_component_by_id_as::<AvatarControlComponent>(owner)?;
    if !avatar.third_person {
        return None;
    }
    let root = avatar.camera_follow_root?;
    let origin = TransformSystem::world_position(world, root)?;
    // A scaled rig (a tiny player) is framed at its own scale, eye spacing
    // included, so the view keeps the size the first-person view has.
    let scale = TransformSystem::world_model(world, root)
        .map(|m| vec3_len([m[1][0], m[1][1], m[1][2]]))
        .filter(|s| s.is_finite() && *s > 0.0)
        .unwrap_or(1.0);
    let height = avatar.model_root_local_y.abs().clamp(1.0, 3.0) * scale;
    // Implicit desktop rigs use their driven head transform as the movement
    // target. An explicit target or an XR locomotion root is at the feet.
    let head_origin = world.parent_of(owner) == Some(root) && avatar.movement_target.is_none();
    let focus_y = if head_origin { -0.45 } else { 0.55 };
    let focus = vec3_add(origin, [0.0, focus_y * height, 0.0]);
    Some(frame_avatar(focus, height, scale))
}

fn frame_avatar(focus: [f32; 3], height: f32, scale: f32) -> TransformMatrix {
    let offset = [3.2, 1.15, 2.2];
    let eye = vec3_add(focus, vec3_scale(offset, height / 1.6));
    let back = vec3_scale(vec3_normalize(offset), scale);
    let right = vec3_scale(vec3_normalize(vec3_cross([0.0, 1.0, 0.0], back)), scale);
    let up = vec3_scale(vec3_normalize(vec3_cross(back, right)), scale);
    [
        [right[0], right[1], right[2], 0.0],
        [up[0], up[1], up[2], 0.0],
        [back[0], back[1], back[2], 0.0],
        [eye[0], eye[1], eye[2], 1.0],
    ]
}
