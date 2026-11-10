//! Eye tracking and provider-independent idle-blink authoring.
use super::*;
pub(super) fn apply(
    world: &mut World,
    id: ComponentId,
    method: &str,
    args: &[Value],
) -> Result<(), String> {
    if let Some(avatar) = world.get_component_by_id_as_mut::<AvatarControlComponent>(id) {
        match method {
            "idle_blink" => avatar.idle_blink = arg_bool(args, 0)?,
            "blink_interval" => {
                let v = arg_f32(args, 0)?;
                if !v.is_finite() || v < 0.3 {
                    return Err("blink_interval must be at least 0.3 seconds".into());
                }
                avatar.blink_interval = v;
            }
            "blink_duration" => {
                let v = arg_f32(args, 0)?;
                if !v.is_finite() || v < 0.02 {
                    return Err("blink_duration must be at least 0.02 seconds".into());
                }
                avatar.blink_duration = v;
            }
            _ => return Err(format!("unknown blink builder {method}")),
        }
        return Ok(());
    }
    if let Some(tracker) = world.get_component_by_id_as_mut::<XREyeTrackingComponent>(id) {
        match method {
            "priority" => tracker.priority = eye_tracking_priority_arg(args)?,
            "enable_pupil_direction_tracking" => {
                tracker.enable_pupil_direction_tracking = arg_bool(args, 0)?;
            }
            "head_rotation_compensation" => {
                tracker.head_rotation_compensation =
                    crate::engine::ecs::component::HeadRotationCompensation::parse(arg_str(
                        args, 0,
                    )?)
                    .ok_or_else(|| {
                        "XREyeTracking.head_rotation_compensation expects 'off' or 'cancel'"
                            .to_string()
                    })?;
            }
            "rotation_limits" => {
                tracker.rotation_limits = Some(rotation_limits_call_args(args, "XREyeTracking")?);
            }
            "rotation_limits_per_eye" => {
                tracker.rotation_limits_per_eye = [
                    Some(rotation_limits_array_arg(args, 0, "XREyeTracking")?),
                    Some(rotation_limits_array_arg(args, 1, "XREyeTracking")?),
                ];
            }
            _ => {}
        }
        return Ok(());
    }
    if let Some(tracker) = world.get_component_by_id_as_mut::<VRChatOSCEyeTrackingComponent>(id) {
        match method {
            "enable_pupil_direction_tracking" => {
                tracker.enable_pupil_direction_tracking = arg_bool(args, 0)?;
            }
            "head_rotation_compensation" => {
                tracker.head_rotation_compensation =
                    crate::engine::ecs::component::HeadRotationCompensation::parse(arg_str(
                        args, 0,
                    )?)
                    .ok_or_else(|| {
                        "VRChatOSCEyeTracking.head_rotation_compensation expects 'off' or 'cancel'"
                            .to_string()
                    })?;
            }
            "rotation_limits" => {
                tracker.rotation_limits =
                    Some(rotation_limits_call_args(args, "VRChatOSCEyeTracking")?);
            }
            "rotation_limits_per_eye" => {
                tracker.rotation_limits_per_eye = [
                    Some(rotation_limits_array_arg(args, 0, "VRChatOSCEyeTracking")?),
                    Some(rotation_limits_array_arg(args, 1, "VRChatOSCEyeTracking")?),
                ];
            }
            _ => {}
        }
        return Ok(());
    }
    if let Some(tracker) = world.get_component_by_id_as_mut::<XREyeTrackingHtcComponent>(id) {
        match method {
            "enable_pupil_direction_tracking" => {
                tracker.enable_pupil_direction_tracking = arg_bool(args, 0)?;
            }
            "head_rotation_compensation" => {
                tracker.head_rotation_compensation =
                    crate::engine::ecs::component::HeadRotationCompensation::parse(arg_str(
                        args, 0,
                    )?)
                    .ok_or_else(|| {
                        "XREyeTrackingHTC.head_rotation_compensation expects 'off' or 'cancel'"
                            .to_string()
                    })?;
            }
            "rotation_limits" => {
                tracker.rotation_limits =
                    Some(rotation_limits_call_args(args, "XREyeTrackingHTC")?);
            }
            "rotation_limits_per_eye" => {
                tracker.rotation_limits_per_eye = [
                    Some(rotation_limits_array_arg(args, 0, "XREyeTrackingHTC")?),
                    Some(rotation_limits_array_arg(args, 1, "XREyeTrackingHTC")?),
                ];
            }
            _ => {}
        }
        return Ok(());
    }
    if let Some(tracker) = world.get_component_by_id_as_mut::<MediaPipeEyeTrackingComponent>(id) {
        match method {
            "head_rotation_compensation" => {
                tracker.head_rotation_compensation =
                    crate::engine::ecs::component::HeadRotationCompensation::parse(arg_str(
                        args, 0,
                    )?)
                    .ok_or_else(|| {
                        "MediaPipeEyeTracking.head_rotation_compensation expects 'off' or 'cancel'"
                            .to_string()
                    })?;
            }
            "rotation_limits" => {
                tracker.rotation_limits =
                    Some(rotation_limits_call_args(args, "MediaPipeEyeTracking")?);
            }
            "rotation_limits_per_eye" => {
                tracker.rotation_limits_per_eye = [
                    Some(rotation_limits_array_arg(args, 0, "MediaPipeEyeTracking")?),
                    Some(rotation_limits_array_arg(args, 1, "MediaPipeEyeTracking")?),
                ];
            }
            _ => {}
        }
        return Ok(());
    }
    Err("not an eye tracking component".into())
}
