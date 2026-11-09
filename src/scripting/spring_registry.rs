//! Secondary-motion component builder implementation.
use super::*;

pub(super) fn apply(
    world: &mut World,
    id: ComponentId,
    method: &str,
    args: &[Value],
) -> Result<(), String> {
    let spring_center = if method == "center" {
        Some(arg_component_ref(world, args, 0)?)
    } else {
        None
    };
    let spring_colliders = if method == "colliders" {
        Some(arg_component_ref_vec(world, args, 0)?)
    } else {
        None
    };
    if let Some(constraint) = world.get_component_by_id_as_mut::<ReturnToRestWhenStillComponent>(id)
    {
        match method {
            "motion_threshold" => constraint.motion_threshold = arg_f32(args, 0)?.max(0.0),
            "still_for" => constraint.still_for = arg_f32(args, 0)?.max(0.0),
            _ => return Err(format!("unknown ReturnToRestWhenStill method '.{method}'")),
        }
        return Ok(());
    }
    if let Some(chain) = world.get_component_by_id_as_mut::<SpringBoneComponent>(id) {
        match method {
            "center" => chain.center = spring_center,
            "enabled" => chain.enabled = arg_bool(args, 0)?,
            "allow_stretch" => chain.allow_stretch = arg_bool(args, 0)?,
            "virtual_end_length_ratio" => {
                chain.virtual_end_length_ratio = Some(arg_f32(args, 0)?.max(0.0))
            }
            "stiffness" => chain.stiffness = arg_f32(args, 0)?.max(0.0),
            "drag_force" => chain.drag_force = arg_f32(args, 0)?.clamp(0.0, 1.0),
            "gravity" => {
                chain.gravity_power = arg_f32(args, 0)?;
                chain.gravity_dir = if args.len() == 2 {
                    val_as_f32_array::<3>(&args[1])?
                } else {
                    [arg_f32(args, 1)?, arg_f32(args, 2)?, arg_f32(args, 3)?]
                };
            }
            "colliders" => chain.colliders = spring_colliders.unwrap_or_default(),
            "hit_radius" => chain.hit_radius = arg_f32(args, 0)?.max(0.0),
            "world_collision" => chain.world_collision = arg_bool(args, 0)?,
            "avatar_collision" => chain.avatar_collision = arg_bool(args, 0)?,
            _ => {}
        }
        return Ok(());
    }
    if let Some(joint) = world.get_component_by_id_as_mut::<SpringJointComponent>(id) {
        match method {
            "stiffness" => joint.stiffness = arg_f32(args, 0)?.max(0.0),
            "drag_force" => joint.drag_force = arg_f32(args, 0)?.clamp(0.0, 1.0),
            "gravity" => {
                joint.gravity_power = arg_f32(args, 0)?;
                joint.gravity_dir = if args.len() == 2 {
                    val_as_f32_array::<3>(&args[1])?
                } else {
                    [arg_f32(args, 1)?, arg_f32(args, 2)?, arg_f32(args, 3)?]
                };
            }
            _ => {}
        }
        return Ok(());
    }
    Err(format!("unsupported spring component method {method}"))
}
