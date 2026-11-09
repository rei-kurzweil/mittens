//! Spring-chain constructor and builder vocabulary.
use super::*;
pub(super) fn configure(component: &mut mms::ComponentBuilder<'_, MittensBinding>) {
    let floats = |count| component_signature(vec![mms::ValueType::F32; count]);
    let booleans = |count| component_signature(vec![mms::ValueType::Bool; count]);
    let strings = |count| component_signature(vec![mms::ValueType::String; count]);
    let any = |count| component_signature(vec![mms::ValueType::Any; count]);
    component
        .constructor("new", strings(1))
        .constructor("from_root", any(1))
        .builder_call("center", any(1))
        .builder_call("colliders", any(1))
        .builder_call("world_collision", booleans(1))
        .builder_call("avatar_collision", booleans(1))
        .builder_call("enabled", booleans(1))
        .builder_call("allow_stretch", booleans(1));
    for method in [
        "virtual_end_length_ratio",
        "stiffness",
        "drag_force",
        "hit_radius",
    ] {
        component.builder_call(method, floats(1));
    }
    // The host accepts either (power, vec3) or
    // (power, x, y, z); overloads remain a tracked seam.
    component.builder_call("gravity", any(2));
}
