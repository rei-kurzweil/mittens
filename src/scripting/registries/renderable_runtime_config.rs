//! MMS constructor vocabulary for the Renderable component family.
use super::*;
pub(super) fn configure(component: &mut mms::ComponentBuilder<'_, MittensBinding>) {
    let canonical = "Renderable";
    let no_args = || component_signature([]);
    let any = |count| component_signature(vec![mms::ValueType::Any; count]);
    configure_tori(component);
    host_constructor(component, canonical, "cube", no_args());
    no_arg_constructors(
        component,
        &[
            "annulus_2d",
            "circle2d",
            "circle_2d",
            "sphere",
            "triangle",
            "square",
            "plane",
            "tetrahedron",
        ],
    );
    component
        .constructor(
            "cone",
            optional_component_signature([mms::ValueType::U32], 0),
        )
        .constructor(
            "icosahedron",
            optional_component_signature([mms::ValueType::U32, mms::ValueType::F32], 0),
        )
        .constructor(
            "wireframe_sphere",
            optional_component_signature(
                [
                    mms::ValueType::U32,
                    mms::ValueType::U32,
                    mms::ValueType::F32,
                ],
                0,
            ),
        )
        .constructor(
            "wireframe_icosahedron",
            optional_component_signature(
                [
                    mms::ValueType::U32,
                    mms::ValueType::F32,
                    mms::ValueType::F32,
                ],
                0,
            ),
        )
        .constructor(
            "wireframe_box",
            optional_component_signature([mms::ValueType::F32], 0),
        )
        .constructor(
            "wireframe_square",
            optional_component_signature([mms::ValueType::F32], 0),
        )
        .constructor(
            "heart",
            optional_component_signature([mms::ValueType::U32], 0),
        )
        // Polygon has a stable mesh key plus its 2D point list.
        // Keep the strict runtime signature aligned with
        // component_registry::polygon_args.
        .constructor("polygon", any(2))
        .constructor(
            "cylinder",
            optional_component_signature(
                [
                    mms::ValueType::F32,
                    mms::ValueType::F32,
                    mms::ValueType::U32,
                ],
                0,
            ),
        )
        .constructor(
            "helix",
            optional_component_signature(
                [
                    mms::ValueType::F32,
                    mms::ValueType::F32,
                    mms::ValueType::F32,
                    mms::ValueType::F32,
                    mms::ValueType::F32,
                    mms::ValueType::U32,
                    mms::ValueType::F32,
                ],
                0,
            ),
        )
        .constructor(
            "helix_tube",
            optional_component_signature(
                [
                    mms::ValueType::F32,
                    mms::ValueType::F32,
                    mms::ValueType::F32,
                    mms::ValueType::F32,
                    mms::ValueType::F32,
                    mms::ValueType::U32,
                    mms::ValueType::U32,
                ],
                0,
            ),
        )
        .constructor(
            "partial_annulus_2d",
            optional_component_signature(
                [
                    mms::ValueType::F32,
                    mms::ValueType::F32,
                    mms::ValueType::F32,
                    mms::ValueType::F32,
                    mms::ValueType::U32,
                ],
                0,
            ),
        )
        .constructor(
            "star",
            optional_component_signature(
                [
                    mms::ValueType::U32,
                    mms::ValueType::F32,
                    mms::ValueType::U32,
                    mms::ValueType::U32,
                ],
                0,
            ),
        );
}

fn configure_tori(component: &mut mms::ComponentBuilder<'_, MittensBinding>) {
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
