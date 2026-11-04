use super::{MittensBinding, component_signature, host_constructor_and_builder};
use meow_meow_script as mms;
pub(super) fn register(
    component: &mut mms::ComponentBuilder<'_, MittensBinding>,
    canonical: &'static str,
) {
    let methods: &[(&str, usize)] = match canonical {
        "Destroyable" => &[("threshold", 1), ("cell_size", 1), ("chunk_size", 1)],
        "Damage" => &[("at", 5)],
        "ImplicitSurface" => &[
            ("bounds", 6),
            ("voxel_size", 1),
            ("iso_level", 1),
            ("smooth_min_radius", 1),
            ("clip_floor", 1),
        ],
        "ImplicitSphere" => &[("radius", 1)],
        _ => unreachable!(),
    };
    for &(name, count) in methods {
        host_constructor_and_builder(
            component,
            canonical,
            name,
            component_signature(vec![mms::ValueType::F32; count]),
        );
    }
}
