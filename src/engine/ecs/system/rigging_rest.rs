//! Rest-pose setup for rigid transform chains, shared with weighted armatures.
use super::*;

pub(super) fn capture_joints(world: &mut World, joints: &[ComponentId]) {
    for &joint in joints {
        let transform = world
            .get_component_by_id_as::<TransformComponent>(joint)
            .unwrap()
            .transform;
        let existing = world.children_of(joint).iter().copied().find(|&id| {
            world
                .get_component_by_id_as::<BoneRestPoseComponent>(id)
                .is_some()
        });
        let snapshot =
            BoneRestPoseComponent::new(transform.translation, transform.rotation, transform.scale);
        if let Some(id) = existing {
            *world
                .get_component_by_id_as_mut::<BoneRestPoseComponent>(id)
                .unwrap() = snapshot;
        } else {
            let id = world.add_component(snapshot);
            world
                .add_child(joint, id)
                .expect("rest pose belongs to an existing joint");
        }
    }
}

impl RiggingSystem {
    pub(super) fn capture_rest_pose(
        &mut self,
        world: &mut World,
        request: ComponentId,
    ) -> Result<(), String> {
        let config = world
            .get_component_by_id_as::<ArmatureComponent>(request)
            .ok_or("missing Armature")?
            .clone();
        let root = super::super::armature_scope::joint_root(world, request)?;
        let joints = super::super::armature_scope::joint_transforms(world, request)?;
        let joint_markers = nodes(world, root)
            .into_iter()
            .filter(|&id| world.get_component_by_id_as::<JointComponent>(id).is_some())
            .collect();
        capture_joints(world, &joints);
        self.bound.insert(
            request,
            BoundRig {
                revision: config.request_revision,
                owner: request,
                joints,
                joint_markers,
                meshes: Vec::new(),
            },
        );
        self.initialized.push(request);
        Ok(())
    }
}
