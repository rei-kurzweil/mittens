use super::{MittensBinding, component_signature, host_constructor_and_builder, host_method};
use meow_meow_script as mms;
pub(super) fn register(
    component: &mut mms::ComponentBuilder<'_, MittensBinding>,
    canonical: &'static str,
) {
    for name in ["on", "off", "auto"] {
        host_constructor_and_builder(component, canonical, name, component_signature([]));
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
        "set_enabled",
        mms::ValueSignature::new(vec![mms::ValueType::Bool], mms::ValueType::Null),
    );
    host_constructor_and_builder(
        component,
        canonical,
        "enabled",
        component_signature([mms::ValueType::Bool]),
    );
    host_constructor_and_builder(
        component,
        canonical,
        "coefficient",
        component_signature([mms::ValueType::F32]),
    );
}
