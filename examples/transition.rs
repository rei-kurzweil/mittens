use cat_engine::{engine, engine::ecs::SignalEmitter, meow_meow, utils};

fn main() {
    utils::logger::init();

    let output = meow_meow::MeowMeowRunner::eval(include_str!("transition.mms"));

    for error in &output.errors {
        eprintln!("[mms error] {error}");
    }
    println!("[transition] {} intent(s)", output.intents.len());

    if !output.errors.is_empty() {
        eprintln!("[transition] FAIL — {} error(s)", output.errors.len());
        std::process::exit(1);
    }

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let scope = engine::ecs::ComponentId::default();
    for intent in output.intents {
        universe.command_queue.push_intent_now(scope, intent);
    }

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
