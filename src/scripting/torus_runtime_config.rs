//! Full torus and capped arc MMS signatures.
use super::*;
pub(super) fn configure(component: &mut mms::ComponentBuilder<'_, MittensBinding>) {
    use mms::ValueType::{F32, U32};
    component
        .constructor(
            "torus",
            optional_component_signature([F32, F32, U32, U32], 0),
        )
        .constructor(
            "partial_torus",
            component_signature([F32, F32, F32, F32, U32, U32]),
        );
}
