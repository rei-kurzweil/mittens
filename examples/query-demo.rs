/// MMS query demo — clickable buttons exercising query()/query_all() forms.
///
/// Loads query-demo.mms, which builds a scene of four labeled buttons. Each
/// button's Click handler runs a different query shape and mutates a target
/// Text component via set_text(...).
///
/// Run: cargo run --release --example query-demo
use cat_engine::{engine, engine::ecs::SignalEmitter, meow_meow, utils};

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let source = include_str!("query-demo.mms");
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
