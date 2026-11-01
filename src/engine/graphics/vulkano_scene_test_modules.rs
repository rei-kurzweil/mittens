mod custom_material_tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/engine/graphics/vulkano_custom_material_tests.rs"
    ));
}
mod bvh_tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/engine/graphics/vulkano_bvh_tests.rs"
    ));
}
