use std::collections::HashMap;
use std::thread::{self, JoinHandle};

use rtrb::{Consumer, Producer, RingBuffer};

use crate::engine::ecs::component::AnimationState;
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::IntentValue;
use crate::engine::ecs::SignalEmitter;
use crate::engine::ecs::SignalKind;
use crate::engine::ecs::World;
use crate::meow_meow::ast::{
    BinOpKind, CallExpression, ComponentExpression,
    Expression, IfStatement, ImportItem, Statement, UnaryOpKind,
};
use crate::meow_meow::object::{CeChild, FrameKind, MaterializedCE, ObjectWorld, Value};
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
    EvalScript { source: String, source_path: Option<String> },
    /// Parse only — returns a debug AST string (used in tests / tooling).
    ParseScript { source: String },
    /// Reply to a pending `EvalResponse::HostCall`. The `id` must match the
    /// correlation id from the HostCall that is being answered.
    HostCallResult { id: u32, value: HostValue },
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum EvalResponse {
    /// A `SpawnComponentTree` (or other) intent ready to be pushed into the engine.
    Intent(IntentValue),
    /// Parse-only debug output (from `ParseScript`).
    ParsedOk { debug_ast: String },
    Error { message: String },
    ShutdownAck,
    /// The evaluator needs the host to perform a side-effecting operation and
    /// return a result before evaluation can continue. The host must push a
    /// matching `EvalRequest::HostCallResult { id, value }` to unblock the
    /// evaluator thread.
    HostCall { id: u32, kind: HostCallKind },
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
            Ok(EvalRequest::EvalScript { source, source_path }) => {
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
    while responses.push(EvalResponse::HostCall { id, kind: kind.clone() }).is_err() {
        std::thread::yield_now();
    }
    loop {
        match requests.pop() {
            Ok(EvalRequest::HostCallResult { id: reply_id, value }) if reply_id == id => {
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
}

/// Accumulator used while evaluating a component expression body block.
/// Collects the results that will be materialized into a `MaterializedCE`.
struct CeBuilder {
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
        while ch.responses.push(EvalResponse::Intent(intent.clone())).is_err() {
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
fn eval_block_stmts(
    stmts: &[Statement],
    ctx: &mut EvalContext<'_>,
) -> Result<StmtEffect, String> {
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

fn eval_stmt(
    stmt: &Statement,
    ctx: &mut EvalContext<'_>,
) -> Result<StmtEffect, String> {
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
        Statement::Reassign { name, value } => {
            let val = eval_expr(value, ctx)?;
            // Inside a CE body: if name not in scope, capture as named property.
            if !ctx.object_world.has(&name.0) {
                if let Some(builder) = ctx.ce_builder.as_mut() {
                    builder.named.push((name.0.clone(), val));
                    return Ok(StmtEffect::None);
                }
            }
            let val = maybe_register_live_component_value(val, ctx);
            ctx.object_world.reassign(&name.0, val)?;
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
        Statement::ForIn { binding, iterable, body } => {
            let items = match eval_expr(iterable, ctx)? {
                Value::Array(a) => a,
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
            let module_val = eval_as_module(&content, Some(&resolved))?;
            let (named, sequence) = match module_val {
                Value::Module { named, sequence } => (named, sequence),
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
                        ctx.object_world.bind(
                            alias.0.clone(),
                            Value::ComponentExpr(Box::new(ce.clone())),
                        );
                    }
                }
            }
            Ok(StmtEffect::None)
        }
    }
}

fn maybe_register_live_component_value(
    val: Value,
    ctx: &mut EvalContext<'_>,
) -> Value {
    // In live mode, binding or reassigning a CE should produce a live handle
    // rather than leave a dead ComponentExpr in scope.
    match (val, ctx.channels.as_mut()) {
        (Value::ComponentExpr(ce), Some(ch)) => {
            let component_type = ce.component_type.clone();
            match ch.call(HostCallKind::Register(*ce.clone())) {
                Some(HostValue::ComponentId(id)) => Value::ComponentObject { id, component_type },
                _ => Value::ComponentExpr(ce),
            }
        }
        (val, _) => val,
    }
}

fn eval_expr_stmt(
    expr: &Expression,
    ctx: &mut EvalContext<'_>,
) -> Result<(), String> {
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
                let args: Vec<Value> = call.args.iter()
                    .map(|a| eval_expr(a, ctx))
                    .collect::<Result<_, _>>()?;
                ctx.ce_builder.as_mut().unwrap().calls.push((callee_id.0.clone(), args));
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
            Value::ComponentExpr(ce) => ctx.ce_builder.as_mut().unwrap()
                .children.push(CeChild::Spawn(*ce)),
            // Reference to a previously Registered live component — splice
            // the detached subtree as a child of the parent CE rather than
            // discarding the value or re-spawning it.
            Value::ComponentObject { id, .. } => {
                ctx.ce_builder.as_mut().unwrap()
                    .children.push(CeChild::Attach(id));
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
            ctx.emits.push(IntentValue::SpawnComponentTree { root: ce, parent: None });
        }
        // Bare top-level reference to a previously Registered ComponentObject —
        // attach as a world root and run the deferred init walk.
        Value::ComponentObject { id, .. } => {
            if let Some(ch) = ctx.channels.as_mut() {
                ch.call(HostCallKind::Attach { parent: None, child: id });
            }
        }
        _ => {}
    }
}

fn is_builtin_fn(name: &str) -> bool {
    matches!(
        name,
        "print" | "assert" | "range" | "emit" | "on" | "query" | "query_all"
    )
}

fn eval_if(
    if_stmt: &IfStatement,
    ctx: &mut EvalContext<'_>,
) -> Result<StmtEffect, String> {
    let cond = eval_expr(&if_stmt.condition, ctx)?;
    let stmts = if is_truthy(&cond) {
        Some(&if_stmt.then_branch.statements)
    } else {
        if_stmt.else_branch.as_ref().map(|b| &b.statements)
    };
    match stmts {
        Some(s) => {
            ctx.object_world.push_frame(FrameKind::Block);
            let result = eval_block_stmts(s, ctx);
            ctx.object_world.pop_frame();
            result
        }
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
    // Evaluate all constructor calls.
    let mut ctor_method: Option<String> = None;
    let mut ctor_args: Vec<Value> = vec![];
    let mut extra_ctor_calls: Vec<(String, Vec<Value>)> = vec![];
    for (i, ctor) in ce.constructors.iter().enumerate() {
        let args: Vec<Value> = ctor.args.iter()
            .map(|a| eval_expr(a, ctx))
            .collect::<Result<_, _>>()?;
        if i == 0 {
            ctor_method = Some(ctor.method.0.clone());
            ctor_args = args;
        } else {
            extra_ctor_calls.push((ctor.method.0.clone(), args));
        }
    }

    // Evaluate the body block with a CE builder context.
    let mut builder = CeBuilder {
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
        };
        eval_block_stmts(&ce.body.statements, &mut body_ctx)
    };
    ctx.object_world.pop_frame();
    body_result?;

    let mce = MaterializedCE {
        component_type: ce.component_type.0.clone(),
        ctor_method,
        ctor_args,
        calls: builder.calls,
        named: builder.named,
        positionals: builder.positionals,
        children: builder.children,
    };
    Ok(Value::ComponentExpr(Box::new(mce)))
}

fn eval_expr(
    expr: &Expression,
    ctx: &mut EvalContext<'_>,
) -> Result<Value, String> {
    match expr {
        Expression::Null => Ok(Value::Null),
        Expression::Bool(b) => Ok(Value::Bool(*b)),
        Expression::Number(n) => Ok(Value::Number(*n)),
        Expression::Dimension(n, unit) => Ok(Value::Dimension { value: *n, unit: *unit }),
        Expression::String(s) => Ok(Value::String(s.clone())),
        Expression::Array(items) => {
            let vals = items
                .iter()
                .map(|e| eval_expr(e, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Array(vals))
        }
        Expression::Identifier(id) => {
            // Look up in scope chain; fall back to bare identifier value (for enum-like flags).
            match ctx.object_world.lookup(&id.0) {
                Some(val) => Ok(val.clone()),
                None => Ok(Value::Identifier(id.0.clone())),
            }
        }
        Expression::Component(ce) => eval_ce(ce, ctx),
        Expression::Function { params, body } => Ok(Value::Function {
            params: params.iter().map(|p| p.0.clone()).collect(),
            body: body.clone(),
            captured_env: ctx.object_world.snapshot_visible(),
        }),
        Expression::Call(call) => eval_call(call, ctx),
        Expression::BinaryOp { op, lhs, rhs } => eval_binop(op, lhs, rhs, ctx),
        Expression::UnaryOp { op, operand } => eval_unaryop(op, operand, ctx),
    }
}

fn eval_call(
    call: &CallExpression,
    ctx: &mut EvalContext<'_>,
) -> Result<Value, String> {
    // Method call: `obj.method(args)` — callee is BinaryOp(Dot, lhs, rhs).
    if let Expression::BinaryOp { op: BinOpKind::Dot, lhs, rhs } = call.callee.as_ref() {
        let receiver = eval_expr(lhs, ctx)?;
        let method_name = match rhs.as_ref() {
            Expression::Identifier(id) => id.0.clone(),
            other => return Err(format!("method call: RHS of '.' must be an identifier, got {:?}", other)),
        };
        let args: Vec<Value> = call.args.iter()
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
        let arg = call.args.first().map(|a| eval_expr(a, ctx)).transpose()?
            .unwrap_or(Value::Null);
        println!("[mms] {}", value_display(&arg));
        return Ok(Value::Null);
    }

    // Built-in: assert(cond, msg)
    if callee_name == "assert" {
        let cond = call.args.first().map(|a| eval_expr(a, ctx)).transpose()?
            .unwrap_or(Value::Null);
        if !is_truthy(&cond) {
            let msg = call.args.get(1).map(|a| eval_expr(a, ctx)).transpose()?
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
        let args: Vec<Value> = call.args.iter()
            .map(|a| eval_expr(a, ctx))
            .collect::<Result<_, _>>()?;
        let selector = match args.first() {
            Some(Value::String(s)) => s.clone(),
            other => return Err(format!(
                "{}(): arg 0 must be a string selector, got {:?}", callee_name, other
            )),
        };
        let handler = match args.get(1) {
            Some(f @ Value::Function { .. }) => Some(f.clone()),
            None => None,
            other => return Err(format!(
                "{}(): arg 1 (optional) must be a function, got {:?}", callee_name, other
            )),
        };
        let result = run_world_query(selector, None, multiple, ctx)?;
        return dispatch_query_result(result, handler, multiple, ctx);
    }

    // Built-in: on(component_object, "SignalKind", fn(event) { ... })
    if callee_name == "on" {
        let args: Vec<Value> = call.args.iter()
            .map(|a| eval_expr(a, ctx))
            .collect::<Result<_, _>>()?;
        let scope = match args.get(0) {
            Some(Value::ComponentObject { id, .. }) => *id,
            other => return Err(format!("on(): arg 0 must be a ComponentObject, got {:?}", other)),
        };
        let signal_kind = match args.get(1) {
            Some(Value::String(s)) => parse_signal_kind(s)?,
            other => return Err(format!("on(): arg 1 must be a signal kind string, got {:?}", other)),
        };
        let handler = match args.get(2) {
            Some(f @ Value::Function { .. }) => f.clone(),
            other => return Err(format!("on(): arg 2 must be a function, got {:?}", other)),
        };
        if let Some(ch) = ctx.channels.as_mut() {
            ch.call(HostCallKind::RegisterHandler { scope, signal_kind, handler });
        }
        return Ok(Value::Null);
    }

    let callee_val = match ctx.object_world.lookup(callee_name) {
        Some(v) => v.clone(),
        None => return Err(format!("undefined: '{}'", callee_name)),
    };

    match callee_val {
        Value::Function { params, body, captured_env } => {
            let args: Vec<Value> = call
                .args
                .iter()
                .map(|a| eval_expr(a, ctx))
                .collect::<Result<_, _>>()?;

            // Push a Function frame seeded with the closure's captured env;
            // bind arg values into the same frame so they shadow captured names.
            ctx.object_world.push_function_frame(captured_env);
            for (param, arg) in params.iter().zip(args.into_iter()) {
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
        let reply = ch.call(HostCallKind::Query { selector, scope, multiple });
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
                return Err(format!(
                    "query: unexpected HostValue reply: {:?}", other
                ));
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
                world.component_name(id).map(|component_type| Value::ComponentObject {
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
    let Value::Function { params, body, captured_env } = handler else {
        return Err(format!("expected function, got {:?}", handler));
    };
    ctx.object_world.push_function_frame(captured_env.clone());
    for (param, arg) in params.iter().zip(args.into_iter()) {
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
        Value::ComponentObject { id, ref component_type } => {
            // Subtree query — `comp.query("sel")` / `comp.query_all("sel")`.
            // Also accepts an optional handler arg (same shape as the free builtins).
            if method == "query" || method == "query_all" {
                let multiple = method == "query_all";
                let selector = match args.first() {
                    Some(Value::String(s)) => s.clone(),
                    other => return Err(format!(
                        "{}(): arg 0 must be a string selector, got {:?}", method, other
                    )),
                };
                let handler = match args.get(1) {
                    Some(f @ Value::Function { .. }) => Some(f.clone()),
                    None => None,
                    other => return Err(format!(
                        "{}(): arg 1 (optional) must be a function, got {:?}", method, other
                    )),
                };
                let result = run_world_query(selector, Some(id), multiple, ctx)?;
                return dispatch_query_result(result, handler, multiple, ctx);
            }

            // Animation playback.
            let anim_state = match method {
                "play"
                    if matches!(
                        component_type.as_str(),
                        "A" | "Animation" | "AnimationComponent" | "animation"
                    ) => Some(AnimationState::Playing),
                "loop_anim"
                    if matches!(
                        component_type.as_str(),
                        "A" | "Animation" | "AnimationComponent" | "animation"
                    ) => Some(AnimationState::Looping),
                "pause"
                    if matches!(
                        component_type.as_str(),
                        "A" | "Animation" | "AnimationComponent" | "animation"
                    ) => Some(AnimationState::Paused),
                _ => None,
            };
            if let Some(state) = anim_state {
                ctx.emits.push(IntentValue::SetAnimationState {
                    component_ids: vec![id],
                    state,
                });
                return Ok(Value::Null);
            }

            // Layout getter: layout.available_width() → current width as Number.
            if matches!(component_type.as_str(), "layout" | "LayoutRoot" | "LayoutComponent")
                && method == "available_width"
            {
                use crate::engine::ecs::component::LayoutComponent;
                let Some(world) = ctx.host_world else {
                    return Err("available_width(): no host world".into());
                };
                let world = unsafe { &mut *world };
                let w = world
                    .get_component_by_id_as::<LayoutComponent>(id)
                    .map(|lo| lo.available_width as f64)
                    .ok_or_else(|| "available_width(): not a LayoutComponent".to_string())?;
                return Ok(Value::Number(w));
            }

            // Layout mutation: layout.set_available_width(N).
            if matches!(component_type.as_str(), "layout" | "LayoutRoot" | "LayoutComponent")
                && method == "set_available_width"
            {
                let width = match args.first() {
                    Some(Value::Number(n)) => *n as f32,
                    Some(other) => return Err(format!(
                        "set_available_width: expected number argument, got {:?}", other
                    )),
                    None => return Err("set_available_width: missing number argument".into()),
                };
                ctx.emits.push(IntentValue::SetLayoutAvailableWidth {
                    component_ids: vec![id],
                    width,
                });
                return Ok(Value::Null);
            }

            // Text mutation: text.set_text("...").
            if matches!(
                component_type.as_str(),
                "Text" | "TXT" | "TextComponent" | "text"
            )
                && method == "set_text"
            {
                let text = match args.first() {
                    Some(Value::String(s)) => s.clone(),
                    Some(other) => return Err(format!(
                        "set_text: expected string argument, got {:?}", other
                    )),
                    None => return Err("set_text: missing string argument".into()),
                };
                ctx.emits.push(IntentValue::SetText {
                    component_ids: vec![id],
                    text,
                });
                return Ok(Value::Null);
            }

            if matches!(
                component_type.as_str(),
                "Text" | "TXT" | "TextComponent" | "text"
            )
                && method == "set_font_size"
            {
                let font_size = match args.first() {
                    Some(Value::Number(n)) => *n as f32,
                    Some(other) => return Err(format!(
                        "set_font_size: expected number argument, got {:?}", other
                    )),
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
                if let Some(t) = world.get_component_by_id_as_mut::<crate::engine::ecs::component::TextComponent>(id) {
                    t.set_font_size(font_size);
                }
                ctx.emits.push(IntentValue::SetText {
                    component_ids: vec![id],
                    text: cur_text,
                });
                return Ok(Value::Null);
            }

            Err(format!("no method '{}' on component type '{}'", method, component_type))
        }
        other => Err(format!("method call '{}': receiver is not a ComponentObject, got {:?}", method, other)),
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
            return Err("query operator '->' was not desugared by QueryDesugarTransform".to_string());
        }
        BinOpKind::Pipe => {
            let lhs_val = eval_expr(lhs, ctx)?;
            let rhs_val = eval_expr(rhs, ctx)?;
            match rhs_val {
                Value::Function { params, body, captured_env } => {
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
        _ => {}
    }

    let l = eval_expr(lhs, ctx)?;
    let r = eval_expr(rhs, ctx)?;

    match op {
        BinOpKind::Add => match (l, r) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
            (Value::String(a), Value::String(b)) => Ok(Value::String(a + &b)),
            (Value::String(a), r) => Ok(Value::String(a + &value_display(&r))),
            (l, Value::String(b)) => Ok(Value::String(value_display(&l) + &b)),
            (l, r) => Err(format!("type error: cannot add {:?} and {:?}", l, r)),
        },
        BinOpKind::Sub => match (l, r) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a - b)),
            (l, r) => Err(format!("type error: cannot subtract {:?} from {:?}", r, l)),
        },
        BinOpKind::Mul => match (l, r) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a * b)),
            (l, r) => Err(format!("type error: cannot multiply {:?} and {:?}", l, r)),
        },
        BinOpKind::Div => match (l, r) {
            (Value::Number(a), Value::Number(b)) => {
                if b == 0.0 {
                    return Err("division by zero".to_string());
                }
                Ok(Value::Number(a / b))
            }
            (l, r) => Err(format!("type error: cannot divide {:?} by {:?}", l, r)),
        },
        BinOpKind::Rem => match (l, r) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a % b)),
            (l, r) => Err(format!("type error: cannot rem {:?} by {:?}", l, r)),
        },
        BinOpKind::Eq    => Ok(Value::Bool(values_equal(&l, &r))),
        BinOpKind::NotEq => Ok(Value::Bool(!values_equal(&l, &r))),
        BinOpKind::Lt    => num_cmp(l, r, |a, b| a < b),
        BinOpKind::Gt    => num_cmp(l, r, |a, b| a > b),
        BinOpKind::LtEq  => num_cmp(l, r, |a, b| a <= b),
        BinOpKind::GtEq  => num_cmp(l, r, |a, b| a >= b),
        BinOpKind::And | BinOpKind::Or | BinOpKind::Pipe | BinOpKind::Query => unreachable!("handled above"),
        BinOpKind::Dot => Err("'.' must be used as part of a method call: obj.method(args)".into()),
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
            v => Err(format!("type error: cannot negate {:?}", v)),
        },
        UnaryOpKind::Not => Ok(Value::Bool(!is_truthy(&val))),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
        Value::Array(arr) => format!("[{}]", arr.iter().map(value_display).collect::<Vec<_>>().join(", ")),
        Value::Function { .. } => "<fn>".into(),
        Value::ComponentObject { id, component_type } => format!("<{}:{:?}>", component_type, id),
        Value::ComponentExpr(_) => "<ce>".into(),
        Value::Object(_) => "<object>".into(),
        Value::Identifier(s) => s.clone(),
        Value::Module { .. } => "<module>".into(),
        Value::Dimension { value, unit } => {
            let suffix = match unit {
                crate::meow_meow::token::Unit::Percent => "%",
                crate::meow_meow::token::Unit::GlyphUnits => "gu",
                crate::meow_meow::token::Unit::Degrees => "deg",
                crate::meow_meow::token::Unit::Radians => "rad",
            };
            format!("{}{}", value, suffix)
        }
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
        (Value::Null, Value::Null)           => true,
        (Value::Bool(a), Value::Bool(b))     => a == b,
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
        "Click"            => Ok(SignalKind::Click),
        "DragStart"        => Ok(SignalKind::DragStart),
        "DragMove"         => Ok(SignalKind::DragMove),
        "DragEnd"          => Ok(SignalKind::DragEnd),
        "RayIntersected"   => Ok(SignalKind::RayIntersected),
        "ParentChanged"    => Ok(SignalKind::ParentChanged),
        "CollisionStarted" => Ok(SignalKind::CollisionStarted),
        "CollisionEnded"   => Ok(SignalKind::CollisionEnded),
        "Scrolling"        => Ok(SignalKind::Scrolling),
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
    world_host: Option<&mut World>,
    emit: Option<&mut dyn SignalEmitter>,
) -> Result<Value, String> {
    let Value::Function { params, body, captured_env } = fn_val else {
        return Err(format!("eval_mms_fn: expected Function, got {:?}", fn_val));
    };
    let mut emits: Vec<IntentValue> = Vec::new();
    let mut world = ObjectWorld::new();
    world.push_function_frame(captured_env.clone());
    for (param, arg) in params.iter().zip(args) {
        world.bind(param.clone(), arg);
    }
    let mut ctx = EvalContext {
        emits: &mut emits,
        source_path: None,
        channels: None,
        ce_builder: None,
        object_world: &mut world,
        host_world: world_host.map(|world| world as *mut World),
    };
    let result = match eval_block_stmts(&body.statements, &mut ctx)? {
        StmtEffect::Return(val) => val,
        _ => Value::Null,
    };
    if let Some(em) = emit {
        for iv in emits {
            em.push_intent_now(ComponentId::default(), iv);
        }
    }
    Ok(result)
}

/// Evaluate a source file as a module (sandboxed — emits go to `sequence`, not the engine).
/// Returns `Value::Module { named, sequence }`.
fn eval_as_module(source: &str, source_path: Option<&str>) -> Result<Value, String> {
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

    Ok(Value::Module { named, sequence })
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
    let line_text = source
        .lines()
        .nth(line.saturating_sub(1))
        .unwrap_or("");
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
