pub mod ast;
pub mod css;
pub mod error;
pub mod evaluator;
pub mod mmq;

pub use ast::{
    AttributeSelector, Combinator, CompoundSelector, QueryAst, SelectorSegment,
    SelectorSequence, SimpleSelector,
};
pub use error::QueryParseError;
pub use evaluator::{QueryEvaluator, QueryTreeAdapter};

pub trait QuerySyntax {
    fn parse(input: &str) -> Result<QueryAst, QueryParseError>;
}