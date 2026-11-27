//! `CollisionDetail::Detailed`: sphere colliders on the torso bones, so a
//! pitched body still collides where the upright capsule does not reach.
//! They share the capsule's movement target, response, and enabled state.
//!
//! Detailed avatars also carry leg and foot obstacles in an overlay
//! CollisionContext of their own, so other avatars collide with their legs
//! rather than with the capsule: a small avatar can walk between a big one's
//! legs but not through them. The capsule then stops being an obstacle
//! (`mover_only`) while still moving its avatar.
use super::*;
use crate::engine::ecs::component::{CollisionContextComponent, CollisionDetail, ComponentRef};

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
        collider.horizontal_correction = true;
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

/// Leg spheres: the bone, the joint it points to, how far along the bone the
/// sphere sits, and its radius as a fraction of the hip-joint spacing. Each
/// foot gets one sphere at the ankle.
const LIMB_SPHERES: [(HumanoidSlot, Option<HumanoidSlot>, f32, f32); 10] = [
    (HumanoidSlot::LeftUpperLeg, Some(HumanoidSlot::LeftLowerLeg), 0.3, 0.42),
    (HumanoidSlot::LeftUpperLeg, Some(HumanoidSlot::LeftLowerLeg), 0.7, 0.36),
    (HumanoidSlot::LeftLowerLeg, Some(HumanoidSlot::LeftFoot), 0.3, 0.32),
    (HumanoidSlot::LeftLowerLeg, Some(HumanoidSlot::LeftFoot), 0.7, 0.28),
    (HumanoidSlot::LeftFoot, None, 0.0, 0.30),
    (HumanoidSlot::RightUpperLeg, Some(HumanoidSlot::RightLowerLeg), 0.3, 0.42),
    (HumanoidSlot::RightUpperLeg, Some(HumanoidSlot::RightLowerLeg), 0.7, 0.36),
    (HumanoidSlot::RightLowerLeg, Some(HumanoidSlot::RightFoot), 0.3, 0.32),
    (HumanoidSlot::RightLowerLeg, Some(HumanoidSlot::RightFoot), 0.7, 0.28),
    (HumanoidSlot::RightFoot, None, 0.0, 0.30),
];

/// Create the leg obstacles once the capsule and bone map exist.
pub(super) fn ensure_limb_obstacles(
    id: ComponentId,
    world: &mut World,
    humanoid_maps: &mut HumanoidBoneMapSystem,
) {
    let Some(avatar) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
        return;
    };
    if avatar.collision_detail != CollisionDetail::Detailed
        || !avatar.limb_collidable_ids.is_empty()
    {
        return;
    }
    let (Some(capsule), Some(model_root)) = (avatar.capsule_collidable_id, avatar.model_root_id)
    else {
        return;
    };
    let Some(report) = humanoid_maps
        .request_for_avc(world, id)
        .and_then(|gltf| humanoid_maps.report(gltf))
    else {
        return;
    };
    let (Some(hips), Some(left), Some(right)) = (
        map_target(report, HumanoidSlot::Hips),
        map_target(report, HumanoidSlot::LeftUpperLeg),
        map_target(report, HumanoidSlot::RightUpperLeg),
    ) else {
        return;
    };
    let width = [left, right]
        .map(|leg| rest_model_relative_to(world, hips, leg).map(|m| [m[3][0], m[3][1], m[3][2]]));
    let [Some(l), Some(r)] = width else {
        return;
    };
    let spacing = ((l[0] - r[0]).powi(2) + (l[1] - r[1]).powi(2) + (l[2] - r[2]).powi(2)).sqrt();
    if !spacing.is_finite() || spacing <= 1e-4 {
        return;
    }
    let mut placements = Vec::with_capacity(LIMB_SPHERES.len());
    for (slot, next, along, radius) in LIMB_SPHERES {
        let Some(bone) = map_target(report, slot) else {
            return;
        };
        let offset = match next {
            Some(next) => {
                let Some(joint) = map_target(report, next)
                    .and_then(|joint| rest_model_relative_to(world, bone, joint))
                else {
                    return;
                };
                [joint[3][0] * along, joint[3][1] * along, joint[3][2] * along]
            }
            None => [0.0; 3],
        };
        placements.push((bone, offset, radius * spacing));
    }
    // The context lives in the model's frame, so walking moves it without
    // refitting; each sphere's zone takes its frame from a point on its bone.
    let context = world.add_component(CollisionContextComponent::new().overlay(true));
    let serialize = world.add_component(SerializeComponent::off());
    let _ = world.set_parent(context, Some(model_root));
    let _ = world.set_parent(serialize, Some(context));
    let mut collidables = Vec::with_capacity(placements.len());
    for (bone, offset, radius) in placements {
        let frame =
            world.add_component(TransformComponent::new().with_position(offset[0], offset[1], offset[2]));
        let frame_serialize = world.add_component(SerializeComponent::off());
        let _ = world.set_parent(frame, Some(bone));
        let _ = world.set_parent(frame_serialize, Some(frame));
        let Some(guid) = world.get_component_record(frame).map(|record| record.guid) else {
            continue;
        };
        let mut zone = ZoneComponent::sphere(radius);
        zone.frame_source = Some(ComponentRef::Guid(guid));
        let zone = world.add_component(zone);
        let mut collider = CollidableComponent::obstacle();
        collider.query_owner = Some(id);
        let collidable = world.add_component(collider);
        let _ = world.set_parent(zone, Some(context));
        let _ = world.set_parent(collidable, Some(zone));
        collidables.push(collidable);
    }
    if let Some(capsule) = world.get_component_by_id_as_mut::<CollidableComponent>(capsule) {
        capsule.mover_only = true;
    }
    if let Some(avatar) = world.get_component_by_id_as_mut::<AvatarControlComponent>(id) {
        avatar.limb_collidable_ids = collidables;
    }
}

/// Leg obstacles follow the capsule's enabled state.
pub(super) fn sync_limb_obstacles(id: ComponentId, world: &mut World) {
    let Some(avatar) = world.get_component_by_id_as::<AvatarControlComponent>(id) else {
        return;
    };
    if avatar.limb_collidable_ids.is_empty() {
        return;
    }
    let limbs = avatar.limb_collidable_ids.clone();
    let Some(enabled) = avatar
        .capsule_collidable_id
        .and_then(|c| world.get_component_by_id_as::<CollidableComponent>(c))
        .map(|capsule| capsule.enabled)
    else {
        return;
    };
    for limb in limbs {
        if let Some(collider) = world.get_component_by_id_as_mut::<CollidableComponent>(limb)
            && collider.enabled != enabled
        {
            collider.enabled = enabled;
        }
    }
}
