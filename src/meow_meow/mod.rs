pub mod ast;
pub mod block_effect_analyzer;
pub mod component_registry;
pub mod evaluator;
pub mod lowering;
pub mod object;
pub mod parser;
pub mod runner;
pub mod token;
pub mod tokenizer;
pub mod transform;
pub mod unparser;

pub use ast::{
    AssignmentStatement, BinOpKind, BlockStatement, CallExpression, ComponentExpression,
    ConstructorCall, Expression, Ident, IfStatement, ReturnStatement, Span, Statement, UnaryOpKind,
};
pub use block_effect_analyzer::*;
pub use evaluator::*;
pub use lowering::*;
pub use object::*;
pub use parser::*;
pub use runner::*;
pub use token::*;
pub use tokenizer::*;

#[cfg(test)]
mod block_effect_analyzer_tests;
#[cfg(test)]
mod tests;
