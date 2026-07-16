/// mms-module-example: visual demo for MMS Phase 6 (module import / export).
///
/// Loads mms-module-example.mms which in turn imports cat.mms via
/// `import { 0 as cat } from "cat.mms"`. Demonstrates cross-file CE import
/// and Positional(ComponentExpr) → Child promotion inside a parent CE body.
///
/// Run from the repo root so relative paths resolve correctly:
///   cargo run --example mms-module-example
///
/// Use WASD/RF/QE + right-mouse drag to navigate.
use mittens_engine::{engine, engine::ecs::SignalEmitter, scripting, utils};

#[path = "example_util/mod.rs"]
mod example_util;

fn main() {
    utils::logger::init();

    // eval_file passes the path so `import "cat.mms"` resolves relative to
    // examples/ — do NOT use include_str! here (that loses the path).
    let output = scripting::MeowMeowRunner::eval_file("examples/mms-module-example.mms");

    for error in &output.errors {
        eprintln!("[mms] error: {error}");
    }
    assert!(
        output.errors.is_empty(),
        "MMS evaluation produced errors: {:?}",
        output.errors,
    );

    println!("[mms] {} intent(s):", output.intents.len());
    for intent in &output.intents {
        if let engine::ecs::IntentValue::SpawnComponentTree { root, .. } = intent {
            println!("  spawn {}", root.component_type);
        }
    }

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let scope = engine::ecs::ComponentId::default();
    for intent in output.intents {
        universe.command_queue.push_intent_now(scope, intent);
    }

    // Camera placed to see the cat from slightly above and in front.
    example_util::spawn_mms_demo_rig(&mut universe, [0.0, 1.2, 4.5]);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.render_assets,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
