/// mms-loops: visual demo + test harness for MMS Phase 5 (for/in, range, break, continue).
///
/// Evaluates mms-loops.mms, asserts on the intent count, then spawns the scene
/// into a live window. Use WASD/RF/QE + right-drag to navigate.
///
///   cargo run --example mms-loops
use cat_engine::{engine, engine::ecs::SignalEmitter, meow_meow, utils};

#[path = "example_util/mod.rs"]
mod example_util;

fn main() {
    utils::logger::init();

    let output = meow_meow::MeowMeowRunner::eval(include_str!("mms-loops.mms"));

    for error in &output.errors {
        eprintln!("[mms] error: {error}");
    }
    println!("[mms] {} intent(s) from mms-loops.mms", output.intents.len());

    assert!(
        output.errors.is_empty(),
        "MMS evaluation produced errors: {:?}",
        output.errors
    );
    // 4×4 grid = 16 cells, minus the 2×2 hole (4 skipped) = 12 intents
    assert_eq!(
        output.intents.len(),
        12,
        "expected 12 SpawnComponentTree intents (got {})",
        output.intents.len()
    );
    println!("[mms] assertions ok — opening window");

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Push all MMS intents.
    let scope = engine::ecs::ComponentId::default();
    for intent in output.intents {
        universe.command_queue.push_intent_now(scope, intent);
    }

    // Camera pulled back and up to see the 4×4 grid centred around (1.5, 0, 1.5).
    example_util::spawn_mms_demo_rig(&mut universe, [1.5, 6.0, 9.0]);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
