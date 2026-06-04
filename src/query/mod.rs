pub mod ast;
pub mod css;
pub mod error;
pub mod evaluator;
pub mod mmq;

use std::sync::Arc;

pub use ast::{
    AttributeSelector, Combinator, CompoundSelector, QueryAst, SelectorSegment, SelectorSequence,
    SimpleSelector,
};
pub use error::QueryParseError;
pub use evaluator::{QueryEvaluator, QueryTreeAdapter};

/// A query parser. Implementors typically own a per-instance AST cache so
/// repeated parses of the same string are amortized to a single
/// `Arc::clone`. The cache is the parser's responsibility — callers just
/// hand the same instance the same string.
pub trait QuerySyntax {
    fn parse(&mut self, input: &str) -> Result<Arc<QueryAst>, QueryParseError>;
}
