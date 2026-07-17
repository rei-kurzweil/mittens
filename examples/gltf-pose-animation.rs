use mittens_engine::{engine, engine::ecs::SignalEmitter, scripting, utils};

fn main() {
    mittens_engine::example_support::ensure_model_assets();
    utils::logger::init();

    let mut universe = engine::Universe::new(engine::ecs::World::default());
    let source = include_str!("gltf-pose-animation.mms");
    let output = scripting::MeowMeowRunner::eval_with_world_and_assets_at_path(
        source,
        Some("examples/gltf-pose-animation.mms"),
        &mut universe.world,
        &mut universe.systems.rx,
        Some(&mut universe.render_assets),
        &mut universe.command_queue,
    );
    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    assert!(
        output.errors.is_empty(),
        "MMS evaluation produced errors. Capture the three documented pose assets first."
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
