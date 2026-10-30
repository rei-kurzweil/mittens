use super::{RendererSettingsComponent, Value, arg_bool, arg_f32, arg_f32_arr};

pub(super) fn apply(
    settings: &mut RendererSettingsComponent,
    method: &str,
    args: &[Value],
) -> Result<(), String> {
    match method {
        "window_size" => {
            *settings =
                settings.with_window_size(arg_f32(args, 0)? as u32, arg_f32(args, 1)? as u32)
        }
        "transmission_depth_compare" => {
            *settings = settings.with_transmission_depth_compare(arg_bool(args, 0)?)
        }
        "desired_xr_resolution" => {
            let value = arg_f32_arr::<2>(args, 0)?;
            let resolution =
                RendererSettingsComponent::validate_xr_resolution(value.map(f64::from))?;
            *settings = settings.with_desired_xr_resolution(resolution);
        }
        _ => {}
    }
    Ok(())
}
