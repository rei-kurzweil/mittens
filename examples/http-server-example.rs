use mittens_engine::{engine, engine::ecs::SignalEmitter, scripting, utils};

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let output = scripting::MeowMeowRunner::eval_with_world_and_assets_at_path(
        include_str!("http-server-example.mms"),
        Some("examples/http-server-example.mms"),
        &mut universe.world,
        &mut universe.systems.rx,
        Some(&mut universe.render_assets),
        &mut universe.command_queue,
    );

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    if !output.errors.is_empty() {
        std::process::exit(1);
    }

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

    println!(
        "[http-server-example] listening on 127.0.0.1:7000; send POST / with curl -X POST http://127.0.0.1:7000/ -d 'hello'"
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
