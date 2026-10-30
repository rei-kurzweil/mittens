//! Walking configuration bindings, including the existing AVC spelling.
use super::{AvatarControlComponent, ComponentId, Value, World, arg_bool, arg_f32};
use crate::engine::ecs::component::WalkingIkComponent;

pub(super) fn create(
    world: &mut World,
    ctor: Option<&str>,
    args: &[Value],
) -> Result<ComponentId, String> {
    let mut config = WalkingIkComponent::new();
    if let Some(method) = ctor {
        apply(&mut config, method, args)?;
    }
    Ok(world.add_component(config))
}

pub(super) fn apply(
    config: &mut WalkingIkComponent,
    method: &str,
    args: &[Value],
) -> Result<(), String> {
    *config = match method {
        "enabled" => config.with_enabled(arg_bool(args, 0)?),
        "stride" => config.with_stride(arg_f32(args, 0)?)?,
        "step_duration" => config.with_step_duration(arg_f32(args, 0)?)?,
        "step_height" => config.with_step_height(arg_f32(args, 0)?)?,
        "step_scale" => config.with_step_scale(arg_f32(args, 0)?)?,
        _ => return Err(format!("unknown WalkingIK method '.{method}'")),
    };
    Ok(())
}

pub(super) fn apply_legacy(
    config: &mut AvatarControlComponent,
    method: &str,
    args: &[Value],
) -> Result<bool, String> {
    *config = match method {
        "walking_enabled" => config.clone().with_walking_enabled(arg_bool(args, 0)?),
        "walking_stride" => config.clone().with_walking_stride(arg_f32(args, 0)?)?,
        "walking_step_duration" => config
            .clone()
            .with_walking_step_duration(arg_f32(args, 0)?)?,
        "walking_step_height" => config.clone().with_walking_step_height(arg_f32(args, 0)?)?,
        _ => return Ok(false),
    };
    Ok(true)
}
