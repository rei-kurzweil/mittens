use mittens_engine::{engine, engine::ecs::SignalEmitter, scripting, utils};

fn main() {
    mittens_engine::example_support::ensure_model_assets();
    utils::logger::init();

    let output = scripting::MeowMeowRunner::eval(include_str!("inline-block-layout.mms"));

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    println!(
        "[mms] {} intent(s) from inline-block-layout.mms",
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

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
