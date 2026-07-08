use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use rtrb::{Consumer, Producer, RingBuffer};

use crate::engine::ecs::ComponentId;
use crate::engine::ecs::IntentValue;
use crate::engine::ecs::SignalEmitter;
use crate::engine::ecs::SignalKind;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{AnimationState, ComponentRef, MusicNote};
use crate::meow_meow::ast::{
    BinOpKind, CallExpression, ComponentExpression, ElseBranch, Expression, IfStatement,
    ImportItem, Statement, UnaryOpKind,
};
use crate::meow_meow::block_effect_analyzer::BlockEffectAnalyzer;
use crate::meow_meow::component_method_registry::{
    invoke_component_method, supports_component_method,
};
use crate::meow_meow::component_registry::{
    component_expr_uses_property_assignment_only, is_universal_component_named_prop,
};
use crate::meow_meow::object::{
    BuiltinTableKind, CeChild, FrameKind, MaterializedCE, Object, ObjectWorld, RuntimeClosure,
    Value,
};
use crate::meow_meow::parser::{MeowMeowParser, ParseError};
use crate::meow_meow::token::TokenizeError;
use crate::meow_meow::tokenizer::MeowMeowTokenizer;
use crate::meow_meow::transform::{EmitLiftTransform, QueryDesugarTransform};

// ---------------------------------------------------------------------------
// Thread protocol
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum EvalRequest {
    /// Parse and evaluate a script. Emitted `SpawnComponentTree` intents come back
    /// as `EvalResponse::Intent` messages.
    EvalScript {
        source: String,
        source_path: Option<String>,
    },
    /// Parse only — returns a debug AST string (used in tests / tooling).
    ParseScript {
        source: String,
    },
    /// Reply to a pending `EvalResponse::HostCall`. The `id` must match the
    /// correlation id from the HostCall that is being answered.
    HostCallResult {
        id: u32,
        value: HostValue,
    },
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum EvalResponse {
    /// A `SpawnComponentTree` (or other) intent ready to be pushed into the engine.
    Intent(IntentValue),
    /// Parse-only debug output (from `ParseScript`).
    ParsedOk {
        debug_ast: String,
    },
    Error {
        message: String,
    },
    ShutdownAck,
    /// The evaluator needs the host to perform a side-effecting operation and
    /// return a result before evaluation can continue. The host must push a
    /// matching `EvalRequest::HostCallResult { id, value }` to unblock the
    /// evaluator thread.
    HostCall {
        id: u32,
        kind: HostCallKind,
    },
}

/// Operations the evaluator can request from the host.
#[derive(Debug, Clone)]
pub enum HostCallKind {
    /// Spawn a component tree and return its root `ComponentId`.
    /// Used for fire-and-forget root emissions (currently unused by the
    /// evaluator — top-level CEs are still pushed as `IntentValue` for now).
    Spawn(MaterializedCE),
    /// Create the component tree in the world but do **not** attach it to a
    /// parent and do **not** run init. Returns the root `ComponentId`. The
    /// caller (typically `let x = CE`) holds the id as a `ComponentObject`
    /// and decides where/when to splice the subtree in.
    Register(MaterializedCE),
    /// Attach a previously `Register`ed (or `Spawn`ed) detached subtree to a
    /// parent and run the deferred init walk. With `parent: None` the subtree
    /// is initialised in place as a world root.
    Attach {
        parent: Option<ComponentId>,
        child: ComponentId,
    },
    /// Register an MMS function as a scoped signal handler.
    /// The host installs the closure and replies with `HostValue::Null`.
    RegisterHandler {
        scope: ComponentId,
        signal_kind: SignalKind,
        name: Option<String>,
        handler: Value,
    },
    /// Query the live ECS world. `scope = None` means search from the world's
    /// canonical roots; `scope = Some(id)` restricts the search to the
    /// subtree rooted at `id`. `multiple` selects between `query_all`
    /// (`true`, returns `ComponentList`) and `query` (`false`, returns the
    /// first match as `Component` or `Null` if none).
    Query {
        selector: String,
        scope: Option<ComponentId>,
        multiple: bool,
    },
    /// Create a new `AudioClipComponent` that shares `source`'s decoded
    /// asset but gets its own playhead (RT instance). Returns the new
    /// component's id, detached — mirrors `Register` semantics so the
    /// caller can splice it via the usual CE-body bare-reference path.
    /// See docs/draft/audio-clip-instance-cloning.md §3.
    AudioClipInstance {
        source: ComponentId,
        start_beat: Option<f64>,
        stop_beat: Option<f64>,
    },
    InvokeComponentMethod {
        id: ComponentId,
        component_type: String,
        method: String,
        args: Vec<Value>,
    },
}

/// Values the host can return in response to a `HostCall`.
#[derive(Debug, Clone)]
pub enum HostValue {
    ComponentId(ComponentId),
    Component {
        id: ComponentId,
        component_type: String,
    },
    ComponentList(Vec<(ComponentId, String)>),
    Null,
}

// ---------------------------------------------------------------------------
// Handle
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct MeowMeowEvaluatorHandle {
    pub requests: Producer<EvalRequest>,
    pub responses: Consumer<EvalResponse>,
    join: Option<JoinHandle<()>>,
}

impl MeowMeowEvaluatorHandle {
    pub fn shutdown_and_join(mut self) {
        let _ = self.requests.push(EvalRequest::Shutdown);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

pub struct MeowMeowEvaluator;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RuntimeClosureExecMode {
    Full,
    KeyframeAudioOnly { beat_context: f64 },
    KeyframeVisualOnly,
}

impl MeowMeowEvaluator {
    pub fn spawn(queue_capacity: usize) -> MeowMeowEvaluatorHandle {
        let (req_prod, req_cons) = RingBuffer::<EvalRequest>::new(queue_capacity);
        let (res_prod, res_cons) = RingBuffer::<EvalResponse>::new(queue_capacity);
        let join = thread::spawn(move || evaluator_thread(req_cons, res_prod));
        MeowMeowEvaluatorHandle {
            requests: req_prod,
            responses: res_cons,
            join: Some(join),
        }
    }
}

// ---------------------------------------------------------------------------
// Worker thread
// ---------------------------------------------------------------------------

fn evaluator_thread(requests: Consumer<EvalRequest>, responses: Producer<EvalResponse>) {
    let mut ch = EvalChannels::new(requests, responses);
    loop {
        if ch.shutdown_requested {
            while ch.responses.push(EvalResponse::ShutdownAck).is_err() {
                std::thread::yield_now();
            }
            break;
        }

        match ch.requests.pop() {
            Ok(EvalRequest::EvalScript {
                source,
                source_path,
            }) => {
                eval_script(&source, source_path.as_deref(), &mut ch);
            }
            Ok(EvalRequest::ParseScript { source }) => {
                let resp = parse_only(&source)
                    .map(|dbg| EvalResponse::ParsedOk { debug_ast: dbg })
                    .unwrap_or_else(|msg| EvalResponse::Error { message: msg });
                let _ = ch.responses.push(resp);
            }
            Ok(EvalRequest::HostCallResult { .. }) => {
                // HostCallResult arriving outside of a spin-wait means the host
                // sent a stale reply. Discard silently.
            }
            Ok(EvalRequest::Shutdown) => {
                let _ = ch.responses.push(EvalResponse::ShutdownAck);
                break;
            }
            Err(rtrb::PopError::Empty) => {
                std::thread::yield_now();
            }
        }
    }
}

/// Emit a `HostCall` and spin-wait for the matching `HostCallResult`.
///
/// Blocks the evaluator thread until the host pushes `HostCallResult { id, value }`.
/// Any `HostCallResult` with a non-matching id is discarded (stale reply).
/// Other request kinds (e.g. Shutdown) are processed normally while waiting.
///
/// Returns `None` if the host sent `HostValue::Null` or if a Shutdown arrived
/// before the reply.
fn host_call(
    id: u32,
    kind: HostCallKind,
    requests: &mut Consumer<EvalRequest>,
    responses: &mut Producer<EvalResponse>,
    shutdown_requested: &mut bool,
) -> Option<HostValue> {
    while responses
        .push(EvalResponse::HostCall {
            id,
            kind: kind.clone(),
        })
        .is_err()
    {
        std::thread::yield_now();
    }
    loop {
        match requests.pop() {
            Ok(EvalRequest::HostCallResult {
                id: reply_id,
                value,
            }) if reply_id == id => {
                return match value {
                    HostValue::Null => None,
                    v => Some(v),
                };
            }
            Ok(EvalRequest::HostCallResult { .. }) => {
                // Stale reply for a different id — discard.
            }
            Ok(EvalRequest::Shutdown) => {
                *shutdown_requested = true;
            }
            Ok(other) => {
                // Other requests (unlikely mid-eval) — re-queue by yielding;
                // in practice only HostCallResult and Shutdown arrive here.
                let _ = other; // consumed, cannot re-push to Consumer
            }
            Err(rtrb::PopError::Empty) => {
                std::thread::yield_now();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Script evaluation
// ---------------------------------------------------------------------------

/// Shared mutable context threaded through evaluation.
///
/// Carries everything that eval functions need beyond the immutable `env`.
/// Adding future context (module cache, call-depth limit, …) is a one-field
/// change here rather than a signature change across every eval function.
struct EvalContext<'a> {
    /// Accumulates `SpawnComponentTree` intents produced by top-level CE emissions.
    emits: &'a mut Vec<IntentValue>,
    /// Filesystem path of the file being evaluated, used to resolve relative
    /// `import "…"` paths. `None` inside function call bodies and when
    /// source was provided as a raw string without a path.
    source_path: Option<&'a str>,
    /// Channel back to the host for HostCall round-trips (reply channel).
    /// `None` when running in fire-and-forget mode (no world access available).
    channels: Option<&'a mut EvalChannels>,
    /// When evaluating inside a CE body block, captures children, builder calls,
    /// positionals, and named assignments instead of emitting to the top level.
    ce_builder: Option<&'a mut CeBuilder>,
    /// Scripting-side runtime storage: variable scope chain (frames) + heap.
    /// All variable bindings, lookups, and reassignments go through this.
    object_world: &'a mut ObjectWorld,
    /// Inline live-world host access used when MMS runs directly inside a
    /// registered engine handler rather than through evaluator channels.
    host_world: Option<*mut World>,
    /// Live component subtree that owns the currently executing imperative block.
    exec_scope: Option<ComponentId>,
    /// Execution policy for deferred runtime closures such as `Keyframe` callbacks.
    runtime_closure_mode: RuntimeClosureExecMode,
}

thread_local! {
    static LIVE_SIGNAL_EMITTER: RefCell<Option<*mut dyn SignalEmitter>> = RefCell::new(None);
}

fn with_live_signal_emitter<R>(emit: Option<*mut dyn SignalEmitter>, f: impl FnOnce() -> R) -> R {
    LIVE_SIGNAL_EMITTER.with(|slot| {
        let prev = slot.replace(emit);
        let result = f();
        let _ = slot.replace(prev);
        result
    })
}

fn push_eval_intent(ctx: &mut EvalContext<'_>, mut intent: IntentValue) {
    match ctx.runtime_closure_mode {
        RuntimeClosureExecMode::Full => {}
        RuntimeClosureExecMode::KeyframeAudioOnly { beat_context } => match &mut intent {
            IntentValue::AudioSchedulePlay {
                beat_context: signal_beat_context,
                ..
            }
            | IntentValue::OscillatorScheduleSetPitch {
                beat_context: signal_beat_context,
                ..
            } => {
                *signal_beat_context = Some(beat_context);
            }
            _ => return,
        },
        RuntimeClosureExecMode::KeyframeVisualOnly => match intent {
            IntentValue::AudioSchedulePlay { .. } | IntentValue::OscillatorScheduleSetPitch { .. } => {
                return;
            }
            _ => {}
        },
    }

    ctx.emits.push(intent);
}

/// Accumulator used while evaluating a component expression body block.
/// Collects the results that will be materialized into a `MaterializedCE`.
struct CeBuilder {
    /// Whether `name = expr` in this CE body should be captured as a named prop.
    component_property_assignment_only: bool,
    /// Remaining chained ctor calls + body builder calls (calls to names not in env).
    calls: Vec<(String, Vec<Value>)>,
    /// Named property assignments (`name = expr` in body where name was pre-injected).
    named: Vec<(String, Value)>,
    /// String-type positional content (e.g. `"hello " + name` in Text body).
    positionals: Vec<Value>,
    /// Child entries in source order — either fresh CEs to spawn or
    /// pre-Registered ComponentIds to splice in.
    children: Vec<CeChild>,
}

/// Live request/response channels plus a monotonic correlation-id counter.
/// Owned by the evaluator thread; passed as `&mut` into eval functions.
pub struct EvalChannels {
    pub requests: Consumer<EvalRequest>,
    pub responses: Producer<EvalResponse>,
    next_id: u32,
    shutdown_requested: bool,
}

impl EvalChannels {
    pub fn new(requests: Consumer<EvalRequest>, responses: Producer<EvalResponse>) -> Self {
        Self {
            requests,
            responses,
            next_id: 0,
            shutdown_requested: false,
        }
    }

    /// Emit a `HostCall` and spin-wait for the matching `HostCallResult`.
    pub fn call(&mut self, kind: HostCallKind) -> Option<HostValue> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        host_call(
            id,
            kind,
            &mut self.requests,
            &mut self.responses,
            &mut self.shutdown_requested,
        )
    }
}

/// Evaluate a script: parse → AstTransforms → walk statements.
/// `ch` is the owned channel pair for the evaluator thread; the borrow is
/// released before intents are flushed so `ch.responses` is free to use again.
fn eval_script(source: &str, source_path: Option<&str>, ch: &mut EvalChannels) {
    let mut stmts = match parse_source(source) {
        Ok(s) => s,
        Err(msg) => {
            let _ = ch.responses.push(EvalResponse::Error { message: msg });
            return;
        }
    };

    EmitLiftTransform::apply(&mut stmts);
    QueryDesugarTransform::apply(&mut stmts);

    let mut emits: Vec<IntentValue> = Vec::new();
    let mut world = ObjectWorld::new();

    // Borrow `ch` into the context for the duration of eval, then release it.
    let eval_result = {
        let mut ctx = EvalContext {
            emits: &mut emits,
            source_path,
            channels: Some(ch),
            ce_builder: None,
            object_world: &mut world,
            host_world: None,
            exec_scope: None,
            runtime_closure_mode: RuntimeClosureExecMode::Full,
        };
        eval_block_stmts(&stmts, &mut ctx)
    }; // ctx (and its borrow of ch) dropped here

    match eval_result {
        Ok(_) => {}
        Err(msg) => {
            let _ = ch.responses.push(EvalResponse::Error { message: msg });
            return;
        }
    }

    for intent in emits {
        while ch
            .responses
            .push(EvalResponse::Intent(intent.clone()))
            .is_err()
        {
            std::thread::yield_now();
        }
    }
}

fn flush_live_statement_emits(ctx: &mut EvalContext<'_>) {
    if ctx.channels.is_none() || ctx.ce_builder.is_some() || ctx.emits.is_empty() {
        return;
    }

    let mut deferred: Vec<IntentValue> = Vec::new();
    for intent in std::mem::take(ctx.emits) {
        match intent {
            IntentValue::SpawnComponentTree { root, parent: None } => {
                if let Some(ch) = ctx.channels.as_mut() {
                    let spawn_result = ch.call(HostCallKind::Spawn((*root).clone()));
                    if !matches!(spawn_result, Some(HostValue::ComponentId(_))) {
                        deferred.push(IntentValue::SpawnComponentTree { root, parent: None });
                    }
                } else {
                    deferred.push(IntentValue::SpawnComponentTree { root, parent: None });
                }
            }
            other => deferred.push(other),
        }
    }
    *ctx.emits = deferred;
}

// ---------------------------------------------------------------------------
// Statement evaluation
// ---------------------------------------------------------------------------

/// Effect produced by evaluating a statement (excluding emits, which go to the emits vec).
///
/// Bindings, reassignments, and import-bindings are applied directly to
/// `ctx.object_world` inside `eval_stmt` — they do not flow through this enum.
/// Only control-flow effects and the `Exported` marker bubble out.
enum StmtEffect {
    None,
    /// `export let X = ...` — the binding has already been written to
    /// `object_world`; this signals that the module body should also register
    /// the name in its named-exports map.
    Exported(String),
    Return(Value),
    Break,
    Continue,
}

/// Evaluate a block of statements against the current top frame of
/// `ctx.object_world`. Returns the first control-flow effect encountered
/// (Return/Break/Continue), `Exported(name)` (only consumed by module body),
/// or `None`.
///
/// Frame management is the *caller*'s responsibility: this function does not
/// push or pop. Function calls, loops, if-bodies, etc. wrap the call.
fn eval_block_stmts(stmts: &[Statement], ctx: &mut EvalContext<'_>) -> Result<StmtEffect, String> {
    for stmt in stmts {
        let effect = eval_stmt(stmt, ctx)?;
        flush_live_statement_emits(ctx);
        match effect {
            StmtEffect::None => {}
            // Exported is only meaningful at module-body level; ignored elsewhere.
            StmtEffect::Exported(_) => {}
            effect => return Ok(effect),
        }
    }
    Ok(StmtEffect::None)
}

fn eval_stmt(stmt: &Statement, ctx: &mut EvalContext<'_>) -> Result<StmtEffect, String> {
    match stmt {
        Statement::Assignment(a) => {
            let val = eval_expr(&a.value, ctx)?;
            let val = maybe_register_live_component_value(val, ctx);
            ctx.object_world.bind(a.name.0.clone(), val);
            if a.exported {
                Ok(StmtEffect::Exported(a.name.0.clone()))
            } else {
                Ok(StmtEffect::None)
            }
        }
        Statement::Reassign { target, value } => {
            let val = eval_expr(value, ctx)?;
            if let Some(builder) = ctx.ce_builder.as_mut() {
                if let Expression::Identifier(name) = target {
                    if builder.component_property_assignment_only
                        || is_universal_component_named_prop(&name.0)
                    {
                        // Property-bag CE bodies capture assignments as named props.
                        // This must win even when `foo` also exists as a lexical binding,
                        // so authored payloads like `row_name = row_name` survive.
                        builder.named.push((name.0.clone(), val));
                        return Ok(StmtEffect::None);
                    }
                }
            }
            let val = maybe_register_live_component_value(val, ctx);
            assign_retarget(target, val, ctx)?;
            Ok(StmtEffect::None)
        }
        Statement::Expression(expr) => {
            eval_expr_stmt(expr, ctx)?;
            Ok(StmtEffect::None)
        }
        Statement::Return(r) => {
            let val = match &r.value {
                Some(expr) => eval_expr(expr, ctx)?,
                None => Value::Null,
            };
            Ok(StmtEffect::Return(val))
        }
        Statement::If(if_stmt) => eval_if(if_stmt, ctx),
        Statement::Block(block) => {
            ctx.object_world.push_frame(FrameKind::Block);
            let result = eval_block_stmts(&block.statements, ctx);
            ctx.object_world.pop_frame();
            result
        }
        Statement::Break => Ok(StmtEffect::Break),
        Statement::Continue => Ok(StmtEffect::Continue),
        Statement::ForIn {
            binding,
            iterable,
            body,
        } => {
            let items = match eval_expr(iterable, ctx)? {
                Value::Array(a) => a,
                Value::Map(map) => map
                    .into_iter()
                    .map(|(key, value)| {
                        Value::Map(HashMap::from([
                            ("key".to_string(), Value::String(key)),
                            ("value".to_string(), value),
                        ]))
                    })
                    .collect(),
                Value::Object(id) => {
                    let Some(items) = id.with_map(|map| {
                        map.iter()
                            .map(|(key, value)| {
                                Value::Map(HashMap::from([
                                    ("key".to_string(), Value::String(key.clone())),
                                    ("value".to_string(), value.clone()),
                                ]))
                            })
                            .collect::<Vec<_>>()
                    }) else {
                        return Err("for/in: invalid object".into());
                    };
                    items
                }
                other => return Err(format!("for/in: expected array, got {:?}", other)),
            };
            ctx.object_world.push_frame(FrameKind::Block);
            let result: Result<StmtEffect, String> = (|| {
                'for_loop: for item in items {
                    ctx.object_world.bind(binding.0.clone(), item);
                    for stmt in &body.statements {
                        match eval_stmt(stmt, ctx)? {
                            StmtEffect::None | StmtEffect::Exported(_) => {}
                            StmtEffect::Return(val) => return Ok(StmtEffect::Return(val)),
                            StmtEffect::Break => return Ok(StmtEffect::None),
                            StmtEffect::Continue => continue 'for_loop,
                        }
                    }
                }
                Ok(StmtEffect::None)
            })();
            ctx.object_world.pop_frame();
            result
        }
        Statement::While { condition, body } => {
            ctx.object_world.push_frame(FrameKind::Block);
            let result: Result<StmtEffect, String> = (|| {
                'while_loop: loop {
                    let cond = eval_expr(condition, ctx)?;
                    if !is_truthy(&cond) {
                        break;
                    }
                    for stmt in &body.statements {
                        match eval_stmt(stmt, ctx)? {
                            StmtEffect::None | StmtEffect::Exported(_) => {}
                            StmtEffect::Return(val) => return Ok(StmtEffect::Return(val)),
                            StmtEffect::Break => break 'while_loop,
                            StmtEffect::Continue => continue 'while_loop,
                        }
                    }
                }
                Ok(StmtEffect::None)
            })();
            ctx.object_world.pop_frame();
            result
        }
        Statement::Import { items, path } => {
            let resolved = resolve_import_path(path, ctx.source_path);
            let content = std::fs::read_to_string(&resolved)
                .map_err(|e| format!("import error: cannot read '{}': {}", path, e))?;
            let module_val = eval_module_source(&content, Some(&resolved))?;
            let (named, sequence) = match module_val {
                Value::Module {
                    named, sequence, ..
                } => (named, sequence),
                _ => return Err("import: internal error".to_string()),
            };
            for item in items {
                match item {
                    ImportItem::Named(id) => {
                        let val = named.get(&id.0).cloned().ok_or_else(|| {
                            format!("import: '{}' is not exported from '{}'", id.0, path)
                        })?;
                        ctx.object_world.bind(id.0.clone(), val);
                    }
                    ImportItem::NamedAlias { name, alias } => {
                        let val = named.get(&name.0).cloned().ok_or_else(|| {
                            format!("import: '{}' is not exported from '{}'", name.0, path)
                        })?;
                        ctx.object_world.bind(alias.0.clone(), val);
                    }
                    ImportItem::PositionalAlias { index, alias } => {
                        let ce = sequence.get(*index).ok_or_else(|| {
                            format!("import: index {} out of range in '{}'", index, path)
                        })?;
                        ctx.object_world
                            .bind(alias.0.clone(), Value::ComponentExpr(Box::new(ce.clone())));
                    }
                }
            }
            Ok(StmtEffect::None)
        }
    }
}

fn maybe_register_live_component_value(val: Value, ctx: &mut EvalContext<'_>) -> Value {
    // In live mode, binding or reassigning a CE should produce a live handle
    // rather than leave a dead ComponentExpr in scope.
    match val {
        Value::ComponentExpr(ce) => {
            let component_type = ce.component_type.clone();
            if let Some(ch) = ctx.channels.as_mut() {
                return match ch.call(HostCallKind::Register(*ce.clone())) {
                    Some(HostValue::ComponentId(id)) => Value::ComponentObject { id, component_type },
                    _ => Value::ComponentExpr(ce),
                };
            }
            if let Some(world) = ctx.host_world {
                let registered = LIVE_SIGNAL_EMITTER.with(|slot| {
                    let Some(host_emit) = *slot.borrow() else {
                        return None;
                    };
                    unsafe {
                        crate::meow_meow::component_registry::spawn_tree_uninitialized(
                            &ce,
                            &mut *world,
                            &mut *host_emit,
                        )
                        .ok()
                    }
                });
                if let Some(id) = registered {
                    return Value::ComponentObject { id, component_type };
                }
            }
            Value::ComponentExpr(ce)
        }
        val => val,
    }
}

fn eval_expr_stmt(expr: &Expression, ctx: &mut EvalContext<'_>) -> Result<(), String> {
    // Special case: emit(expr) — produced by EmitLiftTransform or written explicitly.
    if let Expression::Call(call) = expr {
        if matches!(call.callee.as_ref(), Expression::Identifier(id) if id.0 == "emit") {
            if let Some(arg) = call.args.first() {
                let val = eval_expr(arg, ctx)?;
                push_component_emit(val, ctx);
            }
            return Ok(());
        }

        // Builder call interception: inside a CE body, plain calls to names not in env
        // and not built-ins are captured as builder calls rather than erroring.
        if let Expression::Identifier(callee_id) = call.callee.as_ref() {
            if ctx.ce_builder.is_some()
                && !ctx.object_world.has(&callee_id.0)
                && !is_builtin_fn(&callee_id.0)
            {
                let args: Vec<Value> = call
                    .args
                    .iter()
                    .map(|a| eval_expr(a, ctx))
                    .collect::<Result<_, _>>()?;
                ctx.ce_builder
                    .as_mut()
                    .unwrap()
                    .calls
                    .push((callee_id.0.clone(), args));
                return Ok(());
            }
        }
    }

    // General case: evaluate and route result.
    let val = eval_expr(expr, ctx)?;
    if ctx.ce_builder.is_some() {
        match val {
            // String positionals captured in CE body.
            Value::String(_) => ctx.ce_builder.as_mut().unwrap().positionals.push(val),
            // Fresh CE children captured in CE body.
            Value::ComponentExpr(ce) => ctx
                .ce_builder
                .as_mut()
                .unwrap()
                .children
                .push(CeChild::Spawn(*ce)),
            // Reference to a previously Registered live component — splice
            // the detached subtree as a child of the parent CE rather than
            // discarding the value or re-spawning it.
            Value::ComponentObject { id, .. } => {
                ctx.ce_builder
                    .as_mut()
                    .unwrap()
                    .children
                    .push(CeChild::Attach(id));
            }
            // Other values inside a CE body are discarded (no-op expression statements).
            _ => {}
        }
    } else {
        push_component_emit(val, ctx);
    }
    Ok(())
}

fn push_component_emit(val: Value, ctx: &mut EvalContext<'_>) {
    match val {
        Value::ComponentExpr(ce) => {
            push_eval_intent(ctx, IntentValue::SpawnComponentTree {
                root: ce,
                parent: None,
            });
        }
        // Bare top-level reference to a previously Registered ComponentObject —
        // attach as a world root and run the deferred init walk.
        Value::ComponentObject { id, .. } => {
            if let Some(ch) = ctx.channels.as_mut() {
                ch.call(HostCallKind::Attach {
                    parent: None,
                    child: id,
                });
            }
        }
        _ => {}
    }
}

fn value_to_component_ref_live(world: &World, value: &Value) -> Result<ComponentRef, String> {
    match value {
        Value::ComponentObject { id, .. } => {
            let guid = world
                .get_component_record(*id)
                .map(|record| record.guid)
                .ok_or_else(|| format!("component handle {id:?} not found in world"))?;
            Ok(ComponentRef::Guid(guid))
        }
        Value::String(s) | Value::Identifier(s) => {
            if let Some(hex) = s.strip_prefix("@uuid:") {
                let uuid = uuid::Uuid::parse_str(hex)
                    .map_err(|e| format!("invalid uuid in '@uuid:{hex}': {e}"))?;
                Ok(ComponentRef::Guid(uuid))
            } else {
                Ok(ComponentRef::Query(s.clone()))
            }
        }
        other => Err(format!(
            "expected component handle or selector string, got {other:?}"
        )),
    }
}

fn resolve_live_component_ref_global(world: &World, src: &ComponentRef) -> Option<ComponentId> {
    match src {
        ComponentRef::Guid(uuid) => world.component_id_by_guid(*uuid),
        ComponentRef::Query(selector) => world
            .world_roots()
            .into_iter()
            .find_map(|root| world.find_component(root, selector)),
    }
}

fn assign_retarget(
    target: &Expression,
    value: Value,
    ctx: &mut EvalContext<'_>,
) -> Result<(), String> {
    match target {
        Expression::Identifier(name) => ctx.object_world.reassign(&name.0, value),
        Expression::BinaryOp {
            op: BinOpKind::Dot, ..
        } => {
            let mut path = Vec::new();
            let root_name = flatten_assign_path(target, &mut path)?;
            let mut root_value = ctx
                .object_world
                .lookup(&root_name)
                .cloned()
                .ok_or_else(|| format!("reassignment: '{}' is not defined", root_name))?;
            assign_into_value(&mut root_value, &path, value, ctx)?;
            ctx.object_world.reassign(&root_name, root_value)
        }
        _ => Err("invalid reassignment target".into()),
    }
}

fn flatten_assign_path(target: &Expression, out: &mut Vec<String>) -> Result<String, String> {
    match target {
        Expression::Identifier(name) => Ok(name.0.clone()),
        Expression::BinaryOp {
            op: BinOpKind::Dot,
            lhs,
            rhs,
        } => {
            let root = flatten_assign_path(lhs, out)?;
            let Expression::Identifier(field) = rhs.as_ref() else {
                return Err("invalid reassignment target".into());
            };
            out.push(field.0.clone());
            Ok(root)
        }
        _ => Err("invalid reassignment target".into()),
    }
}

fn assign_into_value(
    current: &mut Value,
    path: &[String],
    value: Value,
    ctx: &mut EvalContext<'_>,
) -> Result<(), String> {
    let Some((field, rest)) = path.split_first() else {
        *current = value;
        return Ok(());
    };

    match current {
        Value::Map(map) => {
            if rest.is_empty() {
                map.insert(field.clone(), value);
                return Ok(());
            }
            let next = map
                .get_mut(field)
                .ok_or_else(|| format!("field assignment: '{}' not found", field))?;
            assign_into_value(next, rest, value, ctx)
        }
        Value::Object(id) => {
            if rest.is_empty() {
                let Some(()) = id.with_map_mut(|map| {
                    map.insert(field.clone(), value);
                }) else {
                    return Err("field assignment: invalid object".into());
                };
                return Ok(());
            }

            let Some(mut next) = id.with_map(|map| map.get(field).cloned()).flatten() else {
                return Err(format!("field assignment: '{}' not found", field));
            };
            assign_into_value(&mut next, rest, value, ctx)?;
            let Some(()) = id.with_map_mut(|map| {
                map.insert(field.clone(), next);
            }) else {
                return Err("field assignment: invalid object".into());
            };
            Ok(())
        }
        other => Err(format!(
            "field assignment: cannot assign through '{}': {:?}",
            field, other
        )),
    }
}

fn is_builtin_fn(name: &str) -> bool {
    matches!(
        name,
        "print" | "assert" | "range" | "emit" | "on" | "query" | "query_all" | "emit_data"
    )
}

fn eval_if(if_stmt: &IfStatement, ctx: &mut EvalContext<'_>) -> Result<StmtEffect, String> {
    let cond = eval_expr(&if_stmt.condition, ctx)?;
    let branch = if is_truthy(&cond) {
        Some(&if_stmt.then_branch)
    } else {
        None
    };
    if let Some(block) = branch {
        ctx.object_world.push_frame(FrameKind::Block);
        let result = eval_block_stmts(&block.statements, ctx);
        ctx.object_world.pop_frame();
        return result;
    }

    match &if_stmt.else_branch {
        Some(ElseBranch::Block(block)) => {
            ctx.object_world.push_frame(FrameKind::Block);
            let result = eval_block_stmts(&block.statements, ctx);
            ctx.object_world.pop_frame();
            result
        }
        Some(ElseBranch::If(next_if)) => eval_if(next_if, ctx),
        None => Ok(StmtEffect::None),
    }
}

// ---------------------------------------------------------------------------
// Expression evaluation
// ---------------------------------------------------------------------------

/// Evaluate a `ComponentExpression` AST node into a `MaterializedCE`.
///
/// All constructor args are evaluated against the current env. The body block
/// is evaluated as a full MMS block statement in a CE builder context:
/// - CE emissions → captured as children
/// - Calls to names not in env → captured as builder calls
/// - `Value::String` expression statements → captured as positionals
/// - Named assignments (`name = expr`) → read from env after block if pre-injected
fn eval_ce(ce: &ComponentExpression, ctx: &mut EvalContext<'_>) -> Result<Value, String> {
    let component_property_assignment_only =
        component_expr_uses_property_assignment_only(&ce.component_type.0);
    let is_keyframe = matches!(ce.component_type.0.as_str(), "KF" | "Keyframe");
    // Evaluate all constructor calls.
    let mut ctor_method: Option<String> = None;
    let mut ctor_args: Vec<Value> = vec![];
    let mut extra_ctor_calls: Vec<(String, Vec<Value>)> = vec![];
    for (i, ctor) in ce.constructors.iter().enumerate() {
        let args: Vec<Value> = ctor
            .args
            .iter()
            .map(|a| eval_expr(a, ctx))
            .collect::<Result<_, _>>()?;
        if i == 0 {
            ctor_method = Some(ctor.method.0.clone());
            ctor_args = args;
        } else {
            extra_ctor_calls.push((ctor.method.0.clone(), args));
        }
    }

    if is_keyframe {
        let mce = MaterializedCE {
            component_type: ce.component_type.0.clone(),
            component_property_assignment_only,
            ctor_method,
            ctor_args,
            calls: extra_ctor_calls,
            named: vec![],
            positionals: vec![],
            deferred_block: Some(RuntimeClosure {
                body: ce.body.clone(),
                captured_env: Arc::new(ctx.object_world.snapshot_visible()),
                heap: ctx.object_world.heap().clone(),
                analysis: Some(BlockEffectAnalyzer::analyze_keyframe_block(&ce.body)),
            }),
            children: vec![],
        };
        return Ok(Value::ComponentExpr(Box::new(mce)));
    }

    // Evaluate the body block with a CE builder context.
    let mut builder = CeBuilder {
        component_property_assignment_only,
        calls: extra_ctor_calls,
        named: vec![],
        positionals: vec![],
        children: vec![],
    };

    // Evaluate the body block, routing CE emissions and builder calls to `builder`.
    // CE bodies are plain Block frames — fully transparent for read & write.
    ctx.object_world.push_frame(FrameKind::Block);
    let body_result = {
        let mut body_ctx = EvalContext {
            emits: ctx.emits,
            source_path: ctx.source_path,
            channels: ctx.channels.as_mut().map(|c| &mut **c),
            ce_builder: Some(&mut builder),
            object_world: ctx.object_world,
            host_world: ctx.host_world,
            exec_scope: ctx.exec_scope,
            runtime_closure_mode: ctx.runtime_closure_mode,
        };
        eval_block_stmts(&ce.body.statements, &mut body_ctx)
    };
    ctx.object_world.pop_frame();
    body_result?;

    let mce = MaterializedCE {
        component_type: ce.component_type.0.clone(),
        component_property_assignment_only,
        ctor_method,
        ctor_args,
        calls: builder.calls,
        named: builder.named,
        positionals: builder.positionals,
        deferred_block: None,
        children: builder.children,
    };
    Ok(Value::ComponentExpr(Box::new(mce)))
}

fn eval_expr(expr: &Expression, ctx: &mut EvalContext<'_>) -> Result<Value, String> {
    match expr {
        Expression::Null => Ok(Value::Null),
        Expression::Bool(b) => Ok(Value::Bool(*b)),
        Expression::Number(n) => Ok(Value::Number(*n)),
        Expression::Dimension(n, unit) => Ok(Value::Dimension {
            value: *n,
            unit: *unit,
        }),
        Expression::String(s) => Ok(Value::String(s.clone())),
        Expression::Array(items) => {
            let vals = items
                .iter()
                .map(|e| eval_expr(e, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Array(vals))
        }
        Expression::Table(fields) => {
            let mut map = HashMap::with_capacity(fields.len());
            for field in fields {
                map.insert(field.name.0.clone(), eval_expr(&field.value, ctx)?);
            }
            Ok(Value::Object(ctx.object_world.alloc_object(Object::Map(map))))
        }
        Expression::Index { base, index } => {
            let base = eval_expr(base, ctx)?;
            let index = eval_expr(index, ctx)?;
            let Value::Array(items) = base else {
                return Err(format!("index: expected array, got {:?}", base));
            };
            let Value::Number(n) = index else {
                return Err(format!("index: expected numeric index, got {:?}", index));
            };
            if n.fract() != 0.0 || n < 0.0 {
                return Err(format!("index: expected non-negative integer, got {n}"));
            }
            items
                .get(n as usize)
                .cloned()
                .ok_or_else(|| format!("index: {n} out of bounds for array of {}", items.len()))
        }
        Expression::Identifier(id) => {
            // Look up in scope chain; fall back to bare identifier value (for enum-like flags).
            match ctx.object_world.lookup(&id.0) {
                Some(val) => Ok(val.clone()),
                None => match id.0.as_str() {
                    "MusicNote" => Ok(Value::BuiltinTable(BuiltinTableKind::MusicNote)),
                    _ => Ok(Value::Identifier(id.0.clone())),
                },
            }
        }
        Expression::Component(ce) => eval_ce(ce, ctx),
        Expression::Function { params, body } => {
            let captured_env = ctx.object_world.snapshot_visible();
            Ok(Value::Function {
                params: params.iter().map(|p| p.0.clone()).collect(),
                body: body.clone(),
                captured_env: Arc::new(captured_env),
                heap: ctx.object_world.heap().clone(),
            })
        }
        Expression::Call(call) => eval_call(call, ctx),
        Expression::BinaryOp { op, lhs, rhs } => eval_binop(op, lhs, rhs, ctx),
        Expression::UnaryOp { op, operand } => eval_unaryop(op, operand, ctx),
    }
}

fn eval_call(call: &CallExpression, ctx: &mut EvalContext<'_>) -> Result<Value, String> {
    // Method call: `obj.method(args)` — callee is BinaryOp(Dot, lhs, rhs).
    if let Expression::BinaryOp {
        op: BinOpKind::Dot,
        lhs,
        rhs,
    } = call.callee.as_ref()
    {
        let receiver = eval_expr(lhs, ctx)?;
        let method_name = match rhs.as_ref() {
            Expression::Identifier(id) => id.0.clone(),
            other => {
                return Err(format!(
                    "method call: RHS of '.' must be an identifier, got {:?}",
                    other
                ));
            }
        };
        let args: Vec<Value> = call
            .args
            .iter()
            .map(|a| eval_expr(a, ctx))
            .collect::<Result<_, _>>()?;
        return eval_method_call(receiver, &method_name, args, ctx);
    }

    let callee_name = match call.callee.as_ref() {
        Expression::Identifier(id) => &id.0,
        other => return Err(format!("cannot call {:?} as a function", other)),
    };

    // Built-in: print(value)
    if callee_name == "print" {
        let arg = call
            .args
            .first()
            .map(|a| eval_expr(a, ctx))
            .transpose()?
            .unwrap_or(Value::Null);
        println!("[mms] {}", value_display(&arg));
        return Ok(Value::Null);
    }

    // Built-in: assert(cond, msg)
    if callee_name == "assert" {
        let cond = call
            .args
            .first()
            .map(|a| eval_expr(a, ctx))
            .transpose()?
            .unwrap_or(Value::Null);
        if !is_truthy(&cond) {
            let msg = call
                .args
                .get(1)
                .map(|a| eval_expr(a, ctx))
                .transpose()?
                .unwrap_or(Value::String("assertion failed".into()));
            return Err(format!("assert: {}", value_display(&msg)));
        }
        return Ok(Value::Null);
    }

    // Built-in: range(n) or range(start, end)
    if callee_name == "range" {
        let args: Vec<Value> = call
            .args
            .iter()
            .map(|a| eval_expr(a, ctx))
            .collect::<Result<_, _>>()?;
        let (start, end) = match args.as_slice() {
            [Value::Number(n)] => (0.0_f64, *n),
            [Value::Number(s), Value::Number(e)] => (*s, *e),
            _ => return Err("range() takes 1 or 2 numeric arguments".into()),
        };
        let count = ((end - start).max(0.0).floor()) as usize;
        let arr = (0..count)
            .map(|i| Value::Number(start + i as f64))
            .collect();
        return Ok(Value::Array(arr));
    }

    // Built-in: query(selector) / query(selector, handler)
    //           query_all(selector) / query_all(selector, handler)
    if callee_name == "query" || callee_name == "query_all" {
        let multiple = callee_name == "query_all";
        let args: Vec<Value> = call
            .args
            .iter()
            .map(|a| eval_expr(a, ctx))
            .collect::<Result<_, _>>()?;
        let selector = match args.first() {
            Some(Value::String(s)) => s.clone(),
            other => {
                return Err(format!(
                    "{}(): arg 0 must be a string selector, got {:?}",
                    callee_name, other
                ));
            }
        };
        let handler = match args.get(1) {
            Some(f @ Value::Function { .. }) => Some(f.clone()),
            None => None,
            other => {
                return Err(format!(
                    "{}(): arg 1 (optional) must be a function, got {:?}",
                    callee_name, other
                ));
            }
        };
        let result = run_world_query(selector, None, multiple, ctx)?;
        return dispatch_query_result(result, handler, multiple, ctx);
    }

    // Built-in:
    //   on(component_object, "SignalKind", fn(event) { ... })
    //   on(component_object, "SignalKind", "handler_name", fn(event) { ... })
    if callee_name == "on" {
        let args: Vec<Value> = call
            .args
            .iter()
            .map(|a| eval_expr(a, ctx))
            .collect::<Result<_, _>>()?;
        let scope = match args.get(0) {
            Some(Value::ComponentObject { id, .. }) => *id,
            other => {
                return Err(format!(
                    "on(): arg 0 must be a ComponentObject, got {:?}",
                    other
                ));
            }
        };
        let signal_kind = match args.get(1) {
            Some(Value::String(s)) => parse_signal_kind(s)?,
            other => {
                return Err(format!(
                    "on(): arg 1 must be a signal kind string, got {:?}",
                    other
                ));
            }
        };
        let (name, handler_idx) = match args.get(2) {
            Some(Value::String(name)) => (Some(name.clone()), 3),
            _ => (None, 2),
        };
        let handler = match args.get(handler_idx) {
            Some(f @ Value::Function { .. }) => f.clone(),
            other => {
                return Err(format!(
                    "on(): arg {} must be a function, got {:?}",
                    handler_idx, other
                ));
            }
        };
        if let Some(ch) = ctx.channels.as_mut() {
            ch.call(HostCallKind::RegisterHandler {
                scope,
                signal_kind,
                name,
                handler,
            });
        }
        return Ok(Value::Null);
    }

    // Built-in:
    //   emit_data(scope_component, "name")
    //   emit_data(scope_component, "name", payload_component)
    if callee_name == "emit_data" {
        let args: Vec<Value> = call
            .args
            .iter()
            .map(|a| eval_expr(a, ctx))
            .collect::<Result<_, _>>()?;
        let scope = match args.first() {
            Some(Value::ComponentObject { id, .. }) => *id,
            other => {
                return Err(format!(
                    "emit_data(): arg 0 must be a ComponentObject, got {:?}",
                    other
                ));
            }
        };
        let name = match args.get(1) {
            Some(Value::String(name)) => name.clone(),
            other => {
                return Err(format!(
                    "emit_data(): arg 1 must be a string, got {:?}",
                    other
                ));
            }
        };
        let payload = match args.get(2) {
            Some(Value::ComponentObject { id, .. }) => Some(*id),
            Some(Value::Null) | None => None,
            other => {
                return Err(format!(
                    "emit_data(): arg 2 must be a ComponentObject or null, got {:?}",
                    other
                ));
            }
        };
        let emitted = LIVE_SIGNAL_EMITTER.with(|slot| {
            let Some(host_emit) = *slot.borrow() else {
                return false;
            };
            unsafe {
                (&mut *host_emit).push_event(
                    scope,
                    crate::engine::ecs::EventSignal::DataEvent { name, payload },
                );
            }
            true
        });
        if !emitted {
            return Err("emit_data(): no live signal emitter".into());
        }
        return Ok(Value::Null);
    }

    let callee_val = match ctx.object_world.lookup(callee_name) {
        Some(v) => v.clone(),
        None => return Err(format!("undefined: '{}'", callee_name)),
    };

    match callee_val {
        Value::Function {
            params,
            body,
            captured_env,
            ..
        } => {
            let args: Vec<Value> = call
                .args
                .iter()
                .map(|a| eval_expr(a, ctx))
                .collect::<Result<_, _>>()?;

            // Push a Function frame seeded with the closure's captured env;
            // bind arg values into the same frame so they shadow captured names.
            ctx.object_world.push_function_frame(captured_env);
            for (index, param) in params.iter().enumerate() {
                let arg = args.get(index).cloned().unwrap_or(Value::Null);
                ctx.object_world.bind(param.clone(), arg);
            }
            let result = {
                let mut func_ctx = EvalContext {
                    emits: ctx.emits,
                    source_path: None,
                    channels: ctx.channels.as_mut().map(|c| &mut **c),
                    ce_builder: None,
                    object_world: ctx.object_world,
                    host_world: ctx.host_world,
                    exec_scope: ctx.exec_scope,
                    runtime_closure_mode: ctx.runtime_closure_mode,
                };
                eval_block_stmts(&body.statements, &mut func_ctx)
            };
            ctx.object_world.pop_frame();
            match result? {
                StmtEffect::Return(val) => Ok(val),
                StmtEffect::None => Ok(Value::Null),
                StmtEffect::Break | StmtEffect::Continue => {
                    Err("break/continue cannot escape a function body".into())
                }
                StmtEffect::Exported(_) => Ok(Value::Null),
            }
        }
        other => Err(format!("cannot call {:?} as a function", other)),
    }
}

/// Issue a Query HostCall and decode the reply into a list of
/// `Value::ComponentObject`s. For `multiple = false` the list contains 0 or 1.
/// Returns Err if the host had no channels available (fire-and-forget mode).
fn run_world_query(
    selector: String,
    scope: Option<ComponentId>,
    multiple: bool,
    ctx: &mut EvalContext<'_>,
) -> Result<Vec<Value>, String> {
    if let Some(ch) = ctx.channels.as_mut() {
        let reply = ch.call(HostCallKind::Query {
            selector,
            scope,
            multiple,
        });
        let out = match reply {
            None => Vec::new(),
            Some(HostValue::Component { id, component_type }) => {
                vec![Value::ComponentObject { id, component_type }]
            }
            Some(HostValue::ComponentList(list)) => list
                .into_iter()
                .map(|(id, component_type)| Value::ComponentObject { id, component_type })
                .collect(),
            Some(other) => {
                return Err(format!("query: unexpected HostValue reply: {:?}", other));
            }
        };
        return Ok(out);
    }

    let Some(world) = ctx.host_world else {
        // No host (fire-and-forget runner) — return empty.
        return Ok(Vec::new());
    };
    let world = unsafe { &mut *world };

    let roots: Vec<ComponentId> = match scope {
        Some(id) => vec![id],
        None => world
            .all_components()
            .filter(|&id| world.parent_of(id).is_none())
            .collect(),
    };

    let mut all_ids: Vec<ComponentId> = Vec::new();
    for root in roots {
        if multiple {
            all_ids.extend(world.find_all_components(root, &selector));
        } else if let Some(found) = world.find_component(root, &selector) {
            all_ids.push(found);
            break;
        }
    }

    let out = if multiple {
        all_ids
            .into_iter()
            .filter_map(|id| {
                world
                    .component_name(id)
                    .map(|component_type| Value::ComponentObject {
                        id,
                        component_type: component_type.to_string(),
                    })
            })
            .collect()
    } else {
        match all_ids.into_iter().next() {
            Some(id) => match world.component_name(id) {
                Some(component_type) => vec![Value::ComponentObject {
                    id,
                    component_type: component_type.to_string(),
                }],
                None => Vec::new(),
            },
            None => Vec::new(),
        }
    };
    Ok(out)
}

/// Shape the query reply: scalar/null for `query`, Array for `query_all`,
/// or invoke the handler when one was supplied.
///
/// - no callback, multiple=false → first match (`Value::ComponentObject`) or `Null`
/// - no callback, multiple=true  → `Value::Array` of matches (possibly empty)
/// - callback,    multiple=false → handler called once with first match (or `Null`); returns `Null`
/// - callback,    multiple=true  → handler called once per match; returns `Null`
fn dispatch_query_result(
    mut matches: Vec<Value>,
    handler: Option<Value>,
    multiple: bool,
    ctx: &mut EvalContext<'_>,
) -> Result<Value, String> {
    if let Some(handler) = handler {
        if multiple {
            for m in matches {
                eval_user_fn(&handler, vec![m], ctx)?;
            }
        } else {
            let arg = matches.into_iter().next().unwrap_or(Value::Null);
            eval_user_fn(&handler, vec![arg], ctx)?;
        }
        return Ok(Value::Null);
    }
    if multiple {
        Ok(Value::Array(matches))
    } else {
        Ok(matches.drain(..).next().unwrap_or(Value::Null))
    }
}

/// Call an MMS `Value::Function` with the given args using the current eval
/// context. Returns whatever the function returns (or `Null`).
fn eval_user_fn(
    handler: &Value,
    args: Vec<Value>,
    ctx: &mut EvalContext<'_>,
) -> Result<Value, String> {
    let Value::Function {
        params,
        body,
        captured_env,
        ..
    } = handler
    else {
        return Err(format!("expected function, got {:?}", handler));
    };
    ctx.object_world.push_function_frame(captured_env.clone());
    for (index, param) in params.iter().enumerate() {
        let arg = args.get(index).cloned().unwrap_or(Value::Null);
        ctx.object_world.bind(param.clone(), arg);
    }
    let result = {
        let mut func_ctx = EvalContext {
            emits: ctx.emits,
            source_path: None,
            channels: ctx.channels.as_mut().map(|c| &mut **c),
            ce_builder: None,
            object_world: ctx.object_world,
            host_world: ctx.host_world,
            exec_scope: ctx.exec_scope,
            runtime_closure_mode: ctx.runtime_closure_mode,
        };
        eval_block_stmts(&body.statements, &mut func_ctx)
    };
    ctx.object_world.pop_frame();
    match result? {
        StmtEffect::Return(val) => Ok(val),
        _ => Ok(Value::Null),
    }
}

/// Dispatch a method call on a `Value::ComponentObject`.
///
/// Produces intents (emitted via `ctx.emits`) or returns a value.
/// Currently supports animation methods: `play`, `pause`, `loop_anim`.
fn eval_method_call(
    receiver: Value,
    method: &str,
    args: Vec<Value>,
    ctx: &mut EvalContext<'_>,
) -> Result<Value, String> {
    match receiver {
        Value::BuiltinTable(BuiltinTableKind::MusicNote) => {
            let pitch_ctor = match method {
                "a" => MusicNote::a,
                "b" => MusicNote::b,
                "c" => MusicNote::c,
                "d" => MusicNote::d,
                "e" => MusicNote::e,
                "f" => MusicNote::f,
                "g" => MusicNote::g,
                _ => return Err(format!("MusicNote: unknown note '{}'", method)),
            };

            let octave = match args.first() {
                Some(Value::Number(n)) if n.is_finite() && *n >= 0.0 && n.fract() == 0.0 => {
                    *n as u16
                }
                other => {
                    return Err(format!(
                        "MusicNote.{}(): arg 0 must be a non-negative integer octave, got {:?}",
                        method, other
                    ));
                }
            };
            let duration_beats = match args.get(1) {
                Some(Value::Number(n)) => *n as f32,
                other => {
                    return Err(format!(
                        "MusicNote.{}(): arg 1 must be a numeric duration, got {:?}",
                        method, other
                    ));
                }
            };
            let mut note = pitch_ctor(octave, duration_beats);
            if let Some(Value::Number(velocity)) = args.get(3) {
                note = note.with_velocity(*velocity as f32);
            }

            let Some(world) = ctx.host_world else {
                return Err(format!("MusicNote.{}(): no host world", method));
            };
            let world = unsafe { &mut *world };
            let target_source = match args.get(2) {
                Some(value) => Some(value_to_component_ref_live(world, value)?),
                None => None,
            };
            let target = target_source
                .as_ref()
                .and_then(|src| resolve_live_component_ref_global(world, src))
                .ok_or_else(|| {
                    format!("MusicNote.{}(): arg 2 must resolve to an audio target", method)
                })?;

            push_eval_intent(ctx, IntentValue::AudioSchedulePlay {
                component_ids: vec![target],
                beat_offset: 0.0,
                beat_context: None,
                note: Some(note),
                gain: None,
                rate: None,
                duration: None,
            });
            Ok(Value::Null)
        }
        Value::ComponentObject {
            id,
            ref component_type,
        } => {
            // Subtree query — `comp.query("sel")` / `comp.query_all("sel")`.
            // Also accepts an optional handler arg (same shape as the free builtins).
            if method == "query" || method == "query_all" {
                let multiple = method == "query_all";
                let selector = match args.first() {
                    Some(Value::String(s)) => s.clone(),
                    other => {
                        return Err(format!(
                            "{}(): arg 0 must be a string selector, got {:?}",
                            method, other
                        ));
                    }
                };
                let handler = match args.get(1) {
                    Some(f @ Value::Function { .. }) => Some(f.clone()),
                    None => None,
                    other => {
                        return Err(format!(
                            "{}(): arg 1 (optional) must be a function, got {:?}",
                            method, other
                        ));
                    }
                };
                let result = run_world_query(selector, Some(id), multiple, ctx)?;
                return dispatch_query_result(result, handler, multiple, ctx);
            }

            if supports_component_method(component_type, method) {
                if let Some(world) = ctx.host_world {
                    let world = unsafe { &mut *world };
                    return invoke_component_method(world, id, component_type, method, &args, |intent| {
                        push_eval_intent(ctx, intent)
                    });
                }
                if let Some(ch) = ctx.channels.as_mut() {
                    match ch.call(HostCallKind::InvokeComponentMethod {
                        id,
                        component_type: component_type.clone(),
                        method: method.to_string(),
                        args: args.clone(),
                    }) {
                        Some(HostValue::Null) | None => return Ok(Value::Null),
                        Some(HostValue::Component { id, component_type }) => {
                            return Ok(Value::ComponentObject { id, component_type });
                        }
                        Some(HostValue::ComponentId(component_id)) => {
                            return Ok(Value::ComponentObject {
                                id: component_id,
                                component_type: component_type.clone(),
                            });
                        }
                        Some(other) => {
                            return Err(format!(
                                "InvokeComponentMethod returned unexpected value {:?}",
                                other
                            ));
                        }
                    }
                }
            }

            // Animation playback.
            let anim_state = match method {
                "play"
                    if matches!(
                        component_type.as_str(),
                        "A" | "Animation" | "AnimationComponent" | "animation"
                    ) =>
                {
                    Some(AnimationState::Playing)
                }
                "loop_anim"
                    if matches!(
                        component_type.as_str(),
                        "A" | "Animation" | "AnimationComponent" | "animation"
                    ) =>
                {
                    Some(AnimationState::Looping)
                }
                "pause"
                    if matches!(
                        component_type.as_str(),
                        "A" | "Animation" | "AnimationComponent" | "animation"
                    ) =>
                {
                    Some(AnimationState::Paused)
                }
                _ => None,
            };
            if let Some(state) = anim_state {
                push_eval_intent(ctx, IntentValue::SetAnimationState {
                    component_ids: vec![id],
                    state,
                });
                return Ok(Value::Null);
            }

            // Layout getter: layout.available_width() → current width as Number.
            if matches!(
                component_type.as_str(),
                "layout" | "LayoutRoot" | "LayoutComponent"
            ) && method == "available_width"
            {
                use crate::engine::ecs::system::layout::measure::layout_root_available_bounds;
                let Some(world) = ctx.host_world else {
                    return Err("available_width(): no host world".into());
                };
                let world = unsafe { &mut *world };
                let (w, _, _) = layout_root_available_bounds(world, id);
                if world
                    .get_component_by_id_as::<crate::engine::ecs::component::LayoutComponent>(id)
                    .is_none()
                {
                    return Err("available_width(): not a LayoutComponent".into());
                }
                let w = w as f64;
                return Ok(Value::Number(w));
            }

            if matches!(
                component_type.as_str(),
                "layout" | "LayoutRoot" | "LayoutComponent"
            ) && method == "available_height"
            {
                use crate::engine::ecs::system::layout::measure::layout_root_available_bounds;
                let Some(world) = ctx.host_world else {
                    return Err("available_height(): no host world".into());
                };
                let world = unsafe { &mut *world };
                if world
                    .get_component_by_id_as::<crate::engine::ecs::component::LayoutComponent>(id)
                    .is_none()
                {
                    return Err("available_height(): not a LayoutComponent".into());
                }
                let (_, h, _) = layout_root_available_bounds(world, id);
                let h = h
                    .map(|value| value as f64)
                    .ok_or_else(|| "available_height(): height is unset".to_string())?;
                return Ok(Value::Number(h));
            }

            // Layout mutation: layout.set_available_width(N).
            if matches!(
                component_type.as_str(),
                "layout" | "LayoutRoot" | "LayoutComponent"
            ) && method == "set_available_width"
            {
                use crate::engine::ecs::component::style::SizeDimension;
                use crate::meow_meow::token::Unit;

                let width = match args.first() {
                    Some(Value::Number(n)) => SizeDimension::GlyphUnits(*n as f32),
                    Some(Value::Dimension {
                        value,
                        unit: Unit::GlyphUnits,
                    }) => SizeDimension::GlyphUnits(*value as f32),
                    Some(Value::Dimension {
                        value,
                        unit: Unit::WorldUnits,
                    }) => SizeDimension::WorldUnits(*value as f32),
                    Some(Value::Dimension { unit, .. }) => {
                        return Err(format!(
                            "set_available_width: expected gu or wu dimension, got {:?}",
                            unit
                        ));
                    }
                    Some(other) => {
                        return Err(format!(
                            "set_available_width: expected number or dimension argument, got {:?}",
                            other
                        ));
                    }
                    None => {
                        return Err(
                            "set_available_width: missing number or dimension argument".into()
                        );
                    }
                };
                push_eval_intent(ctx, IntentValue::SetLayoutAvailableWidth {
                    component_ids: vec![id],
                    width,
                });
                return Ok(Value::Null);
            }

            if matches!(
                component_type.as_str(),
                "layout" | "LayoutRoot" | "LayoutComponent"
            ) && method == "set_available_height"
            {
                use crate::engine::ecs::component::style::SizeDimension;
                use crate::meow_meow::token::Unit;

                let height = match args.first() {
                    Some(Value::Number(n)) => SizeDimension::GlyphUnits(*n as f32),
                    Some(Value::Dimension {
                        value,
                        unit: Unit::GlyphUnits,
                    }) => SizeDimension::GlyphUnits(*value as f32),
                    Some(Value::Dimension {
                        value,
                        unit: Unit::WorldUnits,
                    }) => SizeDimension::WorldUnits(*value as f32),
                    Some(Value::Dimension { unit, .. }) => {
                        return Err(format!(
                            "set_available_height: expected gu or wu dimension, got {:?}",
                            unit
                        ));
                    }
                    Some(other) => {
                        return Err(format!(
                            "set_available_height: expected number or dimension argument, got {:?}",
                            other
                        ));
                    }
                    None => {
                        return Err(
                            "set_available_height: missing number or dimension argument".into()
                        );
                    }
                };
                push_eval_intent(ctx, IntentValue::SetLayoutAvailableHeight {
                    component_ids: vec![id],
                    height,
                });
                return Ok(Value::Null);
            }

            // Layout viz toggle: layout.set_inspect(bool) / .enable_inspect() / .disable_inspect().
            if matches!(
                component_type.as_str(),
                "layout" | "LayoutRoot" | "LayoutComponent"
            ) && matches!(method, "set_inspect" | "enable_inspect" | "disable_inspect")
            {
                let enabled = match (method, args.first()) {
                    ("enable_inspect", _) => true,
                    ("disable_inspect", _) => false,
                    ("set_inspect", Some(Value::Bool(b))) => *b,
                    ("set_inspect", Some(other)) => {
                        return Err(format!(
                            "set_inspect: expected bool argument, got {:?}",
                            other
                        ));
                    }
                    ("set_inspect", None) => {
                        return Err("set_inspect: missing bool argument".into());
                    }
                    _ => unreachable!(),
                };
                push_eval_intent(ctx, IntentValue::SetLayoutInspect {
                    component_ids: vec![id],
                    enabled,
                });
                return Ok(Value::Null);
            }

            // Text mutation: text.set_text("...").
            if matches!(
                component_type.as_str(),
                "Text" | "TXT" | "TextComponent" | "text"
            ) && method == "set_text"
            {
                let text = match args.first() {
                    Some(Value::String(s)) => s.clone(),
                    Some(other) => {
                        return Err(format!(
                            "set_text: expected string argument, got {:?}",
                            other
                        ));
                    }
                    None => return Err("set_text: missing string argument".into()),
                };
                push_eval_intent(ctx, IntentValue::SetText {
                    component_ids: vec![id],
                    text,
                });
                return Ok(Value::Null);
            }

            if matches!(
                component_type.as_str(),
                "T" | "Transform" | "TransformComponent" | "transform"
            ) && method == "set_position"
            {
                let [x, y, z] = match args.as_slice() {
                    [Value::Number(x), Value::Number(y), Value::Number(z)] => {
                        [*x as f32, *y as f32, *z as f32]
                    }
                    other => {
                        return Err(format!(
                            "set_position: expected three numeric arguments, got {:?}",
                            other
                        ));
                    }
                };
                let Some(world) = ctx.host_world else {
                    return Err("set_position(): no host world".into());
                };
                let world = unsafe { &mut *world };
                let t = world
                    .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(
                        id,
                    )
                    .ok_or_else(|| "set_position(): not a TransformComponent".to_string())?;
                let mut next = t.transform;
                next.translation = [x, y, z];
                next.recompute_model();
                push_eval_intent(ctx, IntentValue::UpdateTransform {
                    component_ids: vec![id],
                    translation: next.translation,
                    rotation_quat_xyzw: next.rotation,
                    scale: next.scale,
                });
                return Ok(Value::Null);
            }

            if matches!(
                component_type.as_str(),
                "Camera3D" | "Camera3DComponent" | "camera3d" | "C3D"
            ) && matches!(method, "enabled" | "make_active_camera")
            {
                let Some(world) = ctx.host_world else {
                    return Err(format!("{method}(): no host world"));
                };
                let world = unsafe { &mut *world };
                if method == "enabled" {
                    if args.is_empty() {
                        let enabled = world
                            .get_component_by_id_as::<crate::engine::ecs::component::Camera3DComponent>(id)
                            .ok_or_else(|| "enabled(): not a Camera3DComponent".to_string())?
                            .enabled;
                        return Ok(Value::Bool(enabled));
                    }
                    let enabled = match args.first() {
                        Some(Value::Bool(b)) => *b,
                        Some(other) => {
                            return Err(format!(
                                "enabled: expected bool argument, got {:?}",
                                other
                            ));
                        }
                        None => unreachable!(),
                    };
                    let camera = world
                        .get_component_by_id_as_mut::<crate::engine::ecs::component::Camera3DComponent>(id)
                        .ok_or_else(|| "enabled(): not a Camera3DComponent".to_string())?;
                    camera.enabled = enabled;
                    return Ok(Value::Null);
                }
                push_eval_intent(ctx, IntentValue::MakeActiveCamera {
                    component_ids: vec![id],
                });
                return Ok(Value::Null);
            }

            if matches!(
                component_type.as_str(),
                "CameraXR" | "CameraXRComponent" | "camera_xr" | "CXR"
            ) && matches!(method, "enabled" | "make_active_camera")
            {
                let Some(world) = ctx.host_world else {
                    return Err(format!("{method}(): no host world"));
                };
                let world = unsafe { &mut *world };
                if method == "enabled" {
                    if args.is_empty() {
                        let enabled = world
                            .get_component_by_id_as::<crate::engine::ecs::component::CameraXRComponent>(id)
                            .ok_or_else(|| "enabled(): not a CameraXRComponent".to_string())?
                            .enabled;
                        return Ok(Value::Bool(enabled));
                    }
                    let enabled = match args.first() {
                        Some(Value::Bool(b)) => *b,
                        Some(other) => {
                            return Err(format!(
                                "enabled: expected bool argument, got {:?}",
                                other
                            ));
                        }
                        None => unreachable!(),
                    };
                    let camera = world
                        .get_component_by_id_as_mut::<crate::engine::ecs::component::CameraXRComponent>(id)
                        .ok_or_else(|| "enabled(): not a CameraXRComponent".to_string())?;
                    camera.enabled = enabled;
                    return Ok(Value::Null);
                }
                push_eval_intent(ctx, IntentValue::MakeActiveCamera {
                    component_ids: vec![id],
                });
                return Ok(Value::Null);
            }

            if matches!(
                component_type.as_str(),
                "Text" | "TXT" | "TextComponent" | "text"
            ) && method == "set_font_size"
            {
                let font_size = match args.first() {
                    Some(Value::Number(n)) => *n as f32,
                    Some(other) => {
                        return Err(format!(
                            "set_font_size: expected number argument, got {:?}",
                            other
                        ));
                    }
                    None => return Err("set_font_size: missing number argument".into()),
                };
                let Some(world) = ctx.host_world else {
                    return Err("set_font_size(): no host world".into());
                };
                let world = unsafe { &mut *world };
                let cur_text = world
                    .get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(id)
                    .map(|t| t.text.clone())
                    .ok_or_else(|| "set_font_size(): not a TextComponent".to_string())?;
                if let Some(t) = world
                    .get_component_by_id_as_mut::<crate::engine::ecs::component::TextComponent>(id)
                {
                    t.set_font_size(font_size);
                }
                push_eval_intent(ctx, IntentValue::SetText {
                    component_ids: vec![id],
                    text: cur_text,
                });
                return Ok(Value::Null);
            }

            if matches!(
                component_type.as_str(),
                "ObserverRouter" | "signal_observer_router" | "SignalObserverRouterComponent"
            ) && matches!(method, "blacklist" | "whitelist" | "block" | "allow")
            {
                let Some(world) = ctx.host_world else {
                    return Err(format!("{method}(): no host world"));
                };
                let world = unsafe { &mut *world };
                let router = world
                    .get_component_by_id_as_mut::<crate::engine::ecs::component::SignalObserverRouterComponent>(
                        id,
                    )
                    .ok_or_else(|| format!("{method}(): not a SignalObserverRouterComponent"))?;
                match method {
                    "blacklist" | "whitelist" => {
                        let values = match args.first() {
                            Some(Value::Array(values)) => values,
                            Some(other) => {
                                return Err(format!(
                                    "{method}(): expected array argument, got {:?}",
                                    other
                                ));
                            }
                            None => return Err(format!("{method}(): missing array argument")),
                        };
                        let mut out = Vec::with_capacity(values.len());
                        for value in values {
                            match value {
                                Value::String(s) => out.push(s.clone()),
                                other => {
                                    return Err(format!(
                                        "{method}(): expected string array, got {:?}",
                                        other
                                    ));
                                }
                            }
                        }
                        match method {
                            "blacklist" => router.blacklist = out,
                            "whitelist" => router.whitelist = out,
                            _ => unreachable!(),
                        }
                    }
                    "block" | "allow" => {
                        let name = match args.first() {
                            Some(Value::String(name)) => name.clone(),
                            Some(other) => {
                                return Err(format!(
                                    "{method}(): expected string argument, got {:?}",
                                    other
                                ));
                            }
                            None => return Err(format!("{method}(): missing string argument")),
                        };
                        if method == "block" {
                            if !router.blacklist.iter().any(|item| item == &name) {
                                router.blacklist.push(name);
                            }
                        } else {
                            router.blacklist.retain(|item| item != &name);
                        }
                    }
                    _ => unreachable!(),
                }
                return Ok(Value::Null);
            }

            // AudioClip.instance([start_beat], [stop_beat]) — produce a
            // new clip that shares the receiver's decoded buffer but
            // gets its own playhead. Mirrors `let x = CE` semantics:
            // returns a detached handle, caller attaches by referencing
            // the binding inside a CE body (or manually). See
            // docs/draft/audio-clip-instance-cloning.md §3.
            if matches!(
                component_type.as_str(),
                "AudioClip" | "AudioClipComponent" | "audio_clip"
            ) && method == "instance"
            {
                let start_beat = match args.first() {
                    Some(Value::Number(n)) => Some(*n),
                    Some(Value::Null) | None => None,
                    Some(other) => {
                        return Err(format!(
                            "instance(): start_beat must be a number, got {:?}",
                            other
                        ));
                    }
                };
                let stop_beat = match args.get(1) {
                    Some(Value::Number(n)) => Some(*n),
                    Some(Value::Null) | None => None,
                    Some(other) => {
                        return Err(format!(
                            "instance(): stop_beat must be a number, got {:?}",
                            other
                        ));
                    }
                };

                let Some(ch) = ctx.channels.as_mut() else {
                    return Err("instance(): no host channel".into());
                };
                let new_id = match ch.call(HostCallKind::AudioClipInstance {
                    source: id,
                    start_beat,
                    stop_beat,
                }) {
                    Some(HostValue::ComponentId(cid)) => cid,
                    _ => return Err("instance(): host AudioClipInstance failed".into()),
                };
                return Ok(Value::ComponentObject {
                    id: new_id,
                    component_type: "AudioClip".to_string(),
                });
            }

            Err(format!(
                "no method '{}' on component type '{}'",
                method, component_type
            ))
        }
        other => Err(format!(
            "method call '{}': receiver is not a ComponentObject, got {:?}",
            method, other
        )),
    }
}

fn eval_binop(
    op: &BinOpKind,
    lhs: &Expression,
    rhs: &Expression,
    ctx: &mut EvalContext<'_>,
) -> Result<Value, String> {
    // Short-circuit logical ops.
    match op {
        BinOpKind::And => {
            let l = eval_expr(lhs, ctx)?;
            if !is_truthy(&l) {
                return Ok(Value::Bool(false));
            }
            let r = eval_expr(rhs, ctx)?;
            return Ok(Value::Bool(is_truthy(&r)));
        }
        BinOpKind::Or => {
            let l = eval_expr(lhs, ctx)?;
            if is_truthy(&l) {
                return Ok(Value::Bool(true));
            }
            let r = eval_expr(rhs, ctx)?;
            return Ok(Value::Bool(is_truthy(&r)));
        }
        BinOpKind::Query => {
            // QueryDesugarTransform rewrites all `->` nodes into query()/query_all() calls
            // before eval runs. This arm is only reached if the transform missed one.
            return Err(
                "query operator '->' was not desugared by QueryDesugarTransform".to_string(),
            );
        }
        BinOpKind::Pipe => {
            let lhs_val = eval_expr(lhs, ctx)?;
            let rhs_val = eval_expr(rhs, ctx)?;
            match rhs_val {
                Value::Function {
                    params,
                    body,
                    captured_env,
                    ..
                } => {
                    ctx.object_world.push_function_frame(captured_env);
                    if let Some(param) = params.first() {
                        ctx.object_world.bind(param.clone(), lhs_val);
                    }
                    let result = {
                        let mut func_ctx = EvalContext {
                            emits: ctx.emits,
                            source_path: None,
                            channels: None,
                            ce_builder: None,
                            object_world: ctx.object_world,
                            host_world: None,
                            exec_scope: None,
                            runtime_closure_mode: RuntimeClosureExecMode::Full,
                        };
                        eval_block_stmts(&body.statements, &mut func_ctx)
                    };
                    ctx.object_world.pop_frame();
                    match result? {
                        StmtEffect::Return(val) => return Ok(val),
                        _ => return Ok(Value::Null),
                    }
                }
                other => return Err(format!("pipe: RHS must be a function, got {:?}", other)),
            }
        }
        BinOpKind::Dot => {
            let lhs_val = eval_expr(lhs, ctx)?;
            let Expression::Identifier(field) = rhs else {
                return Err(format!(
                    "field access: RHS of '.' must be an identifier, got {:?}",
                    rhs
                ));
            };
            return match lhs_val {
                Value::BuiltinTable(BuiltinTableKind::MusicNote) => match field.0.as_str() {
                    "a" | "b" | "c" | "d" | "e" | "f" | "g" => {
                        Ok(Value::Identifier(format!("MusicNote.{}", field.0)))
                    }
                    _ => Err(format!("field access: '{}' not found", field.0)),
                },
                Value::Map(fields) => fields
                    .get(&field.0)
                    .cloned()
                    .ok_or_else(|| format!("field access: '{}' not found", field.0)),
                Value::Object(id) => match id.with_map(|fields| fields.get(&field.0).cloned()) {
                    Some(Some(value)) => Ok(value),
                    Some(None) => Err(format!("field access: '{}' not found", field.0)),
                    None => Err("field access: invalid object".into()),
                },
                other => Err(format!(
                    "field access: cannot read '{}' from {:?}",
                    field.0, other
                )),
            };
        }
        _ => {}
    }

    let l = eval_expr(lhs, ctx)?;
    let r = eval_expr(rhs, ctx)?;

    match op {
        BinOpKind::Add => match (l, r) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
            (Value::Dimension { value: a, unit: au }, Value::Dimension { value: b, unit: bu }) => {
                dimension_add(a, au, b, bu)
            }
            (Value::String(a), Value::String(b)) => Ok(Value::String(a + &b)),
            (Value::String(a), r) => Ok(Value::String(a + &value_display(&r))),
            (l, Value::String(b)) => Ok(Value::String(value_display(&l) + &b)),
            (l, r) => Err(format!("type error: cannot add {:?} and {:?}", l, r)),
        },
        BinOpKind::Sub => match (l, r) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a - b)),
            (Value::Dimension { value: a, unit: au }, Value::Dimension { value: b, unit: bu }) => {
                dimension_sub(a, au, b, bu)
            }
            (l, r) => Err(format!("type error: cannot subtract {:?} from {:?}", r, l)),
        },
        BinOpKind::Mul => match (l, r) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a * b)),
            (Value::Dimension { value, unit }, Value::Number(n))
            | (Value::Number(n), Value::Dimension { value, unit }) => {
                dimension_scale(value, unit, n)
            }
            (l, r) => Err(format!("type error: cannot multiply {:?} and {:?}", l, r)),
        },
        BinOpKind::Div => match (l, r) {
            (Value::Number(a), Value::Number(b)) => {
                if b == 0.0 {
                    return Err("division by zero".to_string());
                }
                Ok(Value::Number(a / b))
            }
            (Value::Dimension { value, unit }, Value::Number(n)) => {
                if n == 0.0 {
                    return Err("division by zero".to_string());
                }
                dimension_scale(value, unit, 1.0 / n)
            }
            (l, r) => Err(format!("type error: cannot divide {:?} by {:?}", l, r)),
        },
        BinOpKind::Rem => match (l, r) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a % b)),
            (l, r) => Err(format!("type error: cannot rem {:?} by {:?}", l, r)),
        },
        BinOpKind::Eq => Ok(Value::Bool(values_equal(&l, &r))),
        BinOpKind::NotEq => Ok(Value::Bool(!values_equal(&l, &r))),
        BinOpKind::Lt => num_cmp(l, r, |a, b| a < b),
        BinOpKind::Gt => num_cmp(l, r, |a, b| a > b),
        BinOpKind::LtEq => num_cmp(l, r, |a, b| a <= b),
        BinOpKind::GtEq => num_cmp(l, r, |a, b| a >= b),
        BinOpKind::And | BinOpKind::Or | BinOpKind::Pipe | BinOpKind::Query => {
            unreachable!("handled above")
        }
        BinOpKind::Dot => unreachable!("handled above"),
    }
}

fn eval_unaryop(
    op: &UnaryOpKind,
    operand: &Expression,
    ctx: &mut EvalContext<'_>,
) -> Result<Value, String> {
    let val = eval_expr(operand, ctx)?;
    match op {
        UnaryOpKind::Neg => match val {
            Value::Number(n) => Ok(Value::Number(-n)),
            Value::Dimension { value, unit } => Ok(Value::Dimension {
                value: -value,
                unit,
            }),
            v => Err(format!("type error: cannot negate {:?}", v)),
        },
        UnaryOpKind::Not => Ok(Value::Bool(!is_truthy(&val))),
    }
}

fn dimension_add(
    lhs: f64,
    lhs_unit: crate::meow_meow::token::Unit,
    rhs: f64,
    rhs_unit: crate::meow_meow::token::Unit,
) -> Result<Value, String> {
    use crate::meow_meow::token::Unit;
    if lhs_unit != rhs_unit {
        return Err(format!(
            "type error: cannot add dimensions with different units {:?} and {:?}",
            lhs_unit, rhs_unit
        ));
    }
    if lhs_unit == Unit::Percent {
        return Err("type error: percent arithmetic requires a layout boundary".into());
    }
    Ok(Value::Dimension {
        value: lhs + rhs,
        unit: lhs_unit,
    })
}

fn dimension_sub(
    lhs: f64,
    lhs_unit: crate::meow_meow::token::Unit,
    rhs: f64,
    rhs_unit: crate::meow_meow::token::Unit,
) -> Result<Value, String> {
    dimension_add(lhs, lhs_unit, -rhs, rhs_unit)
}

fn dimension_scale(
    value: f64,
    unit: crate::meow_meow::token::Unit,
    scale: f64,
) -> Result<Value, String> {
    use crate::meow_meow::token::Unit;
    if unit == Unit::Percent {
        return Err("type error: percent arithmetic requires a layout boundary".into());
    }
    Ok(Value::Dimension {
        value: value * scale,
        unit,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn value_as_f32(value: &Value) -> Result<f32, String> {
    match value {
        Value::Number(n) => Ok(*n as f32),
        other => Err(format!("expected number, got {other:?}")),
    }
}

fn value_as_f64(value: &Value) -> Result<f64, String> {
    match value {
        Value::Number(n) => Ok(*n),
        other => Err(format!("expected number, got {other:?}")),
    }
}

fn value_as_u16(value: &Value) -> Result<u16, String> {
    match value {
        Value::Number(n) if n.is_finite() && *n >= 0.0 && n.fract() == 0.0 => Ok(*n as u16),
        other => Err(format!("expected non-negative integer, got {other:?}")),
    }
}

fn value_display(val: &Value) -> String {
    match val {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{}", *n as i64)
            } else {
                n.to_string()
            }
        }
        Value::String(s) => s.clone(),
        Value::Array(arr) => format!(
            "[{}]",
            arr.iter().map(value_display).collect::<Vec<_>>().join(", ")
        ),
        Value::Map(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(key, value)| format!("{key}: {}", value_display(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Function { .. } => "<fn>".into(),
        Value::ComponentObject { id, component_type } => format!("<{}:{:?}>", component_type, id),
        Value::ComponentExpr(_) => "<ce>".into(),
        Value::Object(id) => id
            .with_map(|map| {
                format!(
                    "{{{}}}",
                    map.iter()
                        .map(|(key, value)| format!("{key}: {}", value_display(value)))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
            .unwrap_or_else(|| "<object>".into()),
        Value::Identifier(s) => s.clone(),
        Value::BuiltinTable(BuiltinTableKind::MusicNote) => "<builtin MusicNote>".into(),
        Value::Module { .. } => "<module>".into(),
        Value::Dimension { value, unit } => {
            let suffix = match unit {
                crate::meow_meow::token::Unit::Percent => "%",
                crate::meow_meow::token::Unit::GlyphUnits => "gu",
                crate::meow_meow::token::Unit::WorldUnits => "wu",
                crate::meow_meow::token::Unit::Degrees => "deg",
                crate::meow_meow::token::Unit::Radians => "rad",
            };
            format!("{}{}", value, suffix)
        }
    }
}

fn value_as_f32_array<const N: usize>(value: &Value) -> Result<[f32; N], String> {
    match value {
        Value::Array(items) => {
            if items.len() != N {
                return Err(format!("expected array of {N}, got {}", items.len()));
            }
            let mut out = [0.0_f32; N];
            for (index, item) in items.iter().enumerate() {
                match item {
                    Value::Number(n) => out[index] = *n as f32,
                    other => {
                        return Err(format!(
                            "expected numeric array element at {index}, got {:?}",
                            other
                        ));
                    }
                }
            }
            Ok(out)
        }
        other => Err(format!("expected array, got {:?}", other)),
    }
}

fn is_truthy(val: &Value) -> bool {
    match val {
        Value::Bool(b) => *b,
        Value::Null => false,
        _ => true,
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Number(a), Value::Number(b)) => a == b,
        (Value::String(a), Value::String(b)) => a == b,
        _ => false,
    }
}

fn num_cmp(l: Value, r: Value, f: impl Fn(f64, f64) -> bool) -> Result<Value, String> {
    match (l, r) {
        (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(f(a, b))),
        (l, r) => Err(format!("type error: cannot compare {:?} and {:?}", l, r)),
    }
}

fn parse_signal_kind(s: &str) -> Result<SignalKind, String> {
    match s {
        "Click" => Ok(SignalKind::Click),
        "DragStart" => Ok(SignalKind::DragStart),
        "DragMove" => Ok(SignalKind::DragMove),
        "DragEnd" => Ok(SignalKind::DragEnd),
        "RayIntersected" => Ok(SignalKind::RayIntersected),
        "ParentChanged" => Ok(SignalKind::ParentChanged),
        "CollisionStarted" => Ok(SignalKind::CollisionStarted),
        "CollisionEnded" => Ok(SignalKind::CollisionEnded),
        "SelectionChanged" => Ok(SignalKind::SelectionChanged),
        "SelectionAdded" => Ok(SignalKind::SelectionAdded),
        "SelectionRemoved" => Ok(SignalKind::SelectionRemoved),
        "SelectionCleared" => Ok(SignalKind::SelectionCleared),
        "Scrolling" => Ok(SignalKind::Scrolling),
        "DataEvent" => Ok(SignalKind::DataEvent),
        "XrButtonDown" => Ok(SignalKind::XrButtonDown),
        "XrButtonUp" => Ok(SignalKind::XrButtonUp),
        "XrButtonChanged" => Ok(SignalKind::XrButtonChanged),
        "XrAxisChanged" => Ok(SignalKind::XrAxisChanged),
        "TextInputFocusChanged" => Ok(SignalKind::TextInputFocusChanged),
        "TextInputChanged" => Ok(SignalKind::TextInputChanged),
        other => Err(format!("unknown signal kind: '{}'", other)),
    }
}

/// Evaluate an MMS `Value::Function` with the given positional args.
///
/// Runs inline on the calling thread (no evaluator channel round-trips).
/// `channels` is `None`; when `world_host` is present, live world queries run
/// directly against that world. Intents emitted by the body (e.g. from method
/// dispatch like `anim.play()` or `text.set_text(...)`) are forwarded to `emit`
/// when provided.
pub(crate) fn eval_mms_fn(
    fn_val: &Value,
    args: Vec<Value>,
    channels: Option<&mut EvalChannels>,
    world_host: Option<&mut World>,
    mut emit: Option<&mut dyn SignalEmitter>,
) -> Result<Value, String> {
    let Value::Function {
        params,
        body,
        captured_env,
        heap,
    } = fn_val
    else {
        return Err(format!("eval_mms_fn: expected Function, got {:?}", fn_val));
    };
    let mut emits: Vec<IntentValue> = Vec::new();
    let mut world = ObjectWorld::with_heap(heap.clone());
    world.push_function_frame(captured_env.clone());
    for (index, param) in params.iter().enumerate() {
        let arg = args.get(index).cloned().unwrap_or(Value::Null);
        world.bind(param.clone(), arg);
    }
    let mut ctx = EvalContext {
        emits: &mut emits,
        source_path: None,
        channels,
        ce_builder: None,
        object_world: &mut world,
        host_world: world_host.map(|world| world as *mut World),
        exec_scope: None,
        runtime_closure_mode: RuntimeClosureExecMode::Full,
    };
    let live_emit = emit.as_mut().map(|em| unsafe {
        std::mem::transmute::<&mut dyn SignalEmitter, *mut dyn SignalEmitter>(&mut **em)
    });
    let result = with_live_signal_emitter(live_emit, || {
        match eval_block_stmts(&body.statements, &mut ctx)? {
            StmtEffect::Return(val) => Ok::<Value, String>(val),
            _ => Ok::<Value, String>(Value::Null),
        }
    })?;
    if let Some(em) = emit {
        for iv in emits {
            em.push_intent_now(ComponentId::default(), iv);
        }
    }
    Ok(result)
}

pub(crate) fn eval_runtime_closure(
    closure: &RuntimeClosure,
    channels: Option<&mut EvalChannels>,
    world_host: Option<&mut World>,
    mut emit: Option<&mut dyn SignalEmitter>,
    exec_scope: Option<ComponentId>,
    mode: RuntimeClosureExecMode,
) -> Result<(), String> {
    let mut emits: Vec<IntentValue> = Vec::new();
    let mut world = ObjectWorld::with_heap(closure.heap.clone());
    world.push_function_frame(closure.captured_env.clone());
        let mut ctx = EvalContext {
            emits: &mut emits,
            source_path: None,
            channels,
            ce_builder: None,
            object_world: &mut world,
            host_world: world_host.map(|world| world as *mut World),
            exec_scope,
            runtime_closure_mode: mode,
        };
    let live_emit = emit.as_mut().map(|em| unsafe {
        std::mem::transmute::<&mut dyn SignalEmitter, *mut dyn SignalEmitter>(&mut **em)
    });
    with_live_signal_emitter(live_emit, || {
        let _ = eval_block_stmts(&closure.body.statements, &mut ctx)?;
        Ok::<(), String>(())
    })?;
    if let Some(em) = emit {
        for iv in emits {
            em.push_intent_now(ComponentId::default(), iv);
        }
    }
    Ok(())
}

/// Evaluate a source file as a module (sandboxed — emits go to `sequence`, not the engine).
/// Returns `Value::Module { named, sequence }`.
pub(crate) fn eval_module_source(source: &str, source_path: Option<&str>) -> Result<Value, String> {
    let mut stmts = parse_source(source)?;
    EmitLiftTransform::apply(&mut stmts);
    QueryDesugarTransform::apply(&mut stmts);

    let mut emits: Vec<IntentValue> = Vec::new();
    let mut named: HashMap<String, Value> = HashMap::new();
    let mut world = ObjectWorld::new();
        let mut ctx = EvalContext {
            emits: &mut emits,
            source_path,
            channels: None,
            ce_builder: None,
            object_world: &mut world,
            host_world: None,
            exec_scope: None,
            runtime_closure_mode: RuntimeClosureExecMode::Full,
        };

    for stmt in &stmts {
        match eval_stmt(stmt, &mut ctx)? {
            StmtEffect::Exported(name) => {
                // The binding is already in object_world; copy it into the
                // module's named-exports map.
                if let Some(val) = ctx.object_world.lookup(&name).cloned() {
                    named.insert(name, val);
                }
            }
            StmtEffect::None => {}
            StmtEffect::Return(_) | StmtEffect::Break | StmtEffect::Continue => {}
        }
    }

    let sequence: Vec<MaterializedCE> = emits
        .into_iter()
        .filter_map(|iv| match iv {
            IntentValue::SpawnComponentTree { root, .. } => Some(*root),
            _ => None,
        })
        .collect();

    Ok(Value::Module {
        named,
        sequence,
        heap: world.heap().clone(),
    })
}

fn resolve_import_path(path: &str, source_path: Option<&str>) -> String {
    if let Some(src) = source_path {
        if let Some(parent) = std::path::Path::new(src).parent() {
            return parent.join(path).to_string_lossy().into_owned();
        }
    }
    path.to_string()
}

fn parse_source(source: &str) -> Result<Vec<Statement>, String> {
    let tokens = MeowMeowTokenizer::new(source)
        .tokenize()
        .map_err(|e| tokenize_err_to_string(source, e))?;
    MeowMeowParser::new(tokens)
        .parse_program()
        .map_err(|e| parse_err_to_string(source, e))
}

fn parse_only(source: &str) -> Result<String, String> {
    let stmts = parse_source(source)?;
    Ok(format!("{stmts:#?}"))
}

fn byte_offset_to_line_col(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let before = &source[..offset];
    let line = before.bytes().filter(|&b| b == b'\n').count() + 1;
    let col = before.rfind('\n').map(|p| offset - p).unwrap_or(offset + 1);
    (line, col)
}

fn format_source_context(source: &str, line: usize, col: usize) -> String {
    let line_text = source.lines().nth(line.saturating_sub(1)).unwrap_or("");
    let caret_pad = " ".repeat(col.saturating_sub(1));
    format!("\n  {line_text}\n  {caret_pad}^")
}

fn tokenize_err_to_string(source: &str, e: TokenizeError) -> String {
    let (line, col) = byte_offset_to_line_col(source, e.span.start);
    format!(
        "tokenize error at {}:{}: {}{}",
        line,
        col,
        e.message,
        format_source_context(source, line, col),
    )
}

fn parse_err_to_string(source: &str, e: ParseError) -> String {
    let (line, col) = byte_offset_to_line_col(source, e.span.start);
    format!(
        "parse error at {}:{}: {}{}",
        line,
        col,
        e.message,
        format_source_context(source, line, col),
    )
}

#[cfg(test)]
mod tests {
    use super::eval_module_source;
    use crate::meow_meow::object::Value;

    #[test]
    fn component_body_named_props_can_reference_same_named_bindings() {
        let source = r#"
fn make_payload(row_name, label, mode_value) {
    return Data {
        row_name = row_name
        label = label
        mode_value = mode_value
        row_kind = "EditorMode"
        interactive = true
    }
}

export let payload = make_payload("editor_settings_mode_cursor_3d", "3D Cursor", "cursor_3d")
"#;

        let module = eval_module_source(source, None).expect("module eval");
        let Value::Module { named, .. } = module else {
            panic!("expected module value");
        };
        let payload = named.get("payload").expect("exported payload");
        let Value::ComponentExpr(component) = payload else {
            panic!("expected component expr");
        };

        assert!(component.named.iter().any(|(key, value)| {
            key == "row_name"
                && matches!(value, Value::String(value) if value == "editor_settings_mode_cursor_3d")
        }));
        assert!(component.named.iter().any(|(key, value)| {
            key == "label" && matches!(value, Value::String(value) if value == "3D Cursor")
        }));
        assert!(component.named.iter().any(|(key, value)| {
            key == "mode_value" && matches!(value, Value::String(value) if value == "cursor_3d")
        }));
        assert!(component.component_property_assignment_only);
    }

    #[test]
    fn structural_component_bodies_keep_normal_reassign_semantics() {
        let source = r#"
fn rename_label() {
    let label = "hello"
    return T {
        label = "goodbye"
        Text { label }
    }
}

export let payload = rename_label()
"#;

        let module = eval_module_source(source, None).expect("module eval");
        let Value::Module { named, .. } = module else {
            panic!("expected module value");
        };
        let payload = named.get("payload").expect("exported payload");
        let Value::ComponentExpr(component) = payload else {
            panic!("expected component expr");
        };

        assert!(!component.component_property_assignment_only);
        assert!(component.named.is_empty());
        let Some(crate::meow_meow::object::CeChild::Spawn(text_child)) = component.children.first()
        else {
            panic!("expected text child");
        };
        assert!(matches!(
            text_child.positionals.first(),
            Some(Value::String(label)) if label == "goodbye"
        ));
    }

    #[test]
    fn structural_component_bodies_still_capture_universal_named_props() {
        let source = r#"
export let payload = T {
    name = "paint_panel_root"
    id = "paint_panel_root"
    Text { "hello" }
}
"#;

        let module = eval_module_source(source, None).expect("module eval");
        let Value::Module { named, .. } = module else {
            panic!("expected module value");
        };
        let payload = named.get("payload").expect("exported payload");
        let Value::ComponentExpr(component) = payload else {
            panic!("expected component expr");
        };

        assert!(!component.component_property_assignment_only);
        assert!(component.named.iter().any(|(key, value)| {
            key == "name" && matches!(value, Value::String(value) if value == "paint_panel_root")
        }));
        assert!(component.named.iter().any(|(key, value)| {
            key == "id" && matches!(value, Value::String(value) if value == "paint_panel_root")
        }));
    }

    #[test]
    fn module_eval_supports_table_literals_and_field_access() {
        let source = r#"
export let label = {
    theme = {
        label = "aurora"
    }
}.theme.label
"#;

        let module = eval_module_source(source, None).expect("module eval");
        let Value::Module { named, .. } = module else {
            panic!("expected module value");
        };
        assert!(matches!(
            named.get("label"),
            Some(Value::String(label)) if label == "aurora"
        ));
    }

    #[test]
    fn module_eval_supports_for_in_over_tables() {
        let source = r#"
let total = 0
for entry in {
    apples = 2
    pears = 5
} {
    total = total + entry.value
}
export let total = total
"#;

        let module = eval_module_source(source, None).expect("module eval");
        let Value::Module { named, .. } = module else {
            panic!("expected module value");
        };
        assert!(
            matches!(named.get("total"), Some(Value::Number(total)) if (*total - 7.0).abs() < 1e-6)
        );
    }
}
