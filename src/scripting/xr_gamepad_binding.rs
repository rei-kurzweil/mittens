use super::*;
pub(super) fn register(
    component: &mut mms::ComponentBuilder<'_, MittensBinding>,
    canonical: &'static str,
) {
    component.constructor("new", component_signature([]));
    for name in ["enabled", "enable_flying"] {
        constructor_and_builder(component, name, component_signature([mms::ValueType::Bool]));
    }
    constructor_and_builder(
        component,
        "hand",
        component_signature([mms::ValueType::String]),
    );
    constructor_and_builder(
        component,
        "locomotion",
        optional_component_signature([mms::ValueType::Bool], 0),
    );
    for name in ["speed", "deadzone"] {
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
}
