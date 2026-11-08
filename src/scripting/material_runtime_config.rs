//! Material constructors and builders for the configured MMS host.
use super::*;

pub(super) fn register(
    component: &mut mms::ComponentBuilder<'_, MittensBinding>,
    canonical: &'static str,
) {
    let no_args = || component_signature([]);
    let strings = |count| component_signature(vec![mms::ValueType::String; count]);
    let method = |arguments, result| mms::ValueSignature::new(arguments, result);
    match canonical {
        "MaterialBase" => {
            host_constructor(component, canonical, "new", no_args());
            host_constructor(component, canonical, "extracted_emissive", no_args());
            host_constructor(component, canonical, "planar_reflective", no_args());
        }
        "MaterialDefinition" => {
            host_constructor(component, canonical, "anime", no_args());
            host_constructor(component, canonical, "toon", no_args());
            host_constructor(
                component,
                canonical,
                "new",
                component_signature([mms::ValueType::Component]),
            );
            host_builder_call(component, canonical, "opaque_bucket", strings(1));
            host_builder_call(component, canonical, "fragment", strings(1));
            host_builder_call(component, canonical, "input_f32", strings(1));
            host_builder_call(
                component,
                canonical,
                "inputs",
                component_signature([mms::ValueType::RecordType]),
            );
        }
        "Material" => {
            host_constructor(
                component,
                canonical,
                "new",
                component_signature([mms::ValueType::Component]),
            );
            host_builder_call(
                component,
                canonical,
                "inputs",
                component_signature([mms::ValueType::Table]),
            );
            host_builder_call(
                component,
                canonical,
                "descriptor_set",
                component_signature([mms::ValueType::Table]),
            );
            host_method(
                component,
                canonical,
                "set_input",
                method(
                    vec![mms::ValueType::String, mms::ValueType::Any],
                    mms::ValueType::Null,
                ),
            );
            host_method(
                component,
                canonical,
                "get_input",
                method(vec![mms::ValueType::String], mms::ValueType::Any),
            );
        }
        _ => unreachable!(),
    }
}
