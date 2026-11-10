//! Holding from the raw controller grip without reparenting the pointer or avatar bones.
use super::super::pointer_system::nearest_ancestor_transform;
#[cfg(test)]
#[path = "controller_grab_pose_tests.rs"]
mod tests;
use crate::engine::ecs::component::{ControllerXRComponent, TransformComponent};
use crate::engine::ecs::{ComponentId, World};
use crate::engine::transform::TransformTrs;
use crate::utils::math::{
    mat_to_quat, mat4_inverse, mat4_mul, quat_conjugate, quat_mul, quat_nlerp, quat_rotate_vec3,
    vec3_add, vec3_sub,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct ControllerGripHold {
    controller: ComponentId,
    offset_translation: [f32; 3],
    offset_rotation: [f32; 4],
}
impl ControllerGripHold {
    pub(super) fn start(
        world: &World,
        pointer: ComponentId,
        target_world: [[f32; 4]; 4],
    ) -> Option<(Self, ComponentId)> {
        let mut ancestor = world.parent_of(pointer);
        while let Some(id) = ancestor {
            if let Some(controller) = world.get_component_by_id_as::<ControllerXRComponent>(id) {
                let pose = controller.get_grip_pose(world)?;
                let parent = world
                    .parent_of(id)
                    .and_then(|id| nearest_ancestor_transform(world, id))
                    .or_else(|| {
                        world.children_of(id).iter().copied().find(|&id| {
                            world
                                .get_component_by_id_as::<TransformComponent>(id)
                                .is_some()
                        })
                    })?;
                let position = [target_world[3][0], target_world[3][1], target_world[3][2]];
                let inverse_rotation = quat_conjugate(pose.rotation_quat_xyzw);
                return Some((
                    Self {
                        controller: id,
                        offset_translation: quat_rotate_vec3(
                            inverse_rotation,
                            vec3_sub(position, pose.translation),
                        ),
                        offset_rotation: quat_mul(inverse_rotation, mat_to_quat(target_world)),
                    },
                    parent,
                ));
            }
            ancestor = world.parent_of(id);
        }
        None
    }
    pub(super) fn is_present(&self, world: &World) -> bool {
        world
            .get_component_by_id_as::<ControllerXRComponent>(self.controller)
            .is_some()
    }
    pub(super) fn tick(
        &mut self,
        world: &World,
        parent: ComponentId,
        alpha: f32,
    ) -> Option<TransformTrs> {
        let pose = world
            .get_component_by_id_as::<ControllerXRComponent>(self.controller)?
            .get_grip_pose(world)?;
        self.offset_translation = self.offset_translation.map(|v| v * (1.0 - alpha));
        self.offset_rotation = quat_nlerp(self.offset_rotation, [0., 0., 0., 1.], alpha);
        if self.offset_translation.iter().map(|v| v * v).sum::<f32>() < 1e-6 {
            self.offset_translation = [0.; 3];
        }
        let desired = TransformTrs::new(
            vec3_add(
                pose.translation,
                quat_rotate_vec3(pose.rotation_quat_xyzw, self.offset_translation),
            ),
            quat_mul(pose.rotation_quat_xyzw, self.offset_rotation),
            [1.; 3],
        );
        let parent_world = world
            .get_component_by_id_as::<TransformComponent>(parent)?
            .transform
            .matrix_world;
        TransformTrs::from_matrix(mat4_mul(
            mat4_inverse(parent_world)?,
            desired.to_matrix().ok()?,
        ))
        .ok()
    }
}
