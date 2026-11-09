//! Static spring targets for rigid rigs with captured rest poses and no animation.
//! Cache directions at bind time, then follow only the live external frame. Reading
//! last frame's simulated joint matrices would feed the bend back into its target.
use super::*;

#[derive(Debug, Clone)]
pub(super) struct RestTargets {
    frame: Option<ComponentId>,
    pub(super) directions: Vec<[f32; 3]>,
}

impl RestTargets {
    pub(super) fn bind(world: &World, owner: ComponentId, joints: &[JointConfig]) -> Option<Self> {
        if !world
            .get_component_by_id_as::<ArmatureComponent>(owner)?
            .rest_pose_only
        {
            return None;
        }
        let root = super::super::armature_scope::joint_root(world, owner).ok()?;
        let mut frame = world.parent_of(root);
        while let Some(id) = frame {
            if world
                .get_component_by_id_as::<TransformComponent>(id)
                .is_some()
            {
                break;
            }
            frame = world.parent_of(id);
        }
        let directions = joints
            .iter()
            .map(|joint| {
                let mut rotation = [0.0, 0.0, 0.0, 1.0];
                let mut cursor = Some(joint.parent_id);
                while cursor != frame {
                    let Some(id) = cursor else { break };
                    if let Some(transform) = world.get_component_by_id_as::<TransformComponent>(id)
                    {
                        let local = world
                            .children_of(id)
                            .iter()
                            .find_map(|&child| {
                                world.get_component_by_id_as::<BoneRestPoseComponent>(child)
                            })
                            .map(|rest| rest.rotation)
                            .unwrap_or(transform.transform.rotation);
                        rotation = quat_mul(local, rotation);
                    }
                    cursor = world.parent_of(id);
                }
                quat_rotate_vec3(rotation, joint.rest_direction)
            })
            .collect();
        Some(Self { frame, directions })
    }

    pub(super) fn rotation(&self, world: &World) -> [f32; 4] {
        self.frame
            .and_then(|id| world.get_component_by_id_as::<TransformComponent>(id))
            .map(|transform| mat_to_quat(transform.transform.matrix_world))
            .unwrap_or([0.0, 0.0, 0.0, 1.0])
    }
}
