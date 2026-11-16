//! Rig topology helpers: owning inputs, ancestry, model root, camera anchor.
use super::*;

pub(super) fn refresh_map_camera_anchor(
    avc_id: ComponentId,
    world: &mut World,
    humanoid_maps: &mut HumanoidBoneMapSystem,
    emit: &mut dyn SignalEmitter,
) {
    let Some(gltf) = humanoid_maps.request_for_avc(world, avc_id) else {
        return;
    };
    let Some(report) = humanoid_maps.report(gltf) else {
        return;
    };
    let (old_anchor, seen_generation) = world
        .get_component_by_id_as::<AvatarControlComponent>(avc_id)
        .map(|avc| (avc.splice_camera_bone, avc.humanoid_map_generation))
        .unwrap_or((None, 0));
    if seen_generation == report.generation {
        return;
    }
    let new_anchor = map_target(report, HumanoidSlot::CameraAnchor)
        .or_else(|| map_target(report, HumanoidSlot::Head));
    if let (Some(old), Some(new)) = (old_anchor, new_anchor) {
        if old != new {
            let camera_paths: Vec<_> = world
                .children_of(old)
                .iter()
                .copied()
                .filter(|child| subtree_has_camera(world, *child))
                .collect();
            for path in camera_paths {
                emit_attach(emit, new, path);
            }
        }
    }
    if let Some(avc) = world.get_component_by_id_as_mut::<AvatarControlComponent>(avc_id) {
        if let Some(constraint) = &mut avc.head_neck_body_constraint {
            if constraint.matches_map(
                map_target(report, HumanoidSlot::Head),
                map_target(report, HumanoidSlot::Neck),
            ) {
                constraint.map_generation = report.generation;
            } else {
                avc.head_neck_body_bind_error =
                    Some("mapped head/neck changed; avatar requires reinitialization");
            }
        }
        avc.splice_camera_bone = new_anchor;
        avc.humanoid_map_gltf = Some(gltf);
        avc.humanoid_map_generation = report.generation;
    }
}

pub(super) fn subtree_has_camera(world: &World, root: ComponentId) -> bool {
    let mut pending = vec![root];
    while let Some(component) = pending.pop() {
        if world
            .get_component_by_id_as::<Camera3DComponent>(component)
            .is_some()
            || world
                .get_component_by_id_as::<CameraXRComponent>(component)
                .is_some()
        {
            return true;
        }
        pending.extend(world.children_of(component).iter().copied());
    }
    false
}

pub(super) fn model_root(world: &World, avc: ComponentId) -> Option<ComponentId> {
    world
        .get_component_by_id_as::<AvatarControlComponent>(avc)
        .and_then(|avatar| avatar.model_root_id)
        .filter(|&root| {
            world
                .get_component_by_id_as::<TransformComponent>(root)
                .is_some()
        })
        .or_else(|| {
            world.children_of(avc).iter().copied().find(|&child| {
                world
                    .get_component_by_id_as::<TransformComponent>(child)
                    .is_some()
            })
        })
}

pub(super) fn bind_shared_rig(
    world: &mut World,
    id: ComponentId,
    root: ComponentId,
    report: &HumanoidBoneMapReport,
) {
    let avatar = world
        .get_component_by_id_as_mut::<AvatarControlComponent>(id)
        .unwrap();
    avatar.model_root_id = Some(root);
    avatar.humanoid_map_gltf = Some(report.owning_gltf);
    avatar.humanoid_map_generation = report.generation;
}

pub(super) fn owning_pose_input(world: &World, mut id: ComponentId) -> bool {
    loop {
        if world
            .get_component_by_id_as::<crate::engine::ecs::component::PoseInputComponent>(id)
            .is_some()
        {
            return true;
        }
        let Some(parent) = world.parent_of(id) else {
            return false;
        };
        id = parent;
    }
}

pub(super) fn owning_desktop_input(world: &World, mut id: ComponentId) -> Option<ComponentId> {
    loop {
        if world.get_component_by_id_as::<InputComponent>(id).is_some() {
            return Some(id);
        }
        id = world.parent_of(id)?;
    }
}

pub(super) fn owning_input_xr(world: &World, mut id: ComponentId) -> Option<ComponentId> {
    loop {
        if world
            .get_component_by_id_as::<InputXRComponent>(id)
            .is_some()
        {
            return Some(id);
        }
        id = world.parent_of(id)?;
    }
}

pub(super) fn uniform_world_scale(world: &World, transform: ComponentId) -> Option<f32> {
    let matrix = world
        .get_component_by_id_as::<TransformComponent>(transform)?
        .transform
        .matrix_world;
    let trs = crate::engine::transform::TransformTrs::from_matrix(matrix).ok()?;
    let scale = trs.scale;
    (scale.iter().all(|value| value.is_finite() && *value > 1e-6)
        && (scale[0] - scale[1]).abs() < 1e-4 * scale[0]
        && (scale[0] - scale[2]).abs() < 1e-4 * scale[0])
        .then_some(scale[0])
}

pub(super) fn ancestor_or_self(
    world: &World,
    ancestor: ComponentId,
    mut descendant: ComponentId,
) -> bool {
    loop {
        if ancestor == descendant {
            return true;
        }
        let Some(parent) = world.parent_of(descendant) else {
            return false;
        };
        descendant = parent;
    }
}

pub(super) fn map_target(
    report: &HumanoidBoneMapReport,
    slot: HumanoidSlot,
) -> Option<ComponentId> {
    report.target(slot).map(|target| target.component)
}

pub(super) fn emit_attach(emit: &mut dyn SignalEmitter, parent: ComponentId, child: ComponentId) {
    emit.push_intent_now(
        parent,
        IntentValue::Attach {
            parent: parent,
            child,
        },
    );
}

pub(super) fn update_local_rotation(
    world: &World,
    emit: &mut dyn SignalEmitter,
    component_id: ComponentId,
    rotation: [f32; 4],
) {
    let Some(transform) = world.get_component_by_id_as::<TransformComponent>(component_id) else {
        return;
    };
    if transform.transform.rotation == rotation {
        return;
    }
    let translation = transform.transform.translation;
    let scale = transform.transform.scale;
    emit.push_intent_now(
        component_id,
        IntentValue::UpdateTransform {
            component_id: component_id,
            translation,
            rotation_quat_xyzw: rotation,
            scale,
        },
    );
}
