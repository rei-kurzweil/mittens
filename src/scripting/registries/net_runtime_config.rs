//! Multiplayer session and replication vocabulary.
use super::*;
pub(super) fn configure(
    component: &mut mms::ComponentBuilder<'_, MittensBinding>,
    canonical: &'static str,
) {
    let no_args = || component_signature([]);
    let strings = |count| component_signature(vec![mms::ValueType::String; count]);
    let method = |arguments, result| mms::ValueSignature::new(arguments, result);
    match canonical {
        "NetSession" => {
            component.constructor(
                "host",
                optional_component_signature([mms::ValueType::String], 0),
            );
            component.constructor("join", strings(1));
            component.constructor("from_env", no_args());
            constructor_and_builder(component, "player_name", strings(1));
            constructor_and_builder(
                component,
                "tick_hz",
                component_signature([mms::ValueType::F32]),
            );
            constructor_and_builder(
                component,
                "enabled",
                component_signature([mms::ValueType::Bool]),
            );
            host_method(
                component,
                canonical,
                "send",
                method(vec![mms::ValueType::String; 2], mms::ValueType::Null),
            );
            host_method(
                component,
                canonical,
                "is_host",
                method(vec![], mms::ValueType::Bool),
            );
            host_method(
                component,
                canonical,
                "peer_id",
                method(vec![], mms::ValueType::U32),
            );
            host_method(
                component,
                canonical,
                "local_name",
                method(vec![], mms::ValueType::String),
            );
        }
        "NetSync" => {
            component.constructor("host", strings(1));
            component.constructor("local_player", no_args());
            component.constructor("remote_player", component_signature([mms::ValueType::U32]));
        }
        _ => {}
    }
}
