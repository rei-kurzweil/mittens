use mittens_engine::{engine, engine::ecs::SignalEmitter, scripting, utils};
use std::time::Duration;

fn main() {
    mittens_engine::example_support::ensure_model_assets();
    utils::logger::init();

    let output = scripting::MeowMeowRunner::eval_file_with_timeout(
        "examples/camera-constraint.mms",
        Duration::from_secs(10),
    );

    for error in &output.errors {
        eprintln!("[mms] error: {error}");
    }
    assert!(
        output.errors.is_empty(),
        "MMS evaluation produced errors: {:?}",
        output.errors,
    );

    println!(
        "[mms] {} intent(s) from camera-constraint.mms",
        output.intents.len()
    );

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let scope = engine::ecs::ComponentId::default();
    for intent in output.intents {
        universe.command_queue.push_intent_now(scope, intent);
    }

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
