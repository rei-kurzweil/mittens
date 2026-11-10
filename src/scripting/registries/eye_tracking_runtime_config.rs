//! Eye source and idle blink vocabulary.
use super::*;
pub(super) fn configure(
    component: &mut mms::ComponentBuilder<'_, MittensBinding>,
    canonical: &str,
) {
    let no_args = || component_signature([]);
    let floats = |count| component_signature(vec![mms::ValueType::F32; count]);
    let booleans = |count| component_signature(vec![mms::ValueType::Bool; count]);
    let any = |count| component_signature(vec![mms::ValueType::Any; count]);
    let strings = |count| component_signature(vec![mms::ValueType::String; count]);
    match canonical {
        "XREyeTracking" => {
            component.constructor("on", no_args());
            component.constructor(
                "listen",
                component_signature([mms::ValueType::String, mms::ValueType::U16]),
            );
            component.builder_call("priority", any(1));
            component.builder_call("enable_pupil_direction_tracking", booleans(1));
            component.builder_call("head_rotation_compensation", strings(1));
            component.builder_call("rotation_limits", floats(4));
            component.builder_call("rotation_limits_per_eye", any(2));
        }
        "XREyeTrackingHTC" | "VRChatOSCEyeTracking" | "HTCEyeTracking" => {
            component.constructor("on", no_args());
            component.constructor(
                "listen",
                component_signature([mms::ValueType::String, mms::ValueType::U16]),
            );
            component.builder_call("enable_pupil_direction_tracking", booleans(1));
            component.builder_call("head_rotation_compensation", strings(1));
            component.builder_call("rotation_limits", floats(4));
            component.builder_call("rotation_limits_per_eye", any(2));
        }
        "MediaPipeEyeTracking" => {
            component.constructor("on", no_args());
            component.builder_call("head_rotation_compensation", strings(1));
            component.builder_call("rotation_limits", floats(4));
            component.builder_call("rotation_limits_per_eye", any(2));
        }
        _ => unreachable!(),
    }
}
pub(super) fn configure_blink(component: &mut mms::ComponentBuilder<'_, MittensBinding>) {
    component
        .builder_call("idle_blink", component_signature([mms::ValueType::Bool]))
        .builder_call("blink_interval", component_signature([mms::ValueType::F32]))
        .builder_call("blink_duration", component_signature([mms::ValueType::F32]));
    host_method(
        component,
        "AvatarControl",
        "eyelid_tracking",
        mms::ValueSignature::new(vec![], mms::ValueType::Any),
    );
}
