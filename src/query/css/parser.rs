use crate::query::ast::{
    AttributeSelector, Combinator, CompoundSelector, QueryAst, SelectorSegment, SelectorSequence,
    SimpleSelector,
};
use crate::query::{QueryParseError, QuerySyntax};

pub struct CssQuerySyntax;

impl QuerySyntax for CssQuerySyntax {
    fn parse(input: &str) -> Result<QueryAst, QueryParseError> {
        Parser::new(input).parse_query()
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
                    simple_selectors.push(SimpleSelector::Id(ident));
                }
                Some('.') => {
                    self.bump_char();
                    let ident = self.parse_identifier()?;
                    simple_selectors.push(SimpleSelector::Class(ident));
                }
                Some('[') => {
                    simple_selectors.push(SimpleSelector::Attribute(self.parse_attribute_selector()?));
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
            Some(ch) if is_ident_start(ch) || ch.is_ascii_digit() => self.parse_identifier_or_number(),
            _ => Err(self.err("expected attribute value")),
        }
    }

    fn parse_quoted_string(&mut self) -> Result<String, QueryParseError> {
        let quote = self.bump_char().ok_or_else(|| self.err("expected string quote"))?;
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
    use super::CssQuerySyntax;
    use crate::query::ast::{Combinator, SimpleSelector};
    use crate::query::QuerySyntax;

    #[test]
    fn parses_name_attribute_selector() {
        let ast = CssQuerySyntax::parse("[name='container']").expect("parse");
        assert_eq!(ast.selector_groups.len(), 1);
        assert_eq!(ast.selector_groups[0].segments.len(), 1);
        match &ast.selector_groups[0].segments[0].compound.simple_selectors[0] {
            SimpleSelector::Attribute(attr) => {
                assert_eq!(attr.name, "name");
                assert_eq!(attr.value.as_deref(), Some("container"));
            }
            other => panic!("expected attribute selector, got {:?}", other),
        }
    }

    #[test]
    fn parses_child_and_descendant_combinators() {
        let ast = CssQuerySyntax::parse("#root > [name='container'] .row").expect("parse");
        let segments = &ast.selector_groups[0].segments;
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[1].combinator, Some(Combinator::Child));
        assert_eq!(segments[2].combinator, Some(Combinator::Descendant));
    }
}