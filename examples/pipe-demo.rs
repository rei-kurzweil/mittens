use cat_engine::{engine, engine::ecs::SignalEmitter, meow_meow, utils};

// pipe-demo: demonstrates |> forward pipe operator in MMS.
//
// Run:  cargo run --example pipe-demo
//
// What to look for in the scene:
//   - two horizontal strips of greyscale cubes (gamma-encoded and inverted)
//   - a sphere whose colour is driven by a computed pipe chain
//
// Console output (from MMS print() — requires print() builtin to be implemented):
//   [mms] double(2) = 4
//   [mms] clamp01(double(0.3)) = 0.6
//   ...
//   [mms] pipe-demo: all assertions passed
//
// NOTE: output.prints is not yet on EvalOutput — add it alongside the
//       print() and assert() evaluator builtins when implementing |>.

fn main() {
    utils::logger::init();

    let output = meow_meow::MeowMeowRunner::eval(include_str!("pipe-demo.mms"));

    for error in &output.errors {
        eprintln!("[mms error] {error}");
    }
    println!("[pipe-demo] {} intent(s)", output.intents.len());

    if !output.errors.is_empty() {
        eprintln!("[pipe-demo] FAIL — {} error(s)", output.errors.len());
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
        &universe.render_assets,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
