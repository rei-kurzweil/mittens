/// Component method call demo.
///
/// Loads component-method-call.mms, which authors the scene and wires signal
/// handlers that call `anim.pause()` and `anim.play()` on a live AnimationComponent.
///
/// Click the blue cube  → pause the spinning animation.
/// Click the green cube → resume the spinning animation.
///
/// Run: cargo run --release --example component-method-call
use cat_engine::{engine, engine::ecs::SignalEmitter, meow_meow, utils};

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let source = include_str!("component-method-call.mms");
    let output = meow_meow::MeowMeowRunner::eval_with_world(
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

    for intent in output.intents {
        universe.command_queue.push_intent_now(engine::ecs::ComponentId::default(), intent);
    }

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
