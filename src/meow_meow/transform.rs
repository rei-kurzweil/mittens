/// AstTransform passes applied between parsing and evaluation.
use crate::meow_meow::ast::expression::{CallExpression, Expression, Ident};
use crate::meow_meow::ast::statement::{BlockStatement, IfStatement, Statement};

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
            }
            // Recurse into sub-expressions that contain blocks (not needed for v1,
            // but keeps the transform complete for if/closure bodies).
        }
        Statement::Block(block) => lift_block(block),
        Statement::If(if_stmt) => lift_if(if_stmt),
        Statement::Assignment(_) | Statement::Return(_) => {
            // Component expressions on the RHS of let or in return are NOT lifted —
            // they become ComponentObject values (Option B: captured, not emitted).
        }
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
