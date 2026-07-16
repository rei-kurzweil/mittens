use mittens_engine::{engine, engine::ecs::SignalEmitter, scripting, utils};

fn main() {
    mittens_engine::example_support::ensure_model_assets();
    utils::logger::init();
    // A path-aware evaluation is required so the scene can import its MMS preset module.
    let output = scripting::MeowMeowRunner::eval_file("examples/vtuber-secondary-motion.mms");
    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    assert!(
        output.errors.is_empty(),
        "MMS evaluation produced errors: {:?}",
        output.errors
    );

    let mut universe = engine::Universe::new(engine::ecs::World::default());
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
    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
