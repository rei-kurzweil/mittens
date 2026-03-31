use cat_engine::{engine, engine::ecs::SignalEmitter, meow_meow, utils};

// query-demo: demonstrates the MMS query system.
//
// Run:  cargo run --example query-demo
//
// What to look for in the scene:
//   - 1 hero cube (starts yellow, mutations turn it blue → magenta)
//   - 3 enemy cubes (start red, mutations turn them green, enemy_0 turns orange)
//   - 1 panel with 2 spheres (turn yellow, then colour components go purple)
//
// The scene construction (CE emit) works with the current static runner.
// The query / mutation sections require:
//   - Phase 6: live ComponentId reply channel
//   - Phase 7: component mutation methods (set_rgba, set_position, ...)
//   - HostCall infrastructure for query() / query_all()
//   - name = "id" body bind in the component registry
//
// Console output (requires print() + assert() builtins):
//   [mms] scene built: 1 hero + 3 enemies + 1 panel with 2 children
//   ...
//   [mms] query-demo: all assertions passed
//
// NOTE: output.prints is not yet on EvalOutput — add it alongside the
//       print() and assert() evaluator builtins.

fn main() {
    utils::logger::init();

    let output = meow_meow::MeowMeowRunner::eval(include_str!("query-demo.mms"));

    for error in &output.errors {
        eprintln!("[mms error] {error}");
    }
    println!("[query-demo] {} intent(s)", output.intents.len());

    if !output.errors.is_empty() {
        eprintln!("[query-demo] FAIL — {} error(s)", output.errors.len());
        std::process::exit(1);
    }

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let scope = engine::ecs::ComponentId::default();
    for intent in output.intents {
        universe.command_queue.push_intent_now(scope, intent);
    }

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
