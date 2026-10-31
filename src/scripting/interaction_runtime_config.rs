use super::{
    MittensBinding, component_signature, constructor_and_builder, mms, no_arg_constructors,
};

pub(super) fn configure(name: &str, component: &mut mms::ComponentBuilder<'_, MittensBinding>) {
    if name == "Climbable" {
        no_arg_constructors(component, &["off", "on"]);
    } else {
        no_arg_constructors(component, &["parent", "off", "on"]);
        constructor_and_builder(
            component,
            "to_hand",
            component_signature([mms::ValueType::Bool]),
        );
        component.builder_call("hand_offset", component_signature([mms::ValueType::Array]));
    }
}
