use mittens_engine::{engine, engine::ecs::SignalEmitter, scripting, utils};

fn main() {
    mittens_engine::example_support::ensure_model_assets();
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let source = include_str!("bisket-bones-and-ik.mms");
    let output = scripting::MeowMeowRunner::eval_with_world_at_path(
        source,
        Some("examples/bisket-bones-and-ik.mms"),
        &mut universe.world,
        &mut universe.systems.rx,
        &mut universe.command_queue,
    );

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    println!(
        "[mms] {} intent(s) from bisket-bones-and-ik.mms",
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
