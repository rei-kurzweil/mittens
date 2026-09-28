use mittens_engine::{engine, utils};

fn main() {
    mittens_engine::example_support::ensure_model_assets();
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    universe
        .load_mms_source_at_path(
            include_str!("bisket-desktop-demo.mms"),
            "examples/bisket-desktop-demo.mms",
        )
        .unwrap_or_else(|error| panic!("MMS evaluation failed: {error}"));

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
