/// AstTransform passes applied between parsing and evaluation.
use crate::meow_meow::ast::{BlockStatement, CallExpression, Expression, Ident, IfStatement, Statement};

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
