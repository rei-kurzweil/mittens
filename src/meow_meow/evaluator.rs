use std::collections::HashMap;
use std::thread::{self, JoinHandle};

use rtrb::{Consumer, Producer, RingBuffer};

use crate::engine::ecs::IntentValue;
use crate::meow_meow::ast::{
    BinOpKind, CallExpression, ComponentBodyItem, ComponentExpression, ConstructorCall,
    Expression, Ident, IfStatement, ImportItem, Statement, UnaryOpKind,
};
use crate::meow_meow::object::Value;
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

fn evaluator_thread(mut requests: Consumer<EvalRequest>, mut responses: Producer<EvalResponse>) {
    loop {
        match requests.pop() {
            Ok(EvalRequest::EvalScript { source, source_path }) => {
                eval_script(&source, source_path.as_deref(), &mut responses);
            }
            Ok(EvalRequest::ParseScript { source }) => {
                let resp = parse_only(&source)
                    .map(|dbg| EvalResponse::ParsedOk { debug_ast: dbg })
                    .unwrap_or_else(|msg| EvalResponse::Error { message: msg });
                let _ = responses.push(resp);
            }
            Ok(EvalRequest::Shutdown) => {
                let _ = responses.push(EvalResponse::ShutdownAck);
                break;
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

type Env = HashMap<String, Value>;

/// Shared mutable context threaded through statement evaluation.
///
/// Carries everything that statement-level eval functions need beyond the
/// immutable `env` and the statement itself. Keeping these together means
/// adding future context (module cache, call-depth limit, …) is a one-field
/// change here rather than a signature change across every eval function.
///
/// Expression-level functions (`eval_expr`, `eval_call`, …) take only
/// `emits: &mut Vec<IntentValue>` — they cannot contain import statements and
/// do not need the full context.
struct EvalContext<'a> {
    /// Accumulates `SpawnComponentTree` intents produced by `emit(ce)` calls.
    emits: &'a mut Vec<IntentValue>,
    /// Filesystem path of the file being evaluated, used to resolve relative
    /// `import "…"` paths. `None` inside function call bodies (closures do not
    /// carry their definition-site path) and when source was provided as a
    /// raw string without a path (e.g. via `MeowMeowRunner::eval`).
    source_path: Option<&'a str>,
}

/// Evaluate a script: parse → AstTransforms → walk statements.
/// Each `emit(ce)` call produces an `EvalResponse::Intent(SpawnComponentTree)`.
fn eval_script(source: &str, source_path: Option<&str>, responses: &mut Producer<EvalResponse>) {
    let mut stmts = match parse_source(source) {
        Ok(s) => s,
        Err(msg) => {
            let _ = responses.push(EvalResponse::Error { message: msg });
            return;
        }
    };

    EmitLiftTransform::apply(&mut stmts);
    QueryDesugarTransform::apply(&mut stmts);

    let mut env: Env = HashMap::new();
    let mut emits: Vec<IntentValue> = Vec::new();
    let mut ctx = EvalContext { emits: &mut emits, source_path };

    match eval_block_stmts(&stmts, &mut env, &mut ctx) {
        Ok(_) => {}
        Err(msg) => {
            let _ = responses.push(EvalResponse::Error { message: msg });
        }
    }

    for intent in emits {
        while responses.push(EvalResponse::Intent(intent.clone())).is_err() {
            std::thread::yield_now();
        }
    }
}

// ---------------------------------------------------------------------------
// Statement evaluation
// ---------------------------------------------------------------------------

/// Effect produced by evaluating a statement (excluding emits, which go to the emits vec).
enum StmtEffect {
    None,
    Bind(String, Value),
    /// Like `Bind`, but also registers in the module's named exports map.
    Export(String, Value),
    /// Multiple bindings from an `import` statement.
    ImportBindings(Vec<(String, Value)>),
    /// `x = expr` — update an existing binding in the current env.
    Reassign(String, Value),
    Return(Value),
    Break,
    Continue,
}

/// Evaluate a block of statements, applying all bindings and reassignments
/// directly to `env` (no clone). Bind effects from `let` statements extend env
/// in place; Reassign effects propagate to env (so reassignment inside if-blocks
/// is visible to the enclosing scope). Returns the first control-flow effect
/// (Return/Break/Continue) encountered, or None.
fn eval_block_stmts(
    stmts: &[Statement],
    env: &mut Env,
    ctx: &mut EvalContext<'_>,
) -> Result<StmtEffect, String> {
    for stmt in stmts {
        match eval_stmt(stmt, env, ctx)? {
            StmtEffect::Bind(name, val) | StmtEffect::Export(name, val) => {
                env.insert(name, val);
            }
            StmtEffect::ImportBindings(bindings) => {
                for (name, val) in bindings {
                    env.insert(name, val);
                }
            }
            StmtEffect::Reassign(name, val) => {
                if env.contains_key(&name) {
                    env.insert(name, val);
                } else {
                    return Err(format!("reassignment: '{}' is not defined", name));
                }
            }
            StmtEffect::None => {}
            effect => return Ok(effect),
        }
    }
    Ok(StmtEffect::None)
}

fn eval_stmt(
    stmt: &Statement,
    env: &mut Env,
    ctx: &mut EvalContext<'_>,
) -> Result<StmtEffect, String> {
    match stmt {
        Statement::Assignment(a) => {
            let val = eval_expr(&a.value, env, ctx.emits)?;
            if a.exported {
                Ok(StmtEffect::Export(a.name.0.clone(), val))
            } else {
                Ok(StmtEffect::Bind(a.name.0.clone(), val))
            }
        }
        Statement::Reassign { name, value } => {
            let val = eval_expr(value, env, ctx.emits)?;
            Ok(StmtEffect::Reassign(name.0.clone(), val))
        }
        Statement::Expression(expr) => {
            eval_expr_stmt(expr, env, ctx.emits)?;
            Ok(StmtEffect::None)
        }
        Statement::Return(r) => {
            let val = match &r.value {
                Some(expr) => eval_expr(expr, env, ctx.emits)?,
                None => Value::Null,
            };
            Ok(StmtEffect::Return(val))
        }
        Statement::If(if_stmt) => eval_if(if_stmt, env, ctx),
        Statement::Block(block) => eval_block_stmts(&block.statements, env, ctx),
        Statement::Break => Ok(StmtEffect::Break),
        Statement::Continue => Ok(StmtEffect::Continue),
        Statement::ForIn { binding, iterable, body } => {
            let items = match eval_expr(iterable, env, ctx.emits)? {
                Value::Array(a) => a,
                other => return Err(format!("for/in: expected array, got {:?}", other)),
            };
            // `loop_env` persists across iterations so that reassignment (accumulator
            // patterns like `sum = sum + i`) propagates between iterations.
            let mut loop_env = env.clone();
            'for_loop: for item in items {
                loop_env.insert(binding.0.clone(), item);
                for stmt in &body.statements {
                    match eval_stmt(stmt, &mut loop_env, ctx)? {
                        StmtEffect::None => {}
                        StmtEffect::Bind(n, v) | StmtEffect::Export(n, v) => {
                            loop_env.insert(n, v);
                        }
                        StmtEffect::Reassign(n, v) => {
                            if loop_env.contains_key(&n) {
                                loop_env.insert(n, v);
                            } else {
                                return Err(format!("reassignment: '{}' is not defined", n));
                            }
                        }
                        StmtEffect::ImportBindings(bs) => {
                            for (n, v) in bs { loop_env.insert(n, v); }
                        }
                        StmtEffect::Return(val) => return Ok(StmtEffect::Return(val)),
                        StmtEffect::Break => return Ok(StmtEffect::None),
                        StmtEffect::Continue => continue 'for_loop,
                    }
                }
            }
            Ok(StmtEffect::None)
        }
        Statement::While { condition, body } => {
            let mut loop_env = env.clone();
            'while_loop: loop {
                let cond = eval_expr(condition, &loop_env, ctx.emits)?;
                if !is_truthy(&cond) {
                    break;
                }
                for stmt in &body.statements {
                    match eval_stmt(stmt, &mut loop_env, ctx)? {
                        StmtEffect::None => {}
                        StmtEffect::Bind(n, v) | StmtEffect::Export(n, v) => {
                            loop_env.insert(n, v);
                        }
                        StmtEffect::Reassign(n, v) => {
                            if loop_env.contains_key(&n) {
                                loop_env.insert(n, v);
                            } else {
                                return Err(format!("reassignment: '{}' is not defined", n));
                            }
                        }
                        StmtEffect::ImportBindings(bs) => {
                            for (n, v) in bs { loop_env.insert(n, v); }
                        }
                        StmtEffect::Return(val) => return Ok(StmtEffect::Return(val)),
                        StmtEffect::Break => break 'while_loop,
                        StmtEffect::Continue => continue 'while_loop,
                    }
                }
            }
            Ok(StmtEffect::None)
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
            let mut bindings = Vec::new();
            for item in items {
                match item {
                    ImportItem::Named(id) => {
                        let val = named.get(&id.0).cloned().ok_or_else(|| {
                            format!("import: '{}' is not exported from '{}'", id.0, path)
                        })?;
                        bindings.push((id.0.clone(), val));
                    }
                    ImportItem::NamedAlias { name, alias } => {
                        let val = named.get(&name.0).cloned().ok_or_else(|| {
                            format!("import: '{}' is not exported from '{}'", name.0, path)
                        })?;
                        bindings.push((alias.0.clone(), val));
                    }
                    ImportItem::PositionalAlias { index, alias } => {
                        let ce = sequence.get(*index).ok_or_else(|| {
                            format!("import: index {} out of range in '{}'", index, path)
                        })?;
                        bindings.push((alias.0.clone(), Value::ComponentExpr(Box::new(ce.clone()))));
                    }
                }
            }
            Ok(StmtEffect::ImportBindings(bindings))
        }
    }
}

fn eval_expr_stmt(
    expr: &Expression,
    env: &Env,
    emits: &mut Vec<IntentValue>,
) -> Result<(), String> {
    // Special case: emit(expr) — produced by EmitLiftTransform or written explicitly.
    if let Expression::Call(call) = expr {
        if call.callee == Ident("emit".into()) {
            if let Some(arg) = call.args.first() {
                let val = eval_expr(arg, env, emits)?;
                push_component_emit(val, emits);
            }
            return Ok(());
        }
    }

    // General case: evaluate and emit if the result is a ComponentExpr.
    let val = eval_expr(expr, env, emits)?;
    push_component_emit(val, emits);
    Ok(())
}

fn push_component_emit(val: Value, emits: &mut Vec<IntentValue>) {
    if let Value::ComponentExpr(ce) = val {
        emits.push(IntentValue::SpawnComponentTree { root: ce, parent: None });
    }
}

fn eval_if(
    if_stmt: &IfStatement,
    env: &mut Env,
    ctx: &mut EvalContext<'_>,
) -> Result<StmtEffect, String> {
    let cond = eval_expr(&if_stmt.condition, env, ctx.emits)?;
    if is_truthy(&cond) {
        eval_block_stmts(&if_stmt.then_branch.statements, env, ctx)
    } else if let Some(else_branch) = &if_stmt.else_branch {
        eval_block_stmts(&else_branch.statements, env, ctx)
    } else {
        Ok(StmtEffect::None)
    }
}

// ---------------------------------------------------------------------------
// Expression evaluation
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// CE environment substitution
// ---------------------------------------------------------------------------

/// Convert an evaluated `Value` back to a literal `Expression` so it can be
/// stored inside a `ComponentExpression` (which holds `Expression`, not `Value`).
/// Non-literal values (functions, live component objects) become `Null`.
fn value_to_expr(val: Value) -> Expression {
    match val {
        Value::Null => Expression::Null,
        Value::Bool(b) => Expression::Bool(b),
        Value::Number(n) => Expression::Number(n),
        Value::String(s) => Expression::String(s),
        Value::Identifier(s) => Expression::Identifier(Ident(s)),
        Value::Array(items) => Expression::Array(items.into_iter().map(value_to_expr).collect()),
        Value::ComponentExpr(ce) => Expression::Component(*ce),
        _ => Expression::Null,
    }
}

/// Evaluate an expression against the env and return a literal expression.
/// If evaluation fails, fall back to the original expression unchanged.
fn eval_to_literal(expr: &Expression, env: &Env, emits: &mut Vec<IntentValue>) -> Expression {
    match eval_expr(expr, env, emits) {
        Ok(val) => value_to_expr(val),
        Err(_) => expr.clone(),
    }
}

/// Recursively substitute all expressions in a ComponentExpression with their
/// evaluated-and-literalized equivalents from the current environment. This
/// ensures that loop variables, computed values, etc. are captured as concrete
/// literals when the CE is emitted, rather than stored as unresolved identifiers.
fn subst_ce(ce: &ComponentExpression, env: &Env, emits: &mut Vec<IntentValue>) -> ComponentExpression {
    ComponentExpression {
        component_type: ce.component_type.clone(),
        constructor: ce.constructor.as_ref().map(|c| ConstructorCall {
            method: c.method.clone(),
            args: c.args.iter().map(|a| eval_to_literal(a, env, emits)).collect(),
        }),
        body: ce.body.iter().map(|item| subst_body_item(item, env, emits)).collect(),
    }
}

fn subst_body_item(item: &ComponentBodyItem, env: &Env, emits: &mut Vec<IntentValue>) -> ComponentBodyItem {
    match item {
        ComponentBodyItem::NamedAssignment { name, value } => ComponentBodyItem::NamedAssignment {
            name: name.clone(),
            value: eval_to_literal(value, env, emits),
        },
        ComponentBodyItem::Call(call) => ComponentBodyItem::Call(CallExpression {
            callee: call.callee.clone(),
            args: call.args.iter().map(|a| eval_to_literal(a, env, emits)).collect(),
        }),
        ComponentBodyItem::Child(child_ce) => {
            ComponentBodyItem::Child(subst_ce(child_ce, env, emits))
        }
        ComponentBodyItem::Positional(expr) => {
            // If the expression evaluates to a ComponentExpr (e.g. a variable holding an
            // imported CE), promote it to Child so it goes through the real child-spawn
            // path in the registry. Leaving it as Positional would cause it to reach
            // apply_positional which doesn't know how to spawn component subtrees.
            match eval_expr(expr, env, emits) {
                Ok(Value::ComponentExpr(ce)) => ComponentBodyItem::Child(*ce),
                Ok(val) => ComponentBodyItem::Positional(value_to_expr(val)),
                Err(_) => ComponentBodyItem::Positional(expr.clone()),
            }
        }
    }
}

fn eval_expr(
    expr: &Expression,
    env: &Env,
    emits: &mut Vec<IntentValue>,
) -> Result<Value, String> {
    match expr {
        Expression::Null => Ok(Value::Null),
        Expression::Bool(b) => Ok(Value::Bool(*b)),
        Expression::Number(n) => Ok(Value::Number(*n)),
        Expression::String(s) => Ok(Value::String(s.clone())),
        Expression::Array(items) => {
            let vals = items
                .iter()
                .map(|e| eval_expr(e, env, emits))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Value::Array(vals))
        }
        Expression::Identifier(id) => {
            // Look up in env; fall back to bare identifier value (for enum-like flags).
            match env.get(&id.0) {
                Some(val) => Ok(val.clone()),
                None => Ok(Value::Identifier(id.0.clone())),
            }
        }
        Expression::Component(ce) => Ok(Value::ComponentExpr(Box::new(subst_ce(ce, env, emits)))),
        Expression::Function { params, body } => Ok(Value::Function {
            params: params.iter().map(|p| p.0.clone()).collect(),
            body: body.clone(),
            captured_env: env.clone(),
        }),
        Expression::Call(call) => eval_call(call, env, emits),
        Expression::BinaryOp { op, lhs, rhs } => eval_binop(op, lhs, rhs, env, emits),
        Expression::UnaryOp { op, operand } => eval_unaryop(op, operand, env, emits),
    }
}

fn eval_call(
    call: &CallExpression,
    env: &Env,
    emits: &mut Vec<IntentValue>,
) -> Result<Value, String> {
    // Built-in: print(value)
    if call.callee.0 == "print" {
        let arg = call.args.first().map(|a| eval_expr(a, env, emits)).transpose()?
            .unwrap_or(Value::Null);
        println!("[mms] {}", value_display(&arg));
        return Ok(Value::Null);
    }

    // Built-in: assert(cond, msg)
    if call.callee.0 == "assert" {
        let cond = call.args.first().map(|a| eval_expr(a, env, emits)).transpose()?
            .unwrap_or(Value::Null);
        if !is_truthy(&cond) {
            let msg = call.args.get(1).map(|a| eval_expr(a, env, emits)).transpose()?
                .unwrap_or(Value::String("assertion failed".into()));
            return Err(format!("assert: {}", value_display(&msg)));
        }
        return Ok(Value::Null);
    }

    // Built-in: range(n) or range(start, end)
    if call.callee.0 == "range" {
        let args: Vec<Value> = call
            .args
            .iter()
            .map(|a| eval_expr(a, env, emits))
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

    let callee_val = match env.get(&call.callee.0) {
        Some(v) => v.clone(),
        None => return Err(format!("undefined: '{}'", call.callee.0)),
    };

    match callee_val {
        Value::Function { params, body, captured_env } => {
            let args: Vec<Value> = call
                .args
                .iter()
                .map(|a| eval_expr(a, env, emits))
                .collect::<Result<_, _>>()?;

            let mut call_env = captured_env;
            for (param, arg) in params.iter().zip(args.iter()) {
                call_env.insert(param.clone(), arg.clone());
            }

            let mut func_ctx = EvalContext { emits, source_path: None };
            match eval_block_stmts(&body.statements, &mut call_env, &mut func_ctx)? {
                StmtEffect::Return(val) => Ok(val),
                StmtEffect::None => Ok(Value::Null),
                StmtEffect::Break | StmtEffect::Continue => {
                    Err("break/continue cannot escape a function body".into())
                }
                StmtEffect::Bind(_, _) | StmtEffect::Export(_, _) | StmtEffect::ImportBindings(_) | StmtEffect::Reassign(_, _) => Ok(Value::Null),
            }
        }
        other => Err(format!("cannot call {:?} as a function", other)),
    }
}

fn eval_binop(
    op: &BinOpKind,
    lhs: &Expression,
    rhs: &Expression,
    env: &Env,
    emits: &mut Vec<IntentValue>,
) -> Result<Value, String> {
    // Short-circuit logical ops.
    match op {
        BinOpKind::And => {
            let l = eval_expr(lhs, env, emits)?;
            if !is_truthy(&l) {
                return Ok(Value::Bool(false));
            }
            let r = eval_expr(rhs, env, emits)?;
            return Ok(Value::Bool(is_truthy(&r)));
        }
        BinOpKind::Or => {
            let l = eval_expr(lhs, env, emits)?;
            if is_truthy(&l) {
                return Ok(Value::Bool(true));
            }
            let r = eval_expr(rhs, env, emits)?;
            return Ok(Value::Bool(is_truthy(&r)));
        }
        BinOpKind::Query => {
            // QueryDesugarTransform rewrites all `->` nodes into query()/query_all() calls
            // before eval runs. This arm is only reached if the transform missed one.
            return Err("query operator '->' not yet implemented (HostCall required)".to_string());
        }
        BinOpKind::Pipe => {
            let lhs_val = eval_expr(lhs, env, emits)?;
            let rhs_val = eval_expr(rhs, env, emits)?;
            match rhs_val {
                Value::Function { params, body, captured_env } => {
                    let mut call_env = captured_env;
                    if let Some(param) = params.first() {
                        call_env.insert(param.clone(), lhs_val);
                    }
                    let mut func_ctx = EvalContext { emits, source_path: None };
                    match eval_block_stmts(&body.statements, &mut call_env, &mut func_ctx)? {
                        StmtEffect::Return(val) => return Ok(val),
                        _ => return Ok(Value::Null),
                    }
                }
                other => return Err(format!("pipe: RHS must be a function, got {:?}", other)),
            }
        }
        _ => {}
    }

    let l = eval_expr(lhs, env, emits)?;
    let r = eval_expr(rhs, env, emits)?;

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
    }
}

fn eval_unaryop(
    op: &UnaryOpKind,
    operand: &Expression,
    env: &Env,
    emits: &mut Vec<IntentValue>,
) -> Result<Value, String> {
    let val = eval_expr(operand, env, emits)?;
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
        Value::ComponentObject(id) => format!("<component {:?}>", id),
        Value::ComponentExpr(_) => "<ce>".into(),
        Value::Object(_) => "<object>".into(),
        Value::Identifier(s) => s.clone(),
        Value::Module { .. } => "<module>".into(),
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

/// Evaluate a source file as a module (sandboxed — emits go to `sequence`, not the engine).
/// Returns `Value::Module { named, sequence }`.
fn eval_as_module(source: &str, source_path: Option<&str>) -> Result<Value, String> {
    let mut stmts = parse_source(source)?;
    EmitLiftTransform::apply(&mut stmts);
    QueryDesugarTransform::apply(&mut stmts);

    let mut local_env: Env = HashMap::new();
    let mut emits: Vec<IntentValue> = Vec::new();
    let mut named: HashMap<String, Value> = HashMap::new();
    let mut ctx = EvalContext { emits: &mut emits, source_path };

    for stmt in &stmts {
        match eval_stmt(stmt, &mut local_env, &mut ctx)? {
            StmtEffect::Bind(name, val) => { local_env.insert(name, val); }
            StmtEffect::Export(name, val) => {
                local_env.insert(name.clone(), val.clone());
                named.insert(name, val);
            }
            StmtEffect::ImportBindings(bindings) => {
                for (name, val) in bindings { local_env.insert(name, val); }
            }
            StmtEffect::None => {}
            StmtEffect::Reassign(name, val) => {
                if local_env.contains_key(&name) {
                    local_env.insert(name, val);
                }
            }
            StmtEffect::Return(_) | StmtEffect::Break | StmtEffect::Continue => {}
        }
    }

    let sequence: Vec<crate::meow_meow::ast::ComponentExpression> = emits
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
