//! MMS XR locomotion configuration, separate from the runtime controller.
use super::{Value, arg_bool, arg_f32, arg_str};
use crate::engine::ecs::component::{InputXRGamepadComponent, XrHandPreference};
pub(super) fn apply(
    input: &mut InputXRGamepadComponent,
    method: &str,
    args: &[Value],
) -> Result<(), String> {
    match method {
        "enabled" => input.enabled = arg_bool(args, 0)?,
        "enable_flying" => input.enable_flying = arg_bool(args, 0)?,
        "hand" => {
            input.hand = match arg_str(args, 0)?.to_ascii_lowercase().as_str() {
                "default" => XrHandPreference::Default,
                "left" => XrHandPreference::Left,
                "right" => XrHandPreference::Right,
                "either" => XrHandPreference::Either,
                hand => return Err(format!("unknown XrHandPreference: {hand}")),
            }
        }
        "locomotion" => {
            input.locomotion = if args.is_empty() {
                true
            } else {
                arg_bool(args, 0)?
            }
        }
        "speed" => input.speed = arg_f32(args, 0)?,
        "deadzone" => input.deadzone = arg_f32(args, 0)?,
        _ => return Err(format!("InputXRGamepad: unknown builder '{method}'")),
    }
    Ok(())
}
