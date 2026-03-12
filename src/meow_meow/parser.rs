use crate::meow_meow::ast::expression::{CallExpression, ComponentExpression, Expression, Ident, Parameter};
use crate::meow_meow::ast::statement::{AssignmentStatement, BlockStatement, IfStatement, ReturnStatement, Statement};
use crate::meow_meow::token::{Token, TokenKind};

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub token_index: usize,
}

pub struct MeowMeowParser {
    tokens: Vec<Token>,
    pos: usize,
}

impl MeowMeowParser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse_program(mut self) -> Result<Vec<Statement>, ParseError> {
        let mut statements = Vec::new();
        while !self.is_eof() {
            // allow stray semicolons
            if self.try_consume(&TokenKind::Semicolon) {
                continue;
            }
            statements.push(self.parse_statement()?);
        }
        Ok(statements)
    }

    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match self.peek_kind() {
            TokenKind::Let => {
                self.consume(&TokenKind::Let)?;
                let name = self.expect_ident()?;
                self.consume(&TokenKind::Eq)?;
                let value = self.parse_expression()?;
                self.try_consume(&TokenKind::Semicolon);
                Ok(Statement::Assignment(AssignmentStatement { name, value }))
            }
            TokenKind::Return => {
                self.consume(&TokenKind::Return)?;
                if matches!(self.peek_kind(), TokenKind::Semicolon | TokenKind::RBrace) {
                    self.try_consume(&TokenKind::Semicolon);
                    return Ok(Statement::Return(ReturnStatement { value: None }));
                }
                let value = self.parse_expression()?;
                self.try_consume(&TokenKind::Semicolon);
                Ok(Statement::Return(ReturnStatement {
                    value: Some(value),
                }))
            }
            TokenKind::If => {
                self.consume(&TokenKind::If)?;
                let condition = self.parse_expression()?;
                let then_branch = self.parse_block_statement()?;
                let else_branch = if self.try_consume(&TokenKind::Else) {
                    Some(self.parse_block_statement()?)
                } else {
                    None
                };
                Ok(Statement::If(IfStatement {
                    condition,
                    then_branch,
                    else_branch,
                }))
            }
            TokenKind::LBrace => Ok(Statement::Block(self.parse_block_statement()?)),
            _ => {
                let expr = self.parse_expression()?;
                self.try_consume(&TokenKind::Semicolon);
                Ok(Statement::Expression(expr))
            }
        }
    }

    fn parse_block_statement(&mut self) -> Result<BlockStatement, ParseError> {
        self.consume(&TokenKind::LBrace)?;
        let mut statements = Vec::new();
        while !self.try_consume(&TokenKind::RBrace) {
            if self.is_eof() {
                return Err(self.err("Unterminated block"));
            }
            if self.try_consume(&TokenKind::Semicolon) {
                continue;
            }
            statements.push(self.parse_statement()?);
        }
        Ok(BlockStatement { statements })
    }

    fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        // Minimal expression grammar for the first iteration:
        // - literals: string/number/bool/null
        // - array literals
        // - identifiers
        // - call expressions: ident '(' ... ')'
        // - component expressions: ident (attrs)* '{' body '}'
        match self.peek_kind() {
            TokenKind::String(_) => {
                if let TokenKind::String(s) = self.bump().kind {
                    Ok(Expression::String(s))
                } else {
                    unreachable!()
                }
            }
            TokenKind::Number(_) => {
                if let TokenKind::Number(n) = self.bump().kind {
                    Ok(Expression::Number(n))
                } else {
                    unreachable!()
                }
            }
            TokenKind::True => {
                self.bump();
                Ok(Expression::Bool(true))
            }
            TokenKind::False => {
                self.bump();
                Ok(Expression::Bool(false))
            }
            TokenKind::Null => {
                self.bump();
                Ok(Expression::Null)
            }
            TokenKind::LBracket => self.parse_array(),
            TokenKind::Ident(_) => self.parse_ident_leading_expression(),
            _ => Err(self.err("Unexpected token in expression")),
        }
    }

    fn parse_array(&mut self) -> Result<Expression, ParseError> {
        self.consume(&TokenKind::LBracket)?;
        let mut items = Vec::new();
        if self.try_consume(&TokenKind::RBracket) {
            return Ok(Expression::Array(items));
        }
        loop {
            items.push(self.parse_expression()?);
            if self.try_consume(&TokenKind::Comma) {
                if self.try_consume(&TokenKind::RBracket) {
                    break;
                }
                continue;
            }
            self.consume(&TokenKind::RBracket)?;
            break;
        }
        Ok(Expression::Array(items))
    }

    fn parse_ident_leading_expression(&mut self) -> Result<Expression, ParseError> {
        let ident = self.expect_ident()?;

        // function call: foo(...)
        if self.try_consume(&TokenKind::LParen) {
            let args = self.parse_call_args()?;
            return Ok(Expression::Call(CallExpression { callee: ident, args }));
        }

        // component expression: T name="x" guid="y" { ... }
        // We allow attributes *only* in the header (before the '{').
        let mut parameters = Vec::new();
        while matches!(self.peek_kind(), TokenKind::Ident(_)) {
            // attribute name
            let name = self.expect_ident()?;
            if !self.try_consume(&TokenKind::Eq) {
                // not an attribute; rewind by 1 token and stop scanning attrs
                self.pos -= 1;
                break;
            }
            let value = self.parse_expression()?;
            parameters.push(Parameter { name, value });
        }

        if self.try_consume(&TokenKind::LBrace) {
            let component = self.parse_component_body(ident, parameters)?;
            Ok(Expression::Component(component))
        } else {
            // plain identifier expression
            Ok(Expression::Identifier(ident))
        }
    }

    fn parse_component_body(
        &mut self,
        component_type: Ident,
        parameters: Vec<Parameter>,
    ) -> Result<ComponentExpression, ParseError> {
        let mut positional = Vec::new();
        let mut calls = Vec::new();
        let mut children = Vec::new();

        while !self.try_consume(&TokenKind::RBrace) {
            if self.is_eof() {
                return Err(self.err("Unterminated component body"));
            }

            // Allow commas/semicolons as separators.
            if self.try_consume(&TokenKind::Comma) || self.try_consume(&TokenKind::Semicolon) {
                continue;
            }

            match self.peek_kind() {
                TokenKind::Ident(_) => {
                    // Lookahead to decide between:
                    // - call: ident '(' ... ')'
                    // - child component: ident (attrs)* '{' ... '}'
                    // - positional expression: bare ident (e.g. QUAD_2D)
                    let save = self.pos;
                    let leading = self.expect_ident()?;
                    if self.try_consume(&TokenKind::LParen) {
                        let args = self.parse_call_args()?;
                        calls.push(CallExpression {
                            callee: leading,
                            args,
                        });
                        continue;
                    }

                    // Try parse header attrs + '{'
                    let mut child_params = Vec::new();
                    while matches!(self.peek_kind(), TokenKind::Ident(_)) {
                        let name = self.expect_ident()?;
                        if !self.try_consume(&TokenKind::Eq) {
                            self.pos -= 1;
                            break;
                        }
                        let value = self.parse_expression()?;
                        child_params.push(Parameter { name, value });
                    }

                    if self.try_consume(&TokenKind::LBrace) {
                        let child = self.parse_component_body(leading, child_params)?;
                        children.push(child);
                        continue;
                    }

                    // Not a call nor a child component; treat as positional ident.
                    self.pos = save;
                    positional.push(self.parse_expression()?);
                }
                TokenKind::String(_)
                | TokenKind::Number(_)
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Null
                | TokenKind::LBracket => {
                    positional.push(self.parse_expression()?);
                }
                TokenKind::RBrace => {
                    self.consume(&TokenKind::RBrace)?;
                    break;
                }
                _ => {
                    return Err(self.err("Unexpected token in component body"));
                }
            }
        }

        Ok(ComponentExpression {
            component_type,
            parameters,
            positional,
            calls,
            children,
        })
    }

    fn parse_call_args(&mut self) -> Result<Vec<Expression>, ParseError> {
        let mut args = Vec::new();
        if self.try_consume(&TokenKind::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_expression()?);
            if self.try_consume(&TokenKind::Comma) {
                continue;
            }
            self.consume(&TokenKind::RParen)?;
            break;
        }
        Ok(args)
    }

    fn expect_ident(&mut self) -> Result<Ident, ParseError> {
        match self.bump().kind {
            TokenKind::Ident(s) => Ok(Ident(s)),
            _ => Err(self.err("Expected identifier")),
        }
    }

    fn consume(&mut self, kind: &TokenKind) -> Result<(), ParseError> {
        if self.try_consume(kind) {
            Ok(())
        } else {
            Err(self.err("Unexpected token"))
        }
    }

    fn try_consume(&mut self, kind: &TokenKind) -> bool {
        if std::mem::discriminant(self.peek_kind()) == std::mem::discriminant(kind) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn bump(&mut self) -> Token {
        let t = self.tokens.get(self.pos).cloned().unwrap_or(Token {
            kind: TokenKind::Eof,
            span: crate::meow_meow::ast::expression::Span::new(0, 0),
        });
        self.pos += 1;
        t
    }

    fn peek_kind(&self) -> &TokenKind {
        self.tokens
            .get(self.pos)
            .map(|t| &t.kind)
            .unwrap_or(&TokenKind::Eof)
    }

    fn is_eof(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    fn err(&self, message: &str) -> ParseError {
        ParseError {
            message: message.to_string(),
            token_index: self.pos,
        }
    }
}
