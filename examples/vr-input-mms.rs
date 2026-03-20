use std::time::{Duration, Instant};

use cat_engine::{engine, engine::ecs::SignalEmitter, meow_meow, utils};

fn main() {
    utils::logger::init();

    let src = include_str!("vr-input.mms");

    // -----------------------------------------------------------------------
    // Evaluate the MMS script on the evaluator thread, collect intents.
    // -----------------------------------------------------------------------
    let mut eval = meow_meow::MeowMeowEvaluator::spawn(64);

    eval.requests
        .push(meow_meow::EvalRequest::EvalScript { source: src.to_string() })
        .expect("push EvalScript");
    eval.requests
        .push(meow_meow::EvalRequest::Shutdown)
        .expect("push Shutdown");

    let mut intents: Vec<engine::ecs::IntentValue> = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(2);

    loop {
        match eval.responses.pop() {
            Ok(meow_meow::EvalResponse::Intent(iv)) => {
                intents.push(iv);
            }
            Ok(meow_meow::EvalResponse::Error { message }) => {
                eprintln!("[mms] eval error: {message}");
            }
            Ok(meow_meow::EvalResponse::ParsedOk { .. }) => {}
            Ok(meow_meow::EvalResponse::ShutdownAck) => break,
            Err(rtrb::PopError::Empty) => {
                if Instant::now() > deadline {
                    eprintln!("[mms] timed out waiting for evaluator");
                    break;
                }
                std::thread::yield_now();
            }
        }
    }

    println!("[mms] collected {} intent(s) from vr-input.mms", intents.len());

    // -----------------------------------------------------------------------
    // Boot the engine and inject the intents.
    // -----------------------------------------------------------------------
    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let scope = engine::ecs::ComponentId::default();
    for intent in intents {
        universe.command_queue.push_intent_now(scope, intent);
    }

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    engine::Windowing::run_app(universe).expect("Windowing failed");
}
