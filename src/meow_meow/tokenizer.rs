use crate::meow_meow::ast::Span;
use crate::meow_meow::token::{Token, TokenKind, TokenizeError, Unit};

pub struct MeowMeowTokenizer<'a> {
    input: &'a str,
    bytes: &'a [u8],
    idx: usize,
}

impl<'a> MeowMeowTokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            bytes: input.as_bytes(),
            idx: 0,
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, TokenizeError> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token()?;
            let is_eof = matches!(token.kind, TokenKind::Eof);
            tokens.push(token);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }

    fn next_token(&mut self) -> Result<Token, TokenizeError> {
        self.skip_ws_and_comments()?;
        let start = self.idx;
        if self.idx >= self.bytes.len() {
            return Ok(Token {
                kind: TokenKind::Eof,
                span: Span::new(self.idx, self.idx),
            });
        }

        let b = self.bytes[self.idx];
        let kind = match b {
            b'{' => {
                self.idx += 1;
                TokenKind::LBrace
            }
            b'}' => {
                self.idx += 1;
                TokenKind::RBrace
            }
            b'(' => {
                self.idx += 1;
                TokenKind::LParen
            }
            b')' => {
                self.idx += 1;
                TokenKind::RParen
            }
            b'[' => {
                self.idx += 1;
                TokenKind::LBracket
            }
            b']' => {
                self.idx += 1;
                TokenKind::RBracket
            }
            b',' => {
                self.idx += 1;
                TokenKind::Comma
            }
            b'.' => {
                self.idx += 1;
                TokenKind::Dot
            }
            b';' => {
                self.idx += 1;
                TokenKind::Semicolon
            }
            b'+' => {
                self.idx += 1;
                TokenKind::Plus
            }
            b'-' => {
                self.idx += 1;
                if self.idx < self.bytes.len() && self.bytes[self.idx] == b'>' {
                    self.idx += 1;
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }
            b'*' => {
                self.idx += 1;
                TokenKind::Star
            }
            b'/' => {
                self.idx += 1;
                TokenKind::Slash
            }
            b'%' => {
                self.idx += 1;
                TokenKind::Percent
            }
            b'=' => {
                self.idx += 1;
                if self.idx < self.bytes.len() && self.bytes[self.idx] == b'=' {
                    self.idx += 1;
                    TokenKind::EqEq
                } else {
                    TokenKind::Eq
                }
            }
            b'!' => {
                self.idx += 1;
                if self.idx < self.bytes.len() && self.bytes[self.idx] == b'=' {
                    self.idx += 1;
                    TokenKind::BangEq
                } else {
                    TokenKind::Bang
                }
            }
            b'<' => {
                self.idx += 1;
                if self.idx < self.bytes.len() && self.bytes[self.idx] == b'=' {
                    self.idx += 1;
                    TokenKind::LtEq
                } else {
                    TokenKind::Lt
                }
            }
            b'>' => {
                self.idx += 1;
                if self.idx < self.bytes.len() && self.bytes[self.idx] == b'=' {
                    self.idx += 1;
                    TokenKind::GtEq
                } else {
                    TokenKind::Gt
                }
            }
            b'&' => {
                self.idx += 1;
                if self.idx < self.bytes.len() && self.bytes[self.idx] == b'&' {
                    self.idx += 1;
                    TokenKind::AmpAmp
                } else {
                    return Err(TokenizeError {
                        message: "Expected '&&'".to_string(),
                        span: Span::new(start, self.idx),
                    });
                }
            }
            b'|' => {
                self.idx += 1;
                if self.idx < self.bytes.len() && self.bytes[self.idx] == b'|' {
                    self.idx += 1;
                    TokenKind::PipePipe
                } else if self.idx < self.bytes.len() && self.bytes[self.idx] == b'>' {
                    self.idx += 1;
                    TokenKind::PipeGt
                } else {
                    return Err(TokenizeError {
                        message: "Expected '||' or '|>'".to_string(),
                        span: Span::new(start, self.idx),
                    });
                }
            }
            b'"' => TokenKind::String(self.read_string()?),
            b'0'..=b'9' => {
                let n = self.read_number()?;
                match self.try_read_unit_suffix() {
                    Some(unit) => TokenKind::Dimension(n, unit),
                    None => TokenKind::Number(n),
                }
            }
            _ => {
                if is_ident_start(b) {
                    let ident = self.read_ident();
                    match ident.as_str() {
                        "let" => TokenKind::Let,
                        "if" => TokenKind::If,
                        "else" => TokenKind::Else,
                        "return" => TokenKind::Return,
                        "true" => TokenKind::True,
                        "false" => TokenKind::False,
                        "null" => TokenKind::Null,
                        "fn" => TokenKind::Fn,
                        "for" => TokenKind::For,
                        "while" => TokenKind::While,
                        "in" => TokenKind::In,
                        "break" => TokenKind::Break,
                        "continue" => TokenKind::Continue,
                        "export" => TokenKind::Export,
                        "import" => TokenKind::Import,
                        "from" => TokenKind::From,
                        "as" => TokenKind::As,
                        _ => TokenKind::Ident(ident),
                    }
                } else {
                    return Err(TokenizeError {
                        message: format!("Unexpected character: {}", self.current_char_debug()),
                        span: Span::new(self.idx, self.idx + 1),
                    });
                }
            }
        };

        Ok(Token {
            kind,
            span: Span::new(start, self.idx),
        })
    }

    fn skip_ws_and_comments(&mut self) -> Result<(), TokenizeError> {
        loop {
            while self.idx < self.bytes.len() {
                match self.bytes[self.idx] {
                    b' ' | b'\t' | b'\n' | b'\r' => self.idx += 1,
                    _ => break,
                }
            }

            // line comment
            if self.peek2() == Some((b'/', b'/')) {
                self.idx += 2;
                while self.idx < self.bytes.len() && self.bytes[self.idx] != b'\n' {
                    self.idx += 1;
                }
                continue;
            }

            // block comment
            if self.peek2() == Some((b'/', b'*')) {
                let start = self.idx;
                self.idx += 2;
                while self.idx + 1 < self.bytes.len() {
                    if self.bytes[self.idx] == b'*' && self.bytes[self.idx + 1] == b'/' {
                        self.idx += 2;
                        break;
                    }
                    self.idx += 1;
                }
                if self.idx >= self.bytes.len() {
                    return Err(TokenizeError {
                        message: "Unterminated block comment".to_string(),
                        span: Span::new(start, self.bytes.len()),
                    });
                }
                continue;
            }

            break;
        }

        Ok(())
    }

    fn read_ident(&mut self) -> String {
        let start = self.idx;
        self.idx += 1;
        while self.idx < self.bytes.len() && is_ident_continue(self.bytes[self.idx]) {
            self.idx += 1;
        }
        self.input[start..self.idx].to_string()
    }

    fn read_string(&mut self) -> Result<String, TokenizeError> {
        let start = self.idx;
        // opening quote
        self.idx += 1;
        let mut out = String::new();
        while self.idx < self.bytes.len() {
            let b = self.bytes[self.idx];
            match b {
                b'"' => {
                    self.idx += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.idx += 1;
                    if self.idx >= self.bytes.len() {
                        break;
                    }
                    let esc = self.bytes[self.idx];
                    self.idx += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        _ => {
                            return Err(TokenizeError {
                                message: format!("Unsupported escape: \\\\{}", esc as char),
                                span: Span::new(self.idx - 2, self.idx),
                            });
                        }
                    }
                }
                _ => {
                    // UTF-8: take the next char boundary using str methods
                    let s = &self.input[self.idx..];
                    if let Some(ch) = s.chars().next() {
                        out.push(ch);
                        self.idx += ch.len_utf8();
                    } else {
                        break;
                    }
                }
            }
        }

        Err(TokenizeError {
            message: "Unterminated string literal".to_string(),
            span: Span::new(start, self.idx),
        })
    }

    fn read_number(&mut self) -> Result<f64, TokenizeError> {
        let start = self.idx;
        while self.idx < self.bytes.len() {
            match self.bytes[self.idx] {
                b'0'..=b'9' => self.idx += 1,
                _ => break,
            }
        }
        if self.idx < self.bytes.len() && self.bytes[self.idx] == b'.' {
            self.idx += 1;
            while self.idx < self.bytes.len() {
                match self.bytes[self.idx] {
                    b'0'..=b'9' => self.idx += 1,
                    _ => break,
                }
            }
        }

        let s = &self.input[start..self.idx];
        s.parse::<f64>().map_err(|_| TokenizeError {
            message: format!("Invalid number literal: {s}"),
            span: Span::new(start, self.idx),
        })
    }

    /// Try to consume a unit suffix attached (no whitespace) to a numeric
    /// literal. Recognized: `%`, `gu`, `deg`, `rad`. Returns `None` if the
    /// next character isn't part of a recognized suffix — caller falls back
    /// to a bare `Number` token.
    fn try_read_unit_suffix(&mut self) -> Option<Unit> {
        if self.idx >= self.bytes.len() {
            return None;
        }
        if self.bytes[self.idx] == b'%' {
            self.idx += 1;
            return Some(Unit::Percent);
        }
        // Letter-prefixed suffixes: peek without consuming, only commit on match.
        let start = self.idx;
        let mut end = start;
        while end < self.bytes.len() && is_ident_continue(self.bytes[end]) {
            end += 1;
        }
        if end == start {
            return None;
        }
        let unit = match &self.input[start..end] {
            "gu" => Unit::GlyphUnits,
            "wu" => Unit::WorldUnits,
            "deg" => Unit::Degrees,
            "rad" => Unit::Radians,
            _ => return None,
        };
        self.idx = end;
        Some(unit)
    }

    fn peek2(&self) -> Option<(u8, u8)> {
        if self.idx + 1 >= self.bytes.len() {
            None
        } else {
            Some((self.bytes[self.idx], self.bytes[self.idx + 1]))
        }
    }

    fn current_char_debug(&self) -> String {
        if self.idx >= self.bytes.len() {
            return "<eof>".to_string();
        }
        let s = &self.input[self.idx..];
        s.chars()
            .next()
            .map(|c| c.to_string())
            .unwrap_or("<invalid>".to_string())
    }
}

fn is_ident_start(b: u8) -> bool {
    matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'_')
}

fn is_ident_continue(b: u8) -> bool {
    is_ident_start(b) || matches!(b, b'0'..=b'9')
}
