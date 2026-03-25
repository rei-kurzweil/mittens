use std::collections::HashMap;
use std::thread::{self, JoinHandle};

use rtrb::{Consumer, Producer, RingBuffer};

use crate::engine::ecs::IntentValue;
use crate::meow_meow::ast::{
    BinOpKind, CallExpression, Expression, Ident, IfStatement, Statement,
    UnaryOpKind,
};
use crate::meow_meow::object::Value;
use crate::meow_meow::parser::{MeowMeowParser, ParseError};
use crate::meow_meow::token::TokenizeError;
use crate::meow_meow::tokenizer::MeowMeowTokenizer;
use crate::meow_meow::transform::EmitLiftTransform;

// ---------------------------------------------------------------------------
// Thread protocol
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum EvalRequest {
    /// Parse and evaluate a script. Emitted `SpawnComponentTree` intents come back
    /// as `EvalResponse::Intent` messages.
    EvalScript { source: String },
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
            Ok(EvalRequest::EvalScript { source }) => {
                eval_script(&source, &mut responses);
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

/// Evaluate a script: parse → EmitLiftTransform → walk statements.
/// Each `emit(ce)` call produces an `EvalResponse::Intent(SpawnComponentTree)`.
fn eval_script(source: &str, responses: &mut Producer<EvalResponse>) {
    let mut stmts = match parse_source(source) {
        Ok(s) => s,
        Err(msg) => {
            let _ = responses.push(EvalResponse::Error { message: msg });
            return;
        }
    };

    EmitLiftTransform::apply(&mut stmts);

    let env: Env = HashMap::new();
    let mut emits: Vec<IntentValue> = Vec::new();

    match eval_block_stmts(&stmts, &env, &mut emits) {
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
    Return(Value),
}

/// Evaluate a block of statements in a local scope (cloned from parent env).
/// Returns `Some(value)` if a `return` statement was hit, `None` otherwise.
/// Emits are pushed directly to `emits`.
fn eval_block_stmts(
    stmts: &[Statement],
    env: &Env,
    emits: &mut Vec<IntentValue>,
) -> Result<Option<Value>, String> {
    let mut local_env = env.clone();
    for stmt in stmts {
        match eval_stmt(stmt, &local_env, emits)? {
            StmtEffect::Bind(name, val) => {
                local_env.insert(name, val);
            }
            StmtEffect::None => {}
            StmtEffect::Return(val) => return Ok(Some(val)),
        }
    }
    Ok(None)
}

fn eval_stmt(
    stmt: &Statement,
    env: &Env,
    emits: &mut Vec<IntentValue>,
) -> Result<StmtEffect, String> {
    match stmt {
        Statement::Assignment(a) => {
            let val = eval_expr(&a.value, env, emits)?;
            Ok(StmtEffect::Bind(a.name.0.clone(), val))
        }
        Statement::Expression(expr) => {
            eval_expr_stmt(expr, env, emits)?;
            Ok(StmtEffect::None)
        }
        Statement::Return(r) => {
            let val = match &r.value {
                Some(expr) => eval_expr(expr, env, emits)?,
                None => Value::Null,
            };
            Ok(StmtEffect::Return(val))
        }
        Statement::If(if_stmt) => eval_if(if_stmt, env, emits),
        Statement::Block(block) => {
            match eval_block_stmts(&block.statements, env, emits)? {
                Some(val) => Ok(StmtEffect::Return(val)),
                None => Ok(StmtEffect::None),
            }
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
    env: &Env,
    emits: &mut Vec<IntentValue>,
) -> Result<StmtEffect, String> {
    let cond = eval_expr(&if_stmt.condition, env, emits)?;
    if is_truthy(&cond) {
        match eval_block_stmts(&if_stmt.then_branch.statements, env, emits)? {
            Some(val) => Ok(StmtEffect::Return(val)),
            None => Ok(StmtEffect::None),
        }
    } else if let Some(else_branch) = &if_stmt.else_branch {
        match eval_block_stmts(&else_branch.statements, env, emits)? {
            Some(val) => Ok(StmtEffect::Return(val)),
            None => Ok(StmtEffect::None),
        }
    } else {
        Ok(StmtEffect::None)
    }
}

// ---------------------------------------------------------------------------
// Expression evaluation
// ---------------------------------------------------------------------------

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
        Expression::Component(ce) => Ok(Value::ComponentExpr(Box::new(ce.clone()))),
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

            match eval_block_stmts(&body.statements, &call_env, emits)? {
                Some(val) => Ok(val),
                None => Ok(Value::Null),
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
        _ => {}
    }

    let l = eval_expr(lhs, env, emits)?;
    let r = eval_expr(rhs, env, emits)?;

    match op {
        BinOpKind::Add => match (l, r) {
            (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
            (Value::String(a), Value::String(b)) => Ok(Value::String(a + &b)),
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
        BinOpKind::And | BinOpKind::Or => unreachable!("handled above"),
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

fn parse_source(source: &str) -> Result<Vec<Statement>, String> {
    let tokens = MeowMeowTokenizer::new(source)
        .tokenize()
        .map_err(tokenize_err_to_string)?;
    MeowMeowParser::new(tokens)
        .parse_program()
        .map_err(parse_err_to_string)
}

fn parse_only(source: &str) -> Result<String, String> {
    let stmts = parse_source(source)?;
    Ok(format!("{stmts:#?}"))
}

fn tokenize_err_to_string(e: TokenizeError) -> String {
    format!("tokenize error at {}..{}: {}", e.span.start, e.span.end, e.message)
}

fn parse_err_to_string(e: ParseError) -> String {
    format!("parse error near token #{}: {}", e.token_index, e.message)
}
