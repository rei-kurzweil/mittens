use mittens_engine::engine::ecs::component::EmissiveComponent;
use mittens_engine::engine::{self, ecs::World};

fn main() {
    let world = World::default();
    let mut universe = engine::Universe::new(world);

    universe
        .load_mms_source_at_path(
            include_str!("runtime-spec-smoke.mms"),
            "examples/runtime-spec-smoke.mms",
        )
        .expect("RuntimeSpec evaluation failed");

    let normalized_names: Vec<_> = universe
        .world
        .all_components()
        .filter_map(|id| universe.world.component_name(id))
        .map(|name| name.replace('_', "").to_lowercase())
        .collect();
    assert_eq!(
        normalized_names
            .iter()
            .filter(|name| *name == "camera3d")
            .count(),
        1
    );
    assert_eq!(
        normalized_names
            .iter()
            .filter(|name| *name == "renderable")
            .count(),
        3
    );
    assert_eq!(
        normalized_names
            .iter()
            .filter(|name| *name == "raycastable")
            .count(),
        3
    );
    assert_eq!(
        normalized_names
            .iter()
            .filter(|name| *name == "directionallight")
            .count(),
        1
    );
    assert_eq!(
        normalized_names
            .iter()
            .filter(|name| *name == "emissive")
            .count(),
        3
    );

    let component_with_label = |label: &str| {
        universe
            .world
            .all_components()
            .find(|&id| universe.world.component_label(id) == Some(label))
            .unwrap_or_else(|| panic!("missing component labelled '{label}'"))
    };
    let gallery = component_with_label("runtime-spec-cube-gallery");
    for (color, expected_intensity) in [("red", 3.0), ("cyan", 2.4), ("violet", 3.6)] {
        let cube = component_with_label(&format!("{color}-cube"));
        let mesh = component_with_label(&format!("{color}-mesh"));
        let glow = component_with_label(&format!("{color}-glow"));

        assert_eq!(universe.world.parent_of(cube), Some(gallery));
        assert_eq!(universe.world.parent_of(mesh), Some(cube));
        assert_eq!(universe.world.parent_of(glow), Some(mesh));
        let emissive = universe
            .world
            .get_component_by_id_as::<EmissiveComponent>(glow)
            .expect("labelled glow is an Emissive component");
        assert!((emissive.intensity - expected_intensity).abs() < f32::EPSILON);
    }

    let post_processing = universe.visuals.post_processing();
    assert!(post_processing.is_active());
    let bloom = post_processing.bloom.as_ref().expect("bloom is configured");
    assert!((bloom.intensity - 1.8).abs() < f32::EPSILON);

    println!("RuntimeSpec smoke passed: live-handle cube gallery + camera + light + active bloom");
}
