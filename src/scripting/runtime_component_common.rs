//! Common metadata and graph methods for legacy and configured components.
use super::*;

pub(crate) fn register(
    component: &mut mms::ComponentBuilder<'_, MittensBinding>,
    canonical: &'static str,
) {
    component.host_default_constructor(MittensBinding::ComponentConstructor {
        component: canonical,
        name: None,
    });
    if matches!(canonical, "Data" | "Style") {
        component.body_mode(mms::ComponentBodyMode::PropsOnly);
    } else if canonical == "Keyframe" {
        component.body_mode(mms::ComponentBodyMode::Deferred);
    }
    for shortform in mms::COMPONENT_SHORTFORMS
        .iter()
        .filter(|entry| entry.full == canonical && !entry.short.eq_ignore_ascii_case(canonical))
    {
        component.alias(shortform.short);
    }
    host_property(component, canonical, "name", mms::ValueType::String);
    host_property(component, canonical, "id", mms::ValueType::String);
    host_property(component, canonical, "guid", mms::ValueType::String);
    host_property(component, canonical, "class", mms::ValueType::Any);

    let method = |arguments, result| mms::ValueSignature::new(arguments, result);
    for (name, signature) in [
        (
            "attach",
            method(vec![mms::ValueType::Component], mms::ValueType::Null),
        ),
        (
            "attach_clone",
            method(vec![mms::ValueType::Component], mms::ValueType::Null),
        ),
        (
            "detach",
            mms::ValueSignature::with_optional(
                vec![mms::ValueType::Component],
                0,
                mms::ValueType::Null,
            ),
        ),
        ("detach_all", method(vec![], mms::ValueType::Null)),
        (
            "remove_child",
            method(vec![mms::ValueType::U32], mms::ValueType::Null),
        ),
        ("remove_subtree", method(vec![], mms::ValueType::Null)),
        (
            "set_color",
            method(vec![mms::ValueType::Array], mms::ValueType::Null),
        ),
    ] {
        host_method(component, canonical, name, signature);
    }
}
