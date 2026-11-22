//! The sakura twilight fork of the rooftop example loads without its train.
use crate::engine::Universe;
use crate::engine::ecs::World;

#[test]
fn sakura_twilight_loads_its_city_without_the_train() {
    let path = "examples/sakura-twilight.mms";
    let source = std::fs::read_to_string(path)
        .unwrap()
        .replace("let use_xr = true", "let use_xr = false")
        .replace("let use_desktop = false", "let use_desktop = true");
    let mut scene = Universe::new(World::default());
    scene.load_mms_source_at_path(&source, path).unwrap();
    for _ in 0..3 {
        scene.update(1.0 / 60.0, &Default::default());
    }
    let labels: Vec<&str> = scene
        .world
        .all_components()
        .filter_map(|id| scene.world.component_label(id))
        .collect();
    assert!(
        !labels.iter().any(|label| label.contains("train")),
        "no train parts"
    );
    for name in [
        "sakura_twilight_main",
        "sakura_twilight_row_0",
        "sakura_twilight_row_1",
        "sakura_twilight_row_2",
        "sakura_twilight_row_3",
        "sakura_twilight_road_dashes",
        "sakura_twilight_pickup",
        "kawaii_twilight_sky",
    ] {
        assert_eq!(
            labels.iter().filter(|&&label| label == name).count(),
            1,
            "{name}"
        );
    }
}
