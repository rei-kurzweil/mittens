use cat_engine::{engine, example, utils};

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    example::build_demo_scene_7_shapes(&mut universe);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );
    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
