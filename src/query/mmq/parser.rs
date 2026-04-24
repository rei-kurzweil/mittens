use crate::query::{QueryAst, QueryParseError, QuerySyntax};

pub struct MmqQuerySyntax;

impl QuerySyntax for MmqQuerySyntax {
    fn parse(_input: &str) -> Result<QueryAst, QueryParseError> {
        Err(QueryParseError::new(
            "MMQ parser not implemented yet; use CSS query syntax for now",
            0,
        ))
    }
}