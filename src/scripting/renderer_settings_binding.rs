use super::{
    MittensBinding, component_signature, host_builder_call, host_constructor,
    host_constructor_and_builder,
};
use meow_meow_script as mms;

pub(super) fn register(
    component: &mut mms::ComponentBuilder<'_, MittensBinding>,
    canonical: &'static str,
) {
    host_constructor(component, canonical, "msaa_off", component_signature([]));
    host_constructor_and_builder(
        component,
        canonical,
        "window_size",
        component_signature(vec![mms::ValueType::U32; 2]),
    );
    host_constructor_and_builder(
        component,
        canonical,
        "transmission_depth_compare",
        component_signature([mms::ValueType::Bool]),
    );
    host_constructor(
        component,
        canonical,
        "desired_xr_resolution",
        component_signature([mms::ValueType::Array]),
    );
    host_builder_call(
        component,
        canonical,
        "desired_xr_resolution",
        component_signature([mms::ValueType::Array]),
    );
}
