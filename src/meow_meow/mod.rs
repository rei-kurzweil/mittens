pub mod ast;
pub mod component_registry;
pub mod evaluator;
pub mod object;
pub mod parser;
pub mod token;
pub mod tokenizer;
pub mod transform;

pub use ast::{expression::{
    CallExpression, ComponentBodyItem, ComponentExpression, ConstructorCall, Expression, Ident, Span,
}, statement::*};
pub use evaluator::*;
pub use object::*;
pub use parser::*;
pub use token::*;
pub use tokenizer::*;

#[cfg(test)]
mod tests;
