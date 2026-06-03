/// render-graph-diagram: visual scene exercising all render phases.
///
/// Loads render-graph-diagram.mms and places it in a navigable scene.
/// The MMS file documents the future PostProcessing / Bloom / Bokeh API
/// in comments — see render-graph-diagram.mms for the annotated source.
///
/// Run from the repo root:
///   cargo run --example render-graph-diagram
///
/// Controls: WASD/RF to move, right-mouse-drag to look (desktop).
///
/// See also:
///   docs/spec/render-graph-post-processing.md
///   docs/spec/render-graph-pipeline.svg
///   docs/spec/render-graph-pipeline-post-processing.svg
use cat_engine::{engine, engine::ecs::SignalEmitter, meow_meow, utils};

#[path = "example_util/mod.rs"]
mod example_util;

fn main() {
    utils::logger::init();

    let output = meow_meow::MeowMeowRunner::eval_file("examples/render-graph-diagram.mms");

    for error in &output.errors {
        eprintln!("[mms] error: {error}");
    }
    if !output.errors.is_empty() {
        eprintln!("[mms] {} error(s) — scene may be incomplete", output.errors.len());
    }

    println!("[mms] {} intent(s) emitted:", output.intents.len());
    for intent in &output.intents {
        if let engine::ecs::IntentValue::SpawnComponentTree { root, .. } = intent {
            println!("  spawn {}", root.component_type);
        }
    }

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Push all MMS-emitted intents into the engine.
    let scope = engine::ecs::ComponentId::default();
    for intent in output.intents {
        universe.command_queue.push_intent_now(scope, intent);
    }

    // Camera: pulled back and slightly elevated so the full scene is in view.
    // The emissive orbs above the pedestals are the focal point.
    example_util::spawn_mms_demo_rig(&mut universe, [0.0, 2.2, 7.0]);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &universe.render_assets,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
