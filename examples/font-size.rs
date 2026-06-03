use cat_engine::{engine, engine::ecs::SignalEmitter, meow_meow, utils};

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let source = include_str!("font-size.mms");
    let output = meow_meow::MeowMeowRunner::eval_with_world_at_path(
        source,
        Some("examples/font-size.mms"),
        &mut universe.world,
        &mut universe.systems.rx,
        &mut universe.command_queue,
    );

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    println!("[mms] {} intent(s) from font-size.mms", output.intents.len());

    for intent in output.intents {
        universe
            .command_queue
            .push_intent_now(engine::ecs::ComponentId::default(), intent);
    }

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &universe.render_assets,
        &mut universe.command_queue,
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}