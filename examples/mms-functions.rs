/// mms-functions: visual demo + test harness for MMS Phases 2–4 (arithmetic, if/else, functions).
///
/// Evaluates mms-functions.mms, asserts on the intent count, then spawns the scene
/// into a live window. Use WASD/RF/QE + right-drag to navigate.
///
///   cargo run --example mms-functions
use cat_engine::{engine, engine::ecs::SignalEmitter, meow_meow, utils};

#[path = "example_util/mod.rs"]
mod example_util;

fn main() {
    utils::logger::init();

    let output = meow_meow::MeowMeowRunner::eval(include_str!("mms-functions.mms"));

    for error in &output.errors {
        eprintln!("[mms] error: {error}");
    }
    println!(
        "[mms] {} intent(s) from mms-functions.mms",
        output.intents.len()
    );

    assert!(
        output.errors.is_empty(),
        "MMS evaluation produced errors: {:?}",
        output.errors
    );
    assert_eq!(
        output.intents.len(),
        4,
        "expected 4 SpawnComponentTree intents (got {})",
        output.intents.len()
    );
    println!("[mms] assertions ok — opening window");

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let scope = engine::ecs::ComponentId::default();
    for intent in output.intents {
        universe.command_queue.push_intent_now(scope, intent);
    }

    example_util::spawn_mms_demo_rig(&mut universe, [0.0, 1.0, 5.0]);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
