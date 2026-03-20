use std::collections::HashMap;
use std::thread::{self, JoinHandle};

use rtrb::{Consumer, Producer, RingBuffer};

use crate::engine::ecs::IntentValue;
use crate::meow_meow::ast::expression::{Expression, Ident};
use crate::meow_meow::ast::statement::Statement;
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

    let mut env: HashMap<String, StoredValue> = HashMap::new();

    for stmt in &stmts {
        match eval_stmt(stmt, &env) {
            Ok(StmtEffect::Emit(intent)) => {
                // Keep pushing until there's space (simple back-pressure).
                while responses.push(EvalResponse::Intent(intent.clone())).is_err() {
                    std::thread::yield_now();
                }
            }
            Ok(StmtEffect::Bind(name, val)) => {
                env.insert(name, val);
            }
            Ok(StmtEffect::None) => {}
            Err(msg) => {
                let _ = responses.push(EvalResponse::Error { message: msg });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Stored value (ObjectWorld v1 — flat env, no heap)
// ---------------------------------------------------------------------------

/// A value stored in the evaluator's flat variable environment.
/// In v1, the only interesting case is a captured ComponentExpression.
#[derive(Debug, Clone)]
enum StoredValue {
    /// A ComponentExpression captured via `let x = T { }`.
    /// Not yet live in the engine — becomes a SpawnComponentTree intent when emitted.
    ComponentExpr(Box<crate::meow_meow::ast::expression::ComponentExpression>),
    /// Primitive (numbers, strings, bools — not yet used, but reserved for future eval).
    #[allow(dead_code)]
    Primitive(String),
}

// ---------------------------------------------------------------------------
// Statement evaluation
// ---------------------------------------------------------------------------

enum StmtEffect {
    None,
    Emit(IntentValue),
    Bind(String, StoredValue),
}

fn eval_stmt(stmt: &Statement, env: &HashMap<String, StoredValue>) -> Result<StmtEffect, String> {
    match stmt {
        Statement::Expression(expr) => eval_expr_stmt(expr, env),
        Statement::Assignment(a) => {
            let val = capture_expr(&a.value)?;
            Ok(StmtEffect::Bind(a.name.0.clone(), val))
        }
        Statement::Return(_) | Statement::If(_) | Statement::Block(_) => {
            // Not yet evaluated — ignored in v1 top-level scripts.
            Ok(StmtEffect::None)
        }
    }
}

/// Evaluate an expression in statement position.
/// After `EmitLiftTransform`, any bare `ComponentExpression` has been wrapped as
/// `Call("emit", [ce])`. We handle that call here.
/// Option B: any expression that resolves to a ComponentExpr also emits.
fn eval_expr_stmt(
    expr: &Expression,
    env: &HashMap<String, StoredValue>,
) -> Result<StmtEffect, String> {
    match expr {
        // emit(ce) — produced by EmitLiftTransform or written explicitly
        Expression::Call(call) if call.callee == Ident("emit".into()) => {
            if let Some(arg) = call.args.first() {
                return emit_expr(arg, env);
            }
            Ok(StmtEffect::None)
        }

        // Bare identifier — Option B: emit if it holds a ComponentExpr
        Expression::Identifier(id) => {
            if let Some(StoredValue::ComponentExpr(ce)) = env.get(&id.0) {
                return Ok(StmtEffect::Emit(IntentValue::SpawnComponentTree {
                    root: ce.clone(),
                    parent: None,
                }));
            }
            Ok(StmtEffect::None)
        }

        // Any other call result — ignored in v1
        _ => Ok(StmtEffect::None),
    }
}

fn emit_expr(
    expr: &Expression,
    env: &HashMap<String, StoredValue>,
) -> Result<StmtEffect, String> {
    match expr {
        Expression::Component(ce) => Ok(StmtEffect::Emit(IntentValue::SpawnComponentTree {
            root: Box::new(ce.clone()),
            parent: None,
        })),
        Expression::Identifier(id) => {
            if let Some(StoredValue::ComponentExpr(ce)) = env.get(&id.0) {
                Ok(StmtEffect::Emit(IntentValue::SpawnComponentTree {
                    root: ce.clone(),
                    parent: None,
                }))
            } else {
                Err(format!("emit: '{}' is not a ComponentExpression", id.0))
            }
        }
        other => Err(format!("emit: cannot emit {other:?}")),
    }
}

/// Capture an expression as a `StoredValue` for a `let` binding.
fn capture_expr(expr: &Expression) -> Result<StoredValue, String> {
    match expr {
        Expression::Component(ce) => Ok(StoredValue::ComponentExpr(Box::new(ce.clone()))),
        Expression::Number(n) => Ok(StoredValue::Primitive(n.to_string())),
        Expression::String(s) => Ok(StoredValue::Primitive(s.clone())),
        Expression::Bool(b) => Ok(StoredValue::Primitive(b.to_string())),
        Expression::Null => Ok(StoredValue::Primitive("null".into())),
        other => Err(format!("let binding: cannot capture {other:?}")),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
