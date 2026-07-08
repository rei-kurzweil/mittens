use crate::meow_meow::ast::{BinOpKind, BlockStatement, Expression, Statement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectKind {
    None,
    Audio,
    Visual,
    Mixed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatementEffectSummary {
    pub effect_kind: EffectKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockEffectAnalysis {
    pub contains_audio_effects: bool,
    pub contains_visual_effects: bool,
    pub contains_unknown_effects: bool,
    pub statements: Vec<StatementEffectSummary>,
}

pub struct BlockEffectAnalyzer;

impl BlockEffectAnalyzer {
    pub fn analyze_keyframe_block(body: &BlockStatement) -> BlockEffectAnalysis {
        let statements: Vec<StatementEffectSummary> = body
            .statements
            .iter()
            .map(|stmt| StatementEffectSummary {
                effect_kind: classify_statement(stmt),
            })
            .collect();

        BlockEffectAnalysis {
            contains_audio_effects: statements
                .iter()
                .any(|s| matches!(s.effect_kind, EffectKind::Audio | EffectKind::Mixed)),
            contains_visual_effects: statements
                .iter()
                .any(|s| matches!(s.effect_kind, EffectKind::Visual | EffectKind::Mixed)),
            contains_unknown_effects: statements
                .iter()
                .any(|s| matches!(s.effect_kind, EffectKind::Unknown)),
            statements,
        }
    }
}

fn classify_statement(stmt: &Statement) -> EffectKind {
    match stmt {
        Statement::Expression(expr) => classify_expr(expr),
        Statement::Block(block) => summarize(block.statements.iter().map(classify_statement)),
        Statement::If(if_stmt) => {
            let mut effects = vec![summarize(
                if_stmt
                    .then_branch
                    .statements
                    .iter()
                    .map(classify_statement),
            )];
            if let Some(else_branch) = &if_stmt.else_branch {
                let effect = match else_branch {
                    crate::meow_meow::ast::ElseBranch::Block(block) => {
                        summarize(block.statements.iter().map(classify_statement))
                    }
                    crate::meow_meow::ast::ElseBranch::If(next_if) => {
                        classify_statement(&Statement::If((**next_if).clone()))
                    }
                };
                effects.push(effect);
            }
            summarize(effects)
        }
        Statement::ForIn { body, .. } | Statement::While { body, .. } => {
            summarize(body.statements.iter().map(classify_statement))
        }
        Statement::Assignment(_)
        | Statement::Reassign { .. }
        | Statement::Return(_)
        | Statement::Break
        | Statement::Continue
        | Statement::Import { .. } => EffectKind::None,
    }
}

fn classify_expr(expr: &Expression) -> EffectKind {
    match expr {
        Expression::Call(call) => match call.callee.as_ref() {
            Expression::BinaryOp {
                op: BinOpKind::Dot,
                lhs,
                rhs,
            } => {
                if matches!(lhs.as_ref(), Expression::Identifier(id) if id.0 == "MusicNote")
                    && matches!(rhs.as_ref(), Expression::Identifier(id) if matches!(id.0.as_str(), "a" | "b" | "c" | "d" | "e" | "f" | "g"))
                {
                    EffectKind::Audio
                } else {
                    EffectKind::Unknown
                }
            }
            _ => EffectKind::Unknown,
        },
        _ => EffectKind::None,
    }
}

fn summarize(effects: impl IntoIterator<Item = EffectKind>) -> EffectKind {
    let mut saw_audio = false;
    let mut saw_visual = false;
    let mut saw_unknown = false;

    for effect in effects {
        match effect {
            EffectKind::None => {}
            EffectKind::Audio => saw_audio = true,
            EffectKind::Visual => saw_visual = true,
            EffectKind::Mixed => {
                saw_audio = true;
                saw_visual = true;
            }
            EffectKind::Unknown => saw_unknown = true,
        }
    }

    if saw_unknown {
        EffectKind::Unknown
    } else if saw_audio && saw_visual {
        EffectKind::Mixed
    } else if saw_audio {
        EffectKind::Audio
    } else if saw_visual {
        EffectKind::Visual
    } else {
        EffectKind::None
    }
}
