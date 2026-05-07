use std::time::{Duration, Instant};

use crate::engine::ecs::{IntentValue, RxWorld, SignalEmitter, World};
use crate::meow_meow::evaluator::{
    eval_mms_fn, EvalRequest, EvalResponse, HostCallKind, HostValue, MeowMeowEvaluator,
};
use crate::meow_meow::object::Value;

/// The result of evaluating an MMS script: collected intents and any errors.
#[derive(Debug, Default)]
pub struct EvalOutput {
    pub intents: Vec<IntentValue>,
    pub errors: Vec<String>,
}

/// Synchronous wrapper around [`MeowMeowEvaluator`].
///
/// Spawns an evaluator thread, sends a script, drains all responses to
/// completion, and returns the collected [`EvalOutput`]. The thread is shut
/// down and joined before returning.
pub struct MeowMeowRunner;

impl MeowMeowRunner {
    /// Evaluate `source`, collecting all emitted intents and errors.
    /// Times out after 2 seconds if the evaluator stalls.
    pub fn eval(source: &str) -> EvalOutput {
        Self::eval_impl(source, None, Duration::from_secs(2))
    }

    /// Evaluate `source` with a caller-provided timeout.
    pub fn eval_with_timeout(source: &str, timeout: Duration) -> EvalOutput {
        Self::eval_impl(source, None, timeout)
    }

    /// Evaluate `source` knowing it came from `path` (enables relative imports).
    pub fn eval_with_path(source: &str, path: &str) -> EvalOutput {
        Self::eval_impl(source, Some(path), Duration::from_secs(2))
    }

    /// Read `path` from disk and evaluate it (enables relative imports).
    pub fn eval_file(path: &str) -> EvalOutput {
        Self::eval_file_with_timeout(path, Duration::from_secs(2))
    }

    /// Read `path` from disk and evaluate it (enables relative imports) with a caller-provided timeout.
    pub fn eval_file_with_timeout(path: &str, timeout: Duration) -> EvalOutput {
        match std::fs::read_to_string(path) {
            Ok(source) => Self::eval_impl(&source, Some(path), timeout),
            Err(e) => {
                let mut output = EvalOutput::default();
                output.errors.push(format!("cannot read file '{}': {}", path, e));
                output
            }
        }
    }

    /// Evaluate `source` with live world access.
    ///
    /// Handles two HostCall kinds during evaluation:
    /// - `Spawn`: spawns the component tree into `world` and returns the root `ComponentId`.
    ///   `let x = T {}` binds a `ComponentObject(id)` instead of a dead `ComponentExpr`.
    /// - `RegisterHandler`: installs an MMS function as a scoped signal handler in `rx`.
    ///   `on(obj, "Click", fn(e) { ... })` registers without blocking the evaluator.
    pub fn eval_with_world(
        source: &str,
        world: &mut World,
        rx: &mut RxWorld,
        emit: &mut dyn SignalEmitter,
    ) -> EvalOutput {
        let mut handle = MeowMeowEvaluator::spawn(64);
        handle
            .requests
            .push(EvalRequest::EvalScript {
                source: source.to_string(),
                source_path: None,
            })
            .expect("MeowMeowRunner: push EvalScript");
        handle
            .requests
            .push(EvalRequest::Shutdown)
            .expect("MeowMeowRunner: push Shutdown");

        let mut output = EvalOutput::default();
        let deadline = Instant::now() + Duration::from_secs(5);

        loop {
            match handle.responses.pop() {
                Ok(EvalResponse::Intent(iv)) => output.intents.push(iv),
                Ok(EvalResponse::Error { message }) => output.errors.push(message),
                Ok(EvalResponse::ParsedOk { .. }) => {}
                Ok(EvalResponse::ShutdownAck) => break,
                Ok(EvalResponse::HostCall { id, kind }) => {
                    let reply = match kind {
                        HostCallKind::Spawn(ce) => {
                            match crate::meow_meow::component_registry::spawn_tree(
                                &ce, None, world, emit,
                            ) {
                                Ok(component_id) => HostValue::ComponentId(component_id),
                                Err(e) => {
                                    output.errors.push(format!("HostCall::Spawn error: {e}"));
                                    HostValue::Null
                                }
                            }
                        }
                        HostCallKind::Register(ce) => {
                            match crate::meow_meow::component_registry::spawn_tree_uninitialized(
                                &ce, world, emit,
                            ) {
                                Ok(component_id) => HostValue::ComponentId(component_id),
                                Err(e) => {
                                    output.errors.push(format!("HostCall::Register error: {e}"));
                                    HostValue::Null
                                }
                            }
                        }
                        HostCallKind::Attach { parent, child } => {
                            if let Some(p) = parent {
                                if let Err(e) = world.add_child(p, child) {
                                    output.errors.push(format!("HostCall::Attach error: {e}"));
                                }
                            }
                            // Run the deferred init walk on the (now-attached, or root) subtree.
                            world.init_component_tree(child, emit);
                            HostValue::Null
                        }
                        HostCallKind::Query { selector, scope, multiple } => {
                            let roots: Vec<crate::engine::ecs::ComponentId> = match scope {
                                Some(id) => vec![id],
                                None => world
                                    .all_components()
                                    .filter(|&id| world.parent_of(id).is_none())
                                    .collect(),
                            };
                            let mut all_ids: Vec<crate::engine::ecs::ComponentId> = Vec::new();
                            for r in roots {
                                if multiple {
                                    all_ids.extend(world.find_all_components(r, &selector));
                                } else if let Some(found) = world.find_component(r, &selector) {
                                    all_ids.push(found);
                                    break;
                                }
                            }
                            if multiple {
                                let list = all_ids
                                    .into_iter()
                                    .filter_map(|id| {
                                        world.component_name(id).map(|t| (id, t.to_string()))
                                    })
                                    .collect();
                                HostValue::ComponentList(list)
                            } else {
                                match all_ids.into_iter().next() {
                                    Some(id) => match world.component_name(id) {
                                        Some(t) => HostValue::Component {
                                            id,
                                            component_type: t.to_string(),
                                        },
                                        None => HostValue::Null,
                                    },
                                    None => HostValue::Null,
                                }
                            }
                        }
                        HostCallKind::RegisterHandler { scope, signal_kind, handler } => {
                            rx.add_handler_closure(
                                signal_kind,
                                scope,
                                move |world, emit, _signal| {
                                    if let Err(e) = eval_mms_fn(&handler, vec![Value::Null], Some(world), Some(emit)) {
                                        eprintln!("[mms] handler error: {e}");
                                    }
                                },
                            );
                            HostValue::Null
                        }
                    };
                    let _ = handle.requests.push(EvalRequest::HostCallResult { id, value: reply });
                }
                Err(rtrb::PopError::Empty) => {
                    if Instant::now() > deadline {
                        output.errors.push("MeowMeowRunner: timed out waiting for evaluator".into());
                        break;
                    }
                    std::thread::yield_now();
                }
            }
        }

        handle.shutdown_and_join();
        output
    }

    fn eval_impl(source: &str, source_path: Option<&str>, timeout: Duration) -> EvalOutput {
        let mut handle = MeowMeowEvaluator::spawn(64);

        handle
            .requests
            .push(EvalRequest::EvalScript {
                source: source.to_string(),
                source_path: source_path.map(|s| s.to_string()),
            })
            .expect("MeowMeowRunner: push EvalScript");
        handle
            .requests
            .push(EvalRequest::Shutdown)
            .expect("MeowMeowRunner: push Shutdown");

        let mut output = EvalOutput::default();
        let deadline = Instant::now() + timeout;

        loop {
            match handle.responses.pop() {
                Ok(EvalResponse::Intent(iv)) => output.intents.push(iv),
                Ok(EvalResponse::Error { message }) => output.errors.push(message),
                Ok(EvalResponse::ParsedOk { .. }) => {}
                Ok(EvalResponse::ShutdownAck) => break,
                // Fire-and-forget runner has no world — reply null so the evaluator
                // falls back to ComponentExpr and continues without blocking.
                Ok(EvalResponse::HostCall { id, .. }) => {
                    let _ = handle.requests.push(EvalRequest::HostCallResult {
                        id,
                        value: HostValue::Null,
                    });
                }
                Err(rtrb::PopError::Empty) => {
                    if Instant::now() > deadline {
                        output.errors.push("MeowMeowRunner: timed out waiting for evaluator".into());
                        break;
                    }
                    std::thread::yield_now();
                }
            }
        }

        handle.shutdown_and_join();
        output
    }
}
