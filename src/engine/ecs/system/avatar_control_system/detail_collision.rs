//! `CollisionDetail::Detailed`: sphere colliders on the torso bones, so a
//! pitched body still collides where the upright capsule does not reach.
//! They share the capsule's movement target, response, and enabled state.
use super::*;
use crate::engine::ecs::component::CollisionDetail;

/// Torso bones that carry a sphere. The head is inside the upright capsule.
const DETAIL_BONES: [HumanoidSlot; 3] = [
    HumanoidSlot::Hips,
    HumanoidSlot::Chest,
    HumanoidSlot::UpperChest,
];
/// Sphere radius as a fraction of the hip-joint spacing.
const RADIUS_PER_HIP_WIDTH: f32 = 0.65;

/// Create the torso spheres once the capsule and bone map exist.
pub(super) fn ensure_detail_spheres(
    id: ComponentId,
    world: &mut World,
    humanoid_maps: &mut HumanoidBoneMapSystem,
) {
    let Some(avatar) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
        return;
    };
    if avatar.collision_detail != CollisionDetail::Detailed
        || !avatar.detail_collidable_ids.is_empty()
    {
        return;
    }
    let Some(capsule) = avatar
        .capsule_collidable_id
        .and_then(|c| world.get_component_by_id_as::<CollidableComponent>(c))
    else {
        return;
    };
    let (target, response) = (capsule.movement_target_id, capsule.response);
    let Some(report) = humanoid_maps
        .request_for_avc(world, id)
        .and_then(|gltf| humanoid_maps.report(gltf))
    else {
        return;
    };
    let Some(bones) = DETAIL_BONES
        .iter()
        .map(|&slot| map_target(report, slot))
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    let (Some(left), Some(right)) = (
        map_target(report, HumanoidSlot::LeftUpperLeg),
        map_target(report, HumanoidSlot::RightUpperLeg),
    ) else {
        return;
    };
    let Some(hips) = bones.first().copied() else {
        return;
    };
    let width = [left, right]
        .map(|leg| rest_model_relative_to(world, hips, leg).map(|m| [m[3][0], m[3][1], m[3][2]]));
    let [Some(l), Some(r)] = width else {
        return;
    };
    let radius = RADIUS_PER_HIP_WIDTH
        * ((l[0] - r[0]).powi(2) + (l[1] - r[1]).powi(2) + (l[2] - r[2]).powi(2)).sqrt();
    if !radius.is_finite() || radius <= 1e-4 {
        return;
    }
    let mut collidables = Vec::with_capacity(bones.len());
    for bone in bones {
        let frame = world.add_component(TransformComponent::new());
        let serialize = world.add_component(SerializeComponent::off());
        let zone = world.add_component(ZoneComponent::sphere(radius));
        let mut collider = CollidableComponent::slide().with_runtime_movement_target(target);
        collider.response = response;
        collider.query_owner = Some(id);
        collider.enabled = false;
        let collidable = world.add_component(collider);
        let _ = world.set_parent(frame, Some(bone));
        let _ = world.set_parent(serialize, Some(frame));
        let _ = world.set_parent(zone, Some(frame));
        let _ = world.set_parent(collidable, Some(zone));
        collidables.push(collidable);
    }
    if let Some(avatar) = world.get_component_by_id_as_mut::<AvatarControlComponent>(id) {
        avatar.detail_collidable_ids = collidables;
    }
}

/// Mirror the capsule's routing and gate onto the torso spheres.
pub(super) fn sync_detail_spheres(
    id: ComponentId,
    world: &mut World,
    contact: Option<&mut super::super::static_contact_system::StaticContactSystem>,
    reset: bool,
) {
    let Some(avatar) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
        return;
    };
    if avatar.detail_collidable_ids.is_empty() {
        return;
    }
    let spheres = avatar.detail_collidable_ids.clone();
    let Some(capsule) = avatar
        .capsule_collidable_id
        .and_then(|c| world.get_component_by_id_as::<CollidableComponent>(c))
    else {
        return;
    };
    let (target, response, enabled) = (
        capsule.movement_target_id,
        capsule.response,
        capsule.enabled,
    );
    if reset && let Some(contact) = contact {
        for &sphere in &spheres {
            if let Some(zone) = world.parent_of(sphere) {
                contact.forget_zone(zone);
            }
        }
    }
    for sphere in spheres {
        if let Some(collider) = world.get_component_by_id_as_mut::<CollidableComponent>(sphere) {
            collider.movement_target_id = target;
            collider.response = response;
            collider.enabled = enabled;
        }
    }
}
