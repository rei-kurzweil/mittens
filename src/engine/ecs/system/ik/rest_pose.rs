//! Rest-pose reads shared by avatar control and the IK solvers.
use crate::engine::ecs::component::{BoneRestPoseComponent, TransformComponent};
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::primitives::Transform;
use crate::utils::math::{mat4_identity, mat4_mul};

/// Read a bone's authored bind-pose local TRS via the `BoneRestPoseComponent`
/// sidecar that `GLTFSystem` stamps at node-spawn time.  Falls back to the
/// live `TransformComponent` (then to identity) for non-GLTF skeletons that
/// never had a rest-pose snapshot attached.
pub(crate) fn read_bone_rest_pose(
    world: &World,
    bone_id: ComponentId,
) -> ([f32; 3], [f32; 4], [f32; 3]) {
    if let Some(rest) = world
        .children_of(bone_id)
        .iter()
        .find_map(|&c| world.get_component_by_id_as::<BoneRestPoseComponent>(c))
    {
        return (rest.translation, rest.rotation, rest.scale);
    }
    world
        .get_component_by_id_as::<TransformComponent>(bone_id)
        .map(|t| {
            (
                t.transform.translation,
                t.transform.rotation,
                t.transform.scale,
            )
        })
        .unwrap_or(([0.0; 3], [0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]))
}

/// Compose `descendant`'s authored/rest transform relative to `ancestor`.
///
/// GLTF transforms use their immutable `BoneRestPoseComponent` snapshot.
/// Non-GLTF transforms fall back to their local model matrix. World matrices
/// are deliberately ignored because XR may update an ancestor before the
/// imported subtree has been propagated for the same frame.
pub(crate) fn rest_model_relative_to(
    world: &World,
    ancestor: ComponentId,
    descendant: ComponentId,
) -> Option<[[f32; 4]; 4]> {
    let mut transforms = Vec::new();
    let mut current = Some(descendant);
    let mut found_ancestor = false;

    while let Some(id) = current {
        if id == ancestor {
            found_ancestor = true;
            break;
        }
        if world
            .get_component_by_id_as::<TransformComponent>(id)
            .is_some()
        {
            transforms.push(id);
        }
        current = world.parent_of(id);
    }
    if !found_ancestor {
        return None;
    }

    transforms.reverse();
    Some(transforms.into_iter().fold(mat4_identity(), |model, id| {
        let local = if let Some(rest) = world
            .children_of(id)
            .iter()
            .find_map(|&child| world.get_component_by_id_as::<BoneRestPoseComponent>(child))
        {
            let mut transform = Transform::default();
            transform.translation = rest.translation;
            transform.rotation = rest.rotation;
            transform.scale = rest.scale;
            transform.recompute_model();
            transform.model
        } else {
            world
                .get_component_by_id_as::<TransformComponent>(id)
                .map(|transform| transform.transform.model)
                .unwrap_or_else(mat4_identity)
        };
        mat4_mul(model, local)
    }))
}
