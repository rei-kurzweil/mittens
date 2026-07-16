//! MMQ — Meow Meow Query selector syntax.
//!
//! MMQ MVP grammar:
//!
//! ```text
//! query    := sequence ("," sequence)*
//! sequence := compound (combinator compound)*
//! combinator := whitespace | ">"
//! compound := simple+
//! simple   := "*" | "#" ident | type_ident | "[" attr "]"
//! attr     := ident ("=" (quoted_string | ident_or_number))?
//! ```
//!
//! Differences from CSS syntax:
//! - `#name` selects by component **label** (`SimpleSelector::Name`), not by Id.
//!   The engine's component graph has no separate id concept distinct from label.
//! - No class selector (`.foo`) in MVP — the engine has no class concept.
//!
//! Example: `T#hero`, `#left_hand`, `Renderable#bg`, `T > R`, `#root T`.

use std::collections::HashMap;
use std::sync::Arc;

use crate::ast::{
    AttributeSelector, Combinator, CompoundSelector, QueryAst, SelectorSegment, SelectorSequence,
    SimpleSelector,
};
use crate::{QueryParseError, QuerySyntax};

/// MMQ parser with per-instance AST cache.
///
/// Repeated `parse(s)` calls with the same `s` are amortized to one
/// `Arc::clone` after the first. ASTs are pure functions of the input —
/// the cache never goes stale; it grows monotonically.
#[derive(Default)]
pub struct MmqQuerySyntax {
    cache: HashMap<String, Arc<QueryAst>>,
}

impl MmqQuerySyntax {
    pub fn new() -> Self {
        Self::default()
    }
}

impl QuerySyntax for MmqQuerySyntax {
    fn parse(&mut self, input: &str) -> Result<Arc<QueryAst>, QueryParseError> {
        if let Some(ast) = self.cache.get(input) {
            return Ok(ast.clone());
        }
        let ast = Arc::new(Parser::new(input).parse_query()?);
        self.cache.insert(input.to_string(), ast.clone());
        Ok(ast)
    }
}

struct Parser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn parse_query(&mut self) -> Result<QueryAst, QueryParseError> {
        let mut selector_groups = Vec::new();

        loop {
            self.skip_whitespace();
            if self.is_eof() {
                break;
            }

            selector_groups.push(self.parse_selector_sequence()?);
            self.skip_whitespace();

            if self.peek_char() == Some(',') {
                self.bump_char();
                continue;
            }

            break;
        }

        if selector_groups.is_empty() {
            return Err(self.err("empty query"));
        }

        Ok(QueryAst { selector_groups })
    }

    fn parse_selector_sequence(&mut self) -> Result<SelectorSequence, QueryParseError> {
        let mut segments = Vec::new();
        let first = self.parse_compound_selector()?;
        segments.push(SelectorSegment {
            combinator: None,
            compound: first,
        });

        loop {
            let saw_ws = self.skip_whitespace();
            let combinator = match self.peek_char() {
                Some('>') => {
                    self.bump_char();
                    self.skip_whitespace();
                    Some(Combinator::Child)
                }
                Some(',') | None => break,
                _ if saw_ws => Some(Combinator::Descendant),
                _ => None,
            };

            let Some(combinator) = combinator else {
                break;
            };

            let compound = self.parse_compound_selector()?;
            segments.push(SelectorSegment {
                combinator: Some(combinator),
                compound,
            });
        }

        Ok(SelectorSequence { segments })
    }

    fn parse_compound_selector(&mut self) -> Result<CompoundSelector, QueryParseError> {
        let mut simple_selectors = Vec::new();

        loop {
            match self.peek_char() {
                Some('*') => {
                    self.bump_char();
                    simple_selectors.push(SimpleSelector::Universal);
                }
                Some('#') => {
                    self.bump_char();
                    let ident = self.parse_identifier()?;
                    simple_selectors.push(SimpleSelector::Name(ident));
                }
                Some('@') => {
                    self.bump_char();
                    // Currently only `@uuid:<hex+hyphens>` is recognized.
                    let scheme = self.parse_identifier()?;
                    if scheme != "uuid" {
                        return Err(self.err(format!(
                            "unknown @-selector scheme '{}', expected 'uuid'",
                            scheme
                        )));
                    }
                    self.expect_char(':')?;
                    let guid = self.parse_guid_literal()?;
                    simple_selectors.push(SimpleSelector::Guid(guid));
                }
                Some('[') => {
                    simple_selectors
                        .push(SimpleSelector::Attribute(self.parse_attribute_selector()?));
                }
                Some(ch) if is_ident_start(ch) => {
                    let ident = self.parse_identifier()?;
                    simple_selectors.push(SimpleSelector::Type(ident));
                }
                _ => break,
            }
        }

        if simple_selectors.is_empty() {
            return Err(self.err("expected selector"));
        }

        Ok(CompoundSelector { simple_selectors })
    }

    fn parse_attribute_selector(&mut self) -> Result<AttributeSelector, QueryParseError> {
        self.expect_char('[')?;
        self.skip_whitespace();
        let name = self.parse_identifier()?;
        self.skip_whitespace();

        let value = if self.peek_char() == Some('=') {
            self.bump_char();
            self.skip_whitespace();
            Some(self.parse_attribute_value()?)
        } else {
            None
        };

        self.skip_whitespace();
        self.expect_char(']')?;

        Ok(AttributeSelector { name, value })
    }

    fn parse_attribute_value(&mut self) -> Result<String, QueryParseError> {
        match self.peek_char() {
            Some('\'') | Some('"') => self.parse_quoted_string(),
            Some(ch) if is_ident_start(ch) || ch.is_ascii_digit() => {
                self.parse_identifier_or_number()
            }
            _ => Err(self.err("expected attribute value")),
        }
    }

    fn parse_quoted_string(&mut self) -> Result<String, QueryParseError> {
        let quote = self
            .bump_char()
            .ok_or_else(|| self.err("expected string quote"))?;
        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch == quote {
                let value = self.input[start..self.pos].to_string();
                self.bump_char();
                return Ok(value);
            }
            self.bump_char();
        }
        Err(self.err("unterminated string"))
    }

    fn parse_identifier_or_number(&mut self) -> Result<String, QueryParseError> {
        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if is_ident_continue(ch) || ch.is_ascii_digit() {
                self.bump_char();
            } else {
                break;
            }
        }

        if start == self.pos {
            return Err(self.err("expected identifier"));
        }

        Ok(self.input[start..self.pos].to_string())
    }

    fn parse_guid_literal(&mut self) -> Result<String, QueryParseError> {
        let start = self.pos;
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_hexdigit() || ch == '-' {
                self.bump_char();
            } else {
                break;
            }
        }
        if start == self.pos {
            return Err(self.err("expected guid literal"));
        }
        Ok(self.input[start..self.pos].to_string())
    }

    fn parse_identifier(&mut self) -> Result<String, QueryParseError> {
        let Some(ch) = self.peek_char() else {
            return Err(self.err("expected identifier"));
        };
        if !is_ident_start(ch) {
            return Err(self.err("expected identifier"));
        }

        let start = self.pos;
        self.bump_char();
        while let Some(next) = self.peek_char() {
            if is_ident_continue(next) {
                self.bump_char();
            } else {
                break;
            }
        }

        Ok(self.input[start..self.pos].to_string())
    }

    fn expect_char(&mut self, expected: char) -> Result<(), QueryParseError> {
        match self.bump_char() {
            Some(ch) if ch == expected => Ok(()),
            _ => Err(self.err(format!("expected '{}'", expected))),
        }
    }

    fn skip_whitespace(&mut self) -> bool {
        let start = self.pos;
        while matches!(self.peek_char(), Some(ch) if ch.is_whitespace()) {
            self.bump_char();
        }
        self.pos > start
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn bump_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn err(&self, message: impl Into<String>) -> QueryParseError {
        QueryParseError::new(message, self.pos)
    }
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || ch == '-'
}

fn is_ident_continue(ch: char) -> bool {
    is_ident_start(ch) || ch.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::MmqQuerySyntax;
    use crate::QuerySyntax;
    use crate::ast::{Combinator, SimpleSelector};
    use std::sync::Arc;

    #[test]
    fn parses_name_selector() {
        let mut p = MmqQuerySyntax::new();
        let ast = p.parse("#hero").expect("parse");
        let seg = &ast.selector_groups[0].segments[0];
        assert_eq!(seg.combinator, None);
        match &seg.compound.simple_selectors[0] {
            SimpleSelector::Name(n) => assert_eq!(n, "hero"),
            other => panic!("expected name, got {:?}", other),
        }
    }

    #[test]
    fn parses_type_selector() {
        let mut p = MmqQuerySyntax::new();
        let ast = p.parse("Transform").expect("parse");
        match &ast.selector_groups[0].segments[0].compound.simple_selectors[0] {
            SimpleSelector::Type(t) => assert_eq!(t, "Transform"),
            other => panic!("expected type, got {:?}", other),
        }
    }

    #[test]
    fn parses_type_hash_name_compound() {
        let mut p = MmqQuerySyntax::new();
        let ast = p.parse("T#hero").expect("parse");
        let simples = &ast.selector_groups[0].segments[0].compound.simple_selectors;
        assert_eq!(simples.len(), 2);
        assert!(matches!(&simples[0], SimpleSelector::Type(t) if t == "T"));
        assert!(matches!(&simples[1], SimpleSelector::Name(n) if n == "hero"));
    }

    #[test]
    fn parses_attribute_name_selector_back_compat() {
        let mut p = MmqQuerySyntax::new();
        let ast = p.parse("[name='LeftHand']").expect("parse");
        match &ast.selector_groups[0].segments[0].compound.simple_selectors[0] {
            SimpleSelector::Attribute(a) => {
                assert_eq!(a.name, "name");
                assert_eq!(a.value.as_deref(), Some("LeftHand"));
            }
            other => panic!("expected attribute, got {:?}", other),
        }
    }

    #[test]
    fn parses_descendant_and_child_combinators() {
        let mut p = MmqQuerySyntax::new();
        let ast = p.parse("#root > T C").expect("parse");
        let segs = &ast.selector_groups[0].segments;
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0].combinator, None);
        assert_eq!(segs[1].combinator, Some(Combinator::Child));
        assert_eq!(segs[2].combinator, Some(Combinator::Descendant));
    }

    #[test]
    fn parses_multi_selector_groups() {
        let mut p = MmqQuerySyntax::new();
        let ast = p.parse("T, R").expect("parse");
        assert_eq!(ast.selector_groups.len(), 2);
    }

    #[test]
    fn parses_guid_selector() {
        let mut p = MmqQuerySyntax::new();
        let ast = p
            .parse("@uuid:8c4f3e72-1234-5678-9abc-def012345678")
            .expect("parse");
        match &ast.selector_groups[0].segments[0].compound.simple_selectors[0] {
            SimpleSelector::Guid(g) => {
                assert_eq!(g, "8c4f3e72-1234-5678-9abc-def012345678")
            }
            other => panic!("expected guid, got {:?}", other),
        }
    }

    #[test]
    fn guid_selector_rejects_unknown_scheme() {
        let mut p = MmqQuerySyntax::new();
        assert!(p.parse("@oid:1234").is_err());
    }

    #[test]
    fn cache_returns_same_arc_for_same_input() {
        let mut p = MmqQuerySyntax::new();
        let a = p.parse("#hero").expect("parse");
        let b = p.parse("#hero").expect("parse");
        assert!(Arc::ptr_eq(&a, &b));
    }
}
