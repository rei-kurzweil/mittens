use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryParseError {
    pub message: String,
    pub position: usize,
}

impl QueryParseError {
    pub fn new(message: impl Into<String>, position: usize) -> Self {
        Self {
            message: message.into(),
            position,
        }
    }
}

impl fmt::Display for QueryParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at byte {}", self.message, self.position)
    }
}

impl std::error::Error for QueryParseError {}
