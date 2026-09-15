use mittens_engine::{engine, utils};

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);
    universe
        .load_mms_source_at_path(
            include_str!("runtime-spec-smoke.mms"),
            "examples/runtime-spec-smoke.mms",
        )
        .expect("RuntimeSpec evaluation failed");

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
