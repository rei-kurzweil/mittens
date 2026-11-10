//! Shared constructor/builder and imperative MMS gravity mode handling.
use super::{ComponentId, Value, World, arg_bool, arg_f32};
use crate::engine::ecs::component::{GravityComponent, GravityMode};
pub(super) fn apply(
    gravity: &mut GravityComponent,
    method: &str,
    args: &[Value],
) -> Result<(), String> {
    match method {
        "enabled" | "set_enabled" => {
            if args.len() != 1 {
                return Err(format!("{method}(): expected one boolean argument"));
            }
            gravity.set_enabled(arg_bool(args, 0)?);
        }
        "coefficient" => gravity
            .set_coefficient(arg_f32(args, 0)?)
            .map_err(str::to_owned)?,
        "on" | "off" | "auto" => {
            if !args.is_empty() {
                return Err(format!("{method}(): expected no arguments"));
            }
            gravity.set_mode(match method {
                "on" => GravityMode::On,
                "off" => GravityMode::Off,
                _ => GravityMode::Auto,
            });
        }
        _ => return Err(format!("Gravity: unknown builder '{method}'")),
    }
    Ok(())
}
pub(crate) fn call(
    world: &mut World,
    id: ComponentId,
    method: &str,
    args: &[Value],
) -> Result<Value, String> {
    let gravity = world
        .get_component_by_id_as_mut::<GravityComponent>(id)
        .ok_or("not a Gravity component")?;
    apply(gravity, method, args)?;
    Ok(Value::Null)
}
