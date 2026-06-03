/// mms-loops: visual demo for MMS Phase 5 (for/in, range, break, continue).
///
/// Evaluates mms-loops.mms and opens a live window.
/// Spawn errors print to stdout via [SpawnComponentTree] lines.
/// Use WASD/RF/QE + right-drag to navigate.
///
///   cargo run --example mms-loops
use cat_engine::{engine, engine::ecs::{IntentValue, SignalEmitter}, meow_meow, utils};

#[path = "example_util/mod.rs"]
mod example_util;

fn main() {
    utils::logger::init();

    let output = meow_meow::MeowMeowRunner::eval(include_str!("mms-loops.mms"));

    for error in &output.errors {
        eprintln!("[mms] error: {error}");
    }
    assert!(
        output.errors.is_empty(),
        "MMS evaluation produced errors: {:?}",
        output.errors
    );

    println!("[mms] {} intent(s):", output.intents.len());
    for intent in &output.intents {
        if let IntentValue::SpawnComponentTree { root, .. } = intent {
            println!("  spawn {}", root.component_type);
        }
    }

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let scope = engine::ecs::ComponentId::default();
    for intent in output.intents {
        universe.command_queue.push_intent_now(scope, intent);
    }

    example_util::spawn_mms_demo_rig(&mut universe, [1.5, 6.0, 9.0]);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &universe.render_assets,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
