//! Opt-in translation springs for rigid accessories. Existing hair keeps fixed lengths.
use super::*;
use crate::utils::math::{mat4_inverse, mat4_mul, mat4_mul_vec4};

pub(super) fn validate(
    world: &World,
    owner: ComponentId,
    chain: &SpringBoneComponent,
    joints: &[JointConfig],
) -> Result<(), String> {
    if !world
        .get_component_by_id_as::<ArmatureComponent>(owner)
        .is_some_and(|a| a.rest_pose_only)
    {
        return Err(String::from(
            "allow_stretch requires a captured rest-only Armature",
        ));
    }
    if chain.world_collision || chain.avatar_collision || !chain.colliders.is_empty() {
        return Err(String::from(
            "allow_stretch currently requires world_collision(false), avatar_collision(false), and no colliders",
        ));
    }
    if joints
        .windows(2)
        .any(|pair| pair[1].parent_id != pair[0].id)
    {
        return Err(String::from(
            "allow_stretch requires directly nested joint transforms",
        ));
    }
    Ok(())
}

pub(super) fn step(world: &World, state: &mut BoundChain) {
    let targets = state
        .rest_targets
        .as_ref()
        .expect("validated rest-only rig");
    let rotation = targets.rotation(world);
    for index in 0..state.joints.len() {
        let head = if index == 0 {
            pos(world, state.joints[0].id).unwrap_or(state.current[0])
        } else {
            state.current[index - 1]
        };
        let target = vec3_add(
            head,
            vec3_scale(
                quat_rotate_vec3(rotation, targets.directions[index]),
                state.lengths[index],
            ),
        );
        let joint = &state.joints[index];
        let inertia = vec3_scale(
            vec3_sub(state.current[index], state.previous[index]),
            1.0 - joint.drag,
        );
        let force = vec3_add(
            vec3_scale(
                vec3_sub(target, state.current[index]),
                joint.stiffness * STEP,
            ),
            vec3_scale(joint.gravity, STEP * STEP),
        );
        state.previous[index] = state.current[index];
        // Deliberately do not normalize to a fixed segment length.
        state.current[index] = vec3_add(vec3_add(state.current[index], inertia), force);
    }
}

pub(super) fn restore_translation(world: &mut World, joint: ComponentId) {
    let translation = world
        .children_of(joint)
        .iter()
        .find_map(|&id| world.get_component_by_id_as::<BoneRestPoseComponent>(id))
        .map(|rest| rest.translation);
    if let (Some(translation), Some(t)) = (
        translation,
        world.get_component_by_id_as_mut::<TransformComponent>(joint),
    ) {
        t.transform.translation = translation;
    }
}

pub(super) fn apply(world: &mut World, state: &BoundChain, rest_weight: f32) {
    let mut parent_world = None;
    for (index, joint) in state.joints.iter().enumerate() {
        let parent = if index > 0 {
            parent_world
        } else {
            world
                .get_component_by_id_as::<TransformComponent>(joint.parent_id)
                .map(|t| t.transform.matrix_world)
        };
        let Some(parent) = parent else { return };
        if index > 0 {
            let Some(inverse) = mat4_inverse(parent) else {
                return;
            };
            let p = state.current[index - 1];
            let local = mat4_mul_vec4(inverse, [p[0], p[1], p[2], 1.0]);
            let Some(rest) = world
                .children_of(joint.id)
                .iter()
                .find_map(|&id| world.get_component_by_id_as::<BoneRestPoseComponent>(id))
                .map(|pose| pose.translation)
            else {
                return;
            };
            let t = world
                .get_component_by_id_as_mut::<TransformComponent>(joint.id)
                .unwrap();
            t.transform.translation = std::array::from_fn(|axis| {
                local[axis] * (1.0 - rest_weight) + rest[axis] * rest_weight
            });
        }
        let t = world
            .get_component_by_id_as_mut::<TransformComponent>(joint.id)
            .unwrap();
        t.transform.recompute_model();
        parent_world = Some(mat4_mul(parent, t.transform.model));
    }
}
