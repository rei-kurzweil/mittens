/// MMS signal handler demo.
///
/// Loads signal-handler.mms, which authors the scene and registers Click
/// handlers via `on()`. Clicking a cube prints its name to stdout.
///
/// Run: cargo run --release --example signal-handler
use mittens_engine::{engine, engine::ecs::SignalEmitter, scripting, utils};

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Evaluate the MMS script with live world access.
    // on() calls inside the script register Click handlers into universe.systems.rx.
    let source = include_str!("signal-handler.mms");
    let output = scripting::MeowMeowRunner::eval_with_world(
        source,
        &mut universe.world,
        &mut universe.systems.rx,
        &mut universe.command_queue,
    );

    for err in &output.errors {
        eprintln!("[mms] {err}");
    }
    if !output.errors.is_empty() {
        std::process::exit(1);
    }

    // Any bare CE emissions from the script (not let-bound) are in output.intents.
    // Our script only uses let-bindings so this is empty, but handle it anyway.
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
