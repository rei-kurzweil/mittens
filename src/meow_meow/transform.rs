/// AstTransform passes applied between parsing and evaluation.
use crate::meow_meow::ast::{
    BinOpKind, BlockStatement, CallExpression, ElseBranch, Expression, Ident, IfStatement,
    Statement,
};

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
                    callee: Box::new(Expression::Identifier(Ident("emit".into()))),
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
        lift_else_branch(else_branch);
    }
}

fn lift_else_branch(else_branch: &mut ElseBranch) {
    match else_branch {
        ElseBranch::Block(block) => lift_block(block),
        ElseBranch::If(if_stmt) => lift_if(if_stmt),
    }
}

// ---------------------------------------------------------------------------
// QueryDesugarTransform
// ---------------------------------------------------------------------------

/// Rewrites `->` query/dispatch sugar into explicit `query()`/`query_all()` calls.
///
/// The parser produces `BinOp(Query, lhs, rhs)` for all `->` expressions without
/// interpreting selector/query semantics. This pass rewrites query/dispatch forms
/// before the evaluator runs, leaving only plain `expr |> fn_value` pipes for eval.
///
/// ## Rewrite rules (implemented)
///
/// ```text
/// "selector" -> handler
///     →  query("selector", handler)        // if selector matches #id pattern
///     →  query_all("selector", handler)    // otherwise
/// ```
///
/// ## Deferred (pending Query HostCall + small extra rewrite rules)
///
/// ```text
/// "selector" -> method(args)
///     →  query("selector", fn(r) { r.method(args) })
///
/// scope -> "selector" -> handler
///     →  scope.query("selector", handler)
/// ```
///
/// These do **not** need a dedicated `Expression::MethodCall` AST node — methods
/// are already represented as `CallExpression` with `callee = BinOp(Dot, ..)`.
/// They need: (1) a Query HostCall on the host, (2) detection of `Call`-shaped
/// rhs in the existing `BinOp(Query, ..)` rewrite. See
/// `docs/meow_meow/task/query-engine-and-mms-query-host-calls.md`.
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
        Statement::If(if_stmt) => qd_if(if_stmt),
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

fn qd_if(if_stmt: &mut IfStatement) {
    qd_expr(&mut if_stmt.condition);
    qd_block(&mut if_stmt.then_branch);
    if let Some(else_branch) = &mut if_stmt.else_branch {
        qd_else_branch(else_branch);
    }
}

fn qd_else_branch(else_branch: &mut ElseBranch) {
    match else_branch {
        ElseBranch::Block(block) => qd_block(block),
        ElseBranch::If(if_stmt) => qd_if(if_stmt),
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
        Expression::Index { base, index } => {
            qd_expr(base);
            qd_expr(index);
        }
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

    // Now rewrite this node if it's a query/dispatch expression.
    if let Expression::BinaryOp {
        op: BinOpKind::Query,
        lhs,
        rhs,
    } = expr
    {
        let callee = if let Expression::String(sel) = lhs.as_ref() {
            if selector_is_single(sel) {
                "query"
            } else {
                "query_all"
            }
        } else {
            "query_all" // computed selector — conservative fallback
        };
        let sel_expr = std::mem::replace(lhs.as_mut(), Expression::Null);
        let handler_or_call = std::mem::replace(rhs.as_mut(), Expression::Null);
        // If the rhs is a method-shorthand call (e.g. `set_color(0,1,0,1)`),
        // wrap it as `fn(__qr) { __qr.method(args) }` so the receiver is the
        // implicit query result. We only do this when the call's callee is a
        // bare identifier — anything more complex is treated as a regular
        // expression handler.
        let handler = match handler_or_call {
            Expression::Call(CallExpression { callee, args })
                if matches!(callee.as_ref(), Expression::Identifier(_)) =>
            {
                let method = match *callee {
                    Expression::Identifier(id) => id,
                    _ => unreachable!(),
                };
                wrap_method_shorthand(method, args)
            }
            other => other,
        };
        *expr = Expression::Call(CallExpression {
            callee: Box::new(Expression::Identifier(Ident(callee.into()))),
            args: vec![sel_expr, handler],
        });
    }
}

/// Build `fn(__qr) { __qr.method(args) }` for `"sel" -> method(args)` sugar.
fn wrap_method_shorthand(method: Ident, args: Vec<Expression>) -> Expression {
    let receiver = Ident("__qr".into());
    let receiver_ref = Expression::Identifier(receiver.clone());
    let dot = Expression::BinaryOp {
        op: BinOpKind::Dot,
        lhs: Box::new(receiver_ref),
        rhs: Box::new(Expression::Identifier(method)),
    };
    let call = Expression::Call(CallExpression {
        callee: Box::new(dot),
        args,
    });
    Expression::Function {
        params: vec![receiver],
        body: BlockStatement {
            statements: vec![Statement::Expression(call)],
        },
    }
}
