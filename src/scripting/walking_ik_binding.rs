//! Strict MMS declarations for walking tuning; no pose-system lifecycle hooks.
use super::{
    MittensBinding, component_signature, constructor_and_builder, host_constructor_and_builder,
};
use meow_meow_script as mms;

pub(super) fn register(component: &mut mms::ComponentBuilder<'_, MittensBinding>) {
    host_constructor_and_builder(
        component,
        "WalkingIK",
        "enabled",
        component_signature([mms::ValueType::Bool]),
    );
    for name in ["stride", "step_duration", "step_height", "step_scale"] {
        host_constructor_and_builder(
            component,
            "WalkingIK",
            name,
            component_signature([mms::ValueType::F32]),
        );
    }
}

pub(super) fn register_legacy(component: &mut mms::ComponentBuilder<'_, MittensBinding>) {
    constructor_and_builder(
        component,
        "walking_enabled",
        component_signature([mms::ValueType::Bool]),
    );
    for name in [
        "walking_stride",
        "walking_step_duration",
        "walking_step_height",
    ] {
        constructor_and_builder(component, name, component_signature([mms::ValueType::F32]));
    }
}
