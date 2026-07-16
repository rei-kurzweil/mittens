use mittens_engine::engine::ecs::SignalEmitter;
use mittens_engine::{engine, scripting, utils};

fn main() {
    utils::logger::init();

    let mut universe = engine::Universe::new(engine::ecs::World::default());

    // Load and run the observer-router scene.
    let output = scripting::MeowMeowRunner::eval_with_world_at_path(
        include_str!("observer-router.mms"),
        Some("examples/observer-router.mms"),
        &mut universe.world,
        &mut universe.systems.rx,
        &mut universe.command_queue,
    );

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    println!(
        "[mms] {} intent(s) from observer-router.mms",
        output.intents.len()
    );

    for intent in output.intents {
        universe
            .command_queue
            .push_intent_now(engine::ecs::ComponentId::default(), intent);
    }

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
        &mut universe.command_queue,
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
