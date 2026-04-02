/// AstTransform passes applied between parsing and evaluation.
use crate::meow_meow::ast::{BinOpKind, BlockStatement, CallExpression, Expression, Ident, IfStatement, Statement};

// ---------------------------------------------------------------------------
// EmitLiftTransform
// ---------------------------------------------------------------------------

/// Rewrites free-standing `ComponentExpression` statements into `emit(ce)` calls.
///
/// ```text
/// Statement::Expression(Expression::Component(ce))
///     →  Statement::Expression(Expression::Call { callee: "emit", args: [Component(ce)] })
/// ```
///
/// Applied recursively to all blocks (top-level, function bodies, if-branches, etc.).
/// No syntax or tokenizer changes — purely a structural AST rewrite.
pub struct EmitLiftTransform;

impl EmitLiftTransform {
    pub fn apply(stmts: &mut Vec<Statement>) {
        for stmt in stmts.iter_mut() {
            lift_stmt(stmt);
        }
    }
}

fn lift_stmt(stmt: &mut Statement) {
    match stmt {
        Statement::Expression(expr) => {
            if let Expression::Component(_) = expr {
                // Move the expression out, wrap in emit() call.
                let inner = std::mem::replace(expr, Expression::Null);
                *expr = Expression::Call(CallExpression {
                    callee: Ident("emit".into()),
                    args: vec![inner],
                });
            } else if let Expression::Function { body, .. } = expr {
                lift_block(body);
            }
        }
        Statement::Assignment(a) => {
            if let Expression::Function { body, .. } = &mut a.value {
                lift_block(body);
            }
        }
        Statement::Block(block) => lift_block(block),
        Statement::If(if_stmt) => lift_if(if_stmt),
        Statement::Return(_) => {}
        Statement::ForIn { body, .. } => lift_block(body),
        Statement::While { body, .. } => lift_block(body),
        Statement::Break | Statement::Continue => {}
        Statement::Import { .. } => {}
        Statement::Reassign { .. } => {}
    }
}

fn lift_block(block: &mut BlockStatement) {
    for stmt in block.statements.iter_mut() {
        lift_stmt(stmt);
    }
}

fn lift_if(if_stmt: &mut IfStatement) {
    lift_block(&mut if_stmt.then_branch);
    if let Some(else_branch) = &mut if_stmt.else_branch {
        lift_block(else_branch);
    }
}

// ---------------------------------------------------------------------------
// QueryDesugarTransform
// ---------------------------------------------------------------------------

/// Rewrites `|>` query sugar into explicit `query()`/`query_all()` calls.
///
/// The parser produces `BinOp(Pipe, lhs, rhs)` for all `|>` expressions without
/// inspecting operand types. This pass detects the query-sugar forms and rewrites them
/// before the evaluator runs, leaving only plain `expr |> fn_value` pipes for eval.
///
/// ## Rewrite rules (implemented)
///
/// ```text
/// "selector" |> handler
///     →  query("selector", handler)        // if selector matches #id pattern
///     →  query_all("selector", handler)    // otherwise
/// ```
///
/// ## Deferred (requires Expression::MethodCall)
///
/// ```text
/// "selector" |> method(args)
///     →  query("selector", fn(r) { r.method(args) })
///
/// scope |> "selector" |> handler
///     →  scope.query("selector", handler)
/// ```
///
/// Selector heuristic: starts with `#` and contains no spaces or combinators → single
/// (`query`); otherwise → multiple (`query_all`). Callers can override with the explicit
/// `query()`/`query_all()` function forms.
pub struct QueryDesugarTransform;

impl QueryDesugarTransform {
    pub fn apply(stmts: &mut Vec<Statement>) {
        for stmt in stmts.iter_mut() {
            qd_stmt(stmt);
        }
    }
}

fn selector_is_single(sel: &str) -> bool {
    sel.starts_with('#') && !sel.contains(' ') && !sel.contains('>')
}

fn qd_stmt(stmt: &mut Statement) {
    match stmt {
        Statement::Expression(expr) => qd_expr(expr),
        Statement::Assignment(a) => qd_expr(&mut a.value),
        Statement::Reassign { value, .. } => qd_expr(value),
        Statement::Return(ret) => {
            if let Some(expr) = &mut ret.value {
                qd_expr(expr);
            }
        }
        Statement::If(if_stmt) => {
            qd_expr(&mut if_stmt.condition);
            qd_block(&mut if_stmt.then_branch);
            if let Some(else_branch) = &mut if_stmt.else_branch {
                qd_block(else_branch);
            }
        }
        Statement::Block(block) => qd_block(block),
        Statement::ForIn { iterable, body, .. } => {
            qd_expr(iterable);
            qd_block(body);
        }
        Statement::While { condition, body } => {
            qd_expr(condition);
            qd_block(body);
        }
        Statement::Break | Statement::Continue | Statement::Import { .. } => {}
    }
}

fn qd_block(block: &mut BlockStatement) {
    for stmt in block.statements.iter_mut() {
        qd_stmt(stmt);
    }
}

fn qd_expr(expr: &mut Expression) {
    // Bottom-up: recurse into children first, then rewrite this node.
    match expr {
        Expression::BinaryOp { lhs, rhs, .. } => {
            qd_expr(lhs);
            qd_expr(rhs);
        }
        Expression::UnaryOp { operand, .. } => qd_expr(operand),
        Expression::Call(call) => {
            for arg in call.args.iter_mut() {
                qd_expr(arg);
            }
        }
        Expression::Array(items) => {
            for item in items.iter_mut() {
                qd_expr(item);
            }
        }
        Expression::Function { body, .. } => qd_block(body),
        _ => {}
    }

    // Now rewrite this node if it's a query-sugar pipe.
    if let Expression::BinaryOp { op: BinOpKind::Pipe, lhs, rhs } = expr {
        if let Expression::String(sel) = lhs.as_ref() {
            let callee = if selector_is_single(sel) { "query" } else { "query_all" };
            let sel_expr = Expression::String(sel.clone());
            let handler = std::mem::replace(rhs.as_mut(), Expression::Null);
            *expr = Expression::Call(CallExpression {
                callee: Ident(callee.into()),
                args: vec![sel_expr, handler],
            });
        }
    }
}
