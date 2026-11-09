//! Authored joint discovery shared by binding and secondary motion.
use crate::engine::ecs::component::component_ref::{QueryRootMode, resolve_component_ref};
use crate::engine::ecs::component::{ArmatureComponent, JointComponent, TransformComponent};
use crate::engine::ecs::{ComponentId, World};

pub(crate) fn joint_root(world: &World, owner: ComponentId) -> Result<ComponentId, String> {
    let config = world
        .get_component_by_id_as::<ArmatureComponent>(owner)
        .ok_or("missing Armature")?;
    let reference = config
        .joints
        .as_ref()
        .ok_or("Armature requires a joints reference")?;
    let root = resolve_component_ref(
        world,
        reference,
        Some(owner),
        QueryRootMode::ParentScope { levels_up: 1 },
    )
    .ok_or("joint tree reference is not ready")?;
    if world
        .get_component_by_id_as::<TransformComponent>(root)
        .is_none()
    {
        return Err("joint tree root must be a Transform".into());
    }
    Ok(root)
}
pub(crate) fn joint_transforms(
    world: &World,
    owner: ComponentId,
) -> Result<Vec<ComponentId>, String> {
    let root = joint_root(world, owner)?;
    let mut joints = Vec::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if world
            .get_component_by_id_as::<ArmatureComponent>(id)
            .is_some()
        {
            return Err("joint tree contains an Armature".into());
        }
        if world.get_component_by_id_as::<JointComponent>(id).is_some() {
            let mut parent = world.parent_of(id);
            let joint = loop {
                let p = parent.ok_or("Joint requires a Transform ancestor")?;
                if world
                    .get_component_by_id_as::<TransformComponent>(p)
                    .is_some()
                {
                    break p;
                }
                parent = world.parent_of(p);
            };
            if joints.contains(&joint) {
                return Err("duplicate Joint markers on one transform".into());
            }
            joints.push(joint);
        }
        stack.extend(world.children_of(id).iter().rev().copied());
    }
    if joints.is_empty() {
        return Err("joint tree has no Joint markers".into());
    }
    Ok(joints)
}
