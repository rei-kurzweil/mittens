use super::*;
pub(super) fn register(
    component: &mut mms::ComponentBuilder<'_, MittensBinding>,
    canonical: &'static str,
) {
    component.alias("input_vr_gamepad");
    component.constructor("new", component_signature([]));
    for name in ["enabled", "enable_flying"] {
        constructor_and_builder(component, name, component_signature([mms::ValueType::Bool]));
    }
    for name in ["hand", "stick_mode"] {
        constructor_and_builder(component, name, component_signature([mms::ValueType::String]));
    }
    constructor_and_builder(
        component,
        "locomotion",
        optional_component_signature([mms::ValueType::Bool], 0),
    );
    for name in ["speed", "turn_speed", "deadzone"] {
        constructor_and_builder(component, name, component_signature([mms::ValueType::F32]));
    }
    for name in ["enable", "disable"] {
        host_method(
            component,
            canonical,
            name,
            mms::ValueSignature::new(vec![], mms::ValueType::Null),
        );
    }
    host_method(
        component,
        canonical,
        "set_stick_mode",
        mms::ValueSignature::new(vec![mms::ValueType::String], mms::ValueType::Null),
    );
    host_method(
        component,
        canonical,
        "set_flight_enabled",
        mms::ValueSignature::new(vec![mms::ValueType::Bool], mms::ValueType::Null),
    );
    for name in ["flight_enabled", "flying"] {
        host_method(
            component,
            canonical,
            name,
            mms::ValueSignature::new(vec![], mms::ValueType::Bool),
        );
    }
}
