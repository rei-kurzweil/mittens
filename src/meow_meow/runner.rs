use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::engine::memory_trace;
use crate::engine::ecs::{ComponentId, IntentValue, RxWorld, SignalEmitter, World};
use crate::meow_meow::evaluator::{
    EvalRequest, EvalResponse, HostCallKind, HostValue, MeowMeowEvaluator, eval_mms_fn,
    eval_module_source,
};
use crate::meow_meow::object::{MaterializedCE, Value};

/// The result of evaluating an MMS script: collected intents and any errors.
#[derive(Debug, Default)]
pub struct EvalOutput {
    pub intents: Vec<IntentValue>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LoadedMmsModule {
    pub named_exports: HashMap<String, Value>,
    pub sequence: Vec<MaterializedCE>,
    pub source_path: Option<String>,
}

impl LoadedMmsModule {
    pub fn named_export(&self, name: &str) -> Option<&Value> {
        self.named_exports.get(name)
    }
}

/// Synchronous wrapper around [`MeowMeowEvaluator`].
///
/// Spawns an evaluator thread, sends a script, drains all responses to
/// completion, and returns the collected [`EvalOutput`]. The thread is shut
/// down and joined before returning.
pub struct MeowMeowRunner;

impl MeowMeowRunner {
    fn trace_module_load(label: &str, path: &str) {
        memory_trace::log_line(format!("\n🐈 [startup-memory] {label} path={path}"));
        memory_trace::sample(&format!("🐈 {label} path={path}"), None);
    }

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
                output
                    .errors
                    .push(format!("cannot read file '{}': {}", path, e));
                output
            }
        }
    }

    pub fn load_module_source(
        source: &str,
        source_path: Option<&str>,
    ) -> Result<LoadedMmsModule, String> {
        if let Some(path) = source_path {
            Self::trace_module_load("mms load_module_source:start", path);
        }
        let module = match eval_module_source(source, source_path)? {
            Value::Module { named, sequence } => Ok(LoadedMmsModule {
                named_exports: named,
                sequence,
                source_path: source_path.map(|s| s.to_string()),
            }),
            other => Err(format!(
                "load_module_source: expected module result, got {:?}",
                other
            )),
        }?;
        if let Some(path) = source_path {
            Self::trace_module_load("mms load_module_source:end", path);
        }
        Ok(module)
    }

    pub fn load_module_file(path: &str) -> Result<LoadedMmsModule, String> {
        Self::trace_module_load("mms load_module_file:start", path);
        let source = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read module '{}': {}", path, e))?;
        Self::trace_module_load("mms load_module_file:after read_to_string", path);
        let module = Self::load_module_source(&source, Some(path))?;
        Self::trace_module_load("mms load_module_file:end", path);
        Ok(module)
    }

    pub fn call_mms_module_fn(
        module: &LoadedMmsModule,
        name: &str,
        args: Vec<Value>,
        channels: Option<&mut crate::meow_meow::evaluator::EvalChannels>,
        world_host: Option<&mut World>,
        emit: Option<&mut dyn SignalEmitter>,
    ) -> Result<Value, String> {
        let Some(export) = module.named_export(name) else {
            return Err(format!("call_mms_module_fn: export '{}' not found", name));
        };
        if !matches!(export, Value::Function { .. }) {
            return Err(format!(
                "call_mms_module_fn: export '{}' is not a function",
                name
            ));
        }
        eval_mms_fn(export, args, channels, world_host, emit)
    }

    pub fn materialize_mms_module_component(
        module: &LoadedMmsModule,
        name: &str,
        args: Vec<Value>,
        world_host: Option<&mut World>,
        emit: Option<&mut dyn SignalEmitter>,
    ) -> Result<MaterializedCE, String> {
        let value = Self::call_mms_module_fn(module, name, args, None, world_host, emit)?;
        let Value::ComponentExpr(component_expr) = value else {
            return Err(format!(
                "materialize_mms_module_component: export '{}' did not return a component tree",
                name
            ));
        };
        Ok(*component_expr)
    }

    pub fn materialize_mms_module_component_from_file(
        path: &str,
        name: &str,
        args: Vec<Value>,
        world_host: Option<&mut World>,
        emit: Option<&mut dyn SignalEmitter>,
    ) -> Result<MaterializedCE, String> {
        let module = Self::load_module_file(path)?;
        Self::materialize_mms_module_component(&module, name, args, world_host, emit)
    }

    pub fn spawn_mms_module_component_uninitialized(
        module: &LoadedMmsModule,
        name: &str,
        args: Vec<Value>,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
    ) -> Result<ComponentId, String> {
        let component_expr =
            Self::materialize_mms_module_component(module, name, args, Some(world), Some(emit))?;
        crate::meow_meow::component_registry::spawn_tree_uninitialized(&component_expr, world, emit)
    }

    pub fn spawn_mms_module_component_uninitialized_from_file(
        path: &str,
        name: &str,
        args: Vec<Value>,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
    ) -> Result<ComponentId, String> {
        let module = Self::load_module_file(path)?;
        Self::spawn_mms_module_component_uninitialized(&module, name, args, world, emit)
    }

    pub fn spawn_mms_module_component(
        module: &LoadedMmsModule,
        name: &str,
        args: Vec<Value>,
        parent: Option<ComponentId>,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
    ) -> Result<ComponentId, String> {
        let component_expr =
            Self::materialize_mms_module_component(module, name, args, Some(world), Some(emit))?;
        crate::meow_meow::component_registry::spawn_tree(&component_expr, parent, world, emit)
    }

    pub fn spawn_mms_module_component_from_file(
        path: &str,
        name: &str,
        args: Vec<Value>,
        parent: Option<ComponentId>,
        world: &mut World,
        emit: &mut dyn SignalEmitter,
    ) -> Result<ComponentId, String> {
        let module = Self::load_module_file(path)?;
        Self::spawn_mms_module_component(&module, name, args, parent, world, emit)
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
        Self::eval_with_world_at_path(source, None, world, rx, emit)
    }

    /// Like `eval_with_world`, but also records the source file path so
    /// `import` statements resolve relative to it.
    pub fn eval_with_world_at_path(
        source: &str,
        source_path: Option<&str>,
        world: &mut World,
        rx: &mut RxWorld,
        emit: &mut dyn SignalEmitter,
    ) -> EvalOutput {
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
                        HostCallKind::Query {
                            selector,
                            scope,
                            multiple,
                        } => {
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
                        HostCallKind::RegisterHandler {
                            scope,
                            signal_kind,
                            name,
                            handler,
                        } => {
                            let callback =
                                move |world: &mut World,
                                      emit: &mut dyn SignalEmitter,
                                      signal: &crate::engine::ecs::Signal| {
                                    let arg = match signal.event.as_ref() {
                                        Some(crate::engine::ecs::EventSignal::DataEvent {
                                            name,
                                            ..
                                        }) => Value::String(name.clone()),
                                        _ => Value::Null,
                                    };
                                    if let Err(e) = eval_mms_fn(
                                        &handler,
                                        vec![arg],
                                        None,
                                        Some(world),
                                        Some(emit),
                                    ) {
                                        eprintln!("[mms] handler error: {e}");
                                    }
                                };
                            if let Some(name) = name {
                                rx.add_handler_closure_named(
                                    signal_kind,
                                    scope,
                                    Some(name),
                                    callback,
                                );
                            } else {
                                rx.add_handler_closure(signal_kind, scope, callback);
                            }
                            HostValue::Null
                        }
                        HostCallKind::AudioClipInstance {
                            source,
                            start_beat,
                            stop_beat,
                        } => {
                            use crate::engine::ecs::component::AudioClipComponent;
                            match world.get_component_by_id_as::<AudioClipComponent>(source) {
                                Some(src) => {
                                    let mut c = AudioClipComponent::instance_of(src);
                                    if let Some(sb) = start_beat {
                                        c.start_beat = sb;
                                    }
                                    if let Some(eb) = stop_beat {
                                        c.stop_beat = Some(eb);
                                    }
                                    let id = world.add_component(c);
                                    HostValue::ComponentId(id)
                                }
                                None => {
                                    output.errors.push(
                                        "HostCall::AudioClipInstance: source is not an AudioClip"
                                            .to_string(),
                                    );
                                    HostValue::Null
                                }
                            }
                        }
                    };
                    let _ = handle
                        .requests
                        .push(EvalRequest::HostCallResult { id, value: reply });
                }
                Err(rtrb::PopError::Empty) => {
                    if Instant::now() > deadline {
                        output
                            .errors
                            .push("MeowMeowRunner: timed out waiting for evaluator".into());
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
                        output
                            .errors
                            .push("MeowMeowRunner: timed out waiting for evaluator".into());
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
