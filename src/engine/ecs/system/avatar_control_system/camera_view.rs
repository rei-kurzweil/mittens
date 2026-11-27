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

/// The third-person camera's place on a sphere around the player: world yaw
/// and pitch of the direction from the player to the camera. Inactive until
/// the first orbit input after third person turns on; until then the camera
/// sits behind the player's movement root.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct ThirdPersonOrbit {
    pub(crate) active: bool,
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
}

const DEFAULT_PITCH: f32 = 0.29;
const MIN_PITCH: f32 = -0.35;
const MAX_PITCH: f32 = 1.2;
/// Orbit radius in player heights (about 4 m for a 1.6 m player).
const DISTANCE_PER_HEIGHT: f32 = 2.5;
/// The camera's collision sphere and the closest it comes to the player.
const CAMERA_RADIUS: f32 = 0.25;
const MIN_DISTANCE: f32 = 0.35;

/// Behind the movement root (its +Z), slightly above.
fn behind(world: &World, root: ComponentId) -> (f32, f32) {
    let yaw = TransformSystem::world_model(world, root)
        .map(|m| m[2][0].atan2(m[2][2]))
        .filter(|yaw| yaw.is_finite())
        .unwrap_or(0.0);
    (yaw, DEFAULT_PITCH)
}

/// Orbit the third-person camera by stick input (x: around, y: up and down;
/// pushing right turns the view right, pushing up looks up).
pub(crate) fn orbit_third_person(world: &mut World, owner: ComponentId, stick: [f32; 2], speed: f32) {
    let Some(avatar) = world.get_component_by_id_as::<AvatarControlComponent>(owner) else {
        return;
    };
    let mut orbit = avatar.third_person_orbit;
    if !orbit.active {
        let Some(root) = avatar.camera_follow_root else {
            return;
        };
        (orbit.yaw, orbit.pitch) = behind(world, root);
        orbit.active = true;
    }
    orbit.yaw -= stick[0] * speed;
    orbit.pitch = (orbit.pitch - stick[1] * speed).clamp(MIN_PITCH, MAX_PITCH);
    if let Some(avatar) = world.get_component_by_id_as_mut::<AvatarControlComponent>(owner) {
        avatar.third_person_orbit = orbit;
    }
}

/// The avatar shown in third person, when `camera` is the active view of one.
pub(crate) fn third_person_owner(world: &World, owner: Option<ComponentId>) -> Option<ComponentId> {
    let owner = owner?;
    world
        .get_component_by_id_as::<AvatarControlComponent>(owner)
        .is_some_and(|avatar| avatar.third_person)
        .then_some(owner)
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
    let (yaw, pitch) = if avatar.third_person_orbit.active {
        (avatar.third_person_orbit.yaw, avatar.third_person_orbit.pitch)
    } else {
        behind(world, root)
    };
    let back = [yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos()];
    let distance = unblocked_distance(world, root, focus, back, DISTANCE_PER_HEIGHT * height, scale);
    Some(frame_avatar(focus, back, distance, scale))
}

/// How far out from `focus` along `back` the camera's sphere fits: the
/// nearest solid hit by its centre and four edge rays, less its radius. The
/// player's own colliders (under `root`) never block it.
fn unblocked_distance(
    world: &World,
    root: ComponentId,
    focus: [f32; 3],
    back: [f32; 3],
    full: f32,
    scale: f32,
) -> f32 {
    use crate::engine::ecs::system::world_raycast::raycast_filtered;
    let radius = CAMERA_RADIUS * scale;
    let right = vec3_normalize(vec3_cross([0.0, 1.0, 0.0], back));
    let up = vec3_normalize(vec3_cross(back, right));
    let reach = full + radius;
    let own = |zone: ComponentId| {
        let mut node = Some(zone);
        while let Some(id) = node {
            if id == root {
                return false;
            }
            node = world.parent_of(id);
        }
        true
    };
    let nearest = [[0.0; 3], right, vec3_scale(right, -1.0), up, vec3_scale(up, -1.0)]
        .into_iter()
        .filter_map(|edge| {
            let origin = vec3_add(focus, vec3_scale(edge, radius));
            raycast_filtered(world, origin, back, reach, own)
        })
        .fold(reach, f32::min);
    (nearest - radius).clamp(MIN_DISTANCE * scale, full)
}

fn frame_avatar(focus: [f32; 3], back: [f32; 3], distance: f32, scale: f32) -> TransformMatrix {
    let eye = vec3_add(focus, vec3_scale(back, distance));
    let right = vec3_scale(vec3_normalize(vec3_cross([0.0, 1.0, 0.0], back)), scale);
    let up = vec3_scale(vec3_normalize(vec3_cross(back, right)), scale);
    let back = vec3_scale(back, scale);
    [
        [right[0], right[1], right[2], 0.0],
        [up[0], up[1], up[2], 0.0],
        [back[0], back[1], back[2], 0.0],
        [eye[0], eye[1], eye[2], 1.0],
    ]
}
