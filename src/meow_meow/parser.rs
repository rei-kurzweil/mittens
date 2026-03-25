use crate::meow_meow::ast::{
    AssignmentStatement, BinOpKind, BlockStatement, CallExpression, ComponentBodyItem,
    ComponentExpression, ConstructorCall, Expression, Ident, IfStatement, ReturnStatement,
    Statement, UnaryOpKind,
};
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
            TokenKind::Fn => {
                self.bump(); // consume `fn`
                // `fn name(params) { body }` — named function sugar for `let name = fn(params) { body }`
                if matches!(self.peek_kind(), TokenKind::Ident(_)) {
                    let name = self.expect_ident()?;
                    let func = self.parse_fn_body()?;
                    self.try_consume(&TokenKind::Semicolon);
                    Ok(Statement::Assignment(AssignmentStatement { name, value: func }))
                } else {
                    // anonymous fn in statement position — unusual but valid
                    let func = self.parse_fn_body()?;
                    self.try_consume(&TokenKind::Semicolon);
                    Ok(Statement::Expression(func))
                }
            }
            TokenKind::Return => {
                self.consume(&TokenKind::Return)?;
                if matches!(self.peek_kind(), TokenKind::Semicolon | TokenKind::RBrace) {
                    self.try_consume(&TokenKind::Semicolon);
                    return Ok(Statement::Return(ReturnStatement { value: None }));
                }
                let value = self.parse_expression()?;
                self.try_consume(&TokenKind::Semicolon);
                Ok(Statement::Return(ReturnStatement { value: Some(value) }))
            }
            TokenKind::If => {
                self.consume(&TokenKind::If)?;
                // No parentheses: `if condition { }` — condition is everything up to `{`
                let condition = self.parse_expression()?;
                let then_branch = self.parse_block_statement()?;
                let else_branch = if self.try_consume(&TokenKind::Else) {
                    Some(self.parse_block_statement()?)
                } else {
                    None
                };
                Ok(Statement::If(IfStatement { condition, then_branch, else_branch }))
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

    /// Pratt parser entry point.
    fn parse_expression(&mut self) -> Result<Expression, ParseError> {
        self.parse_expr_bp(0)
    }

    /// Pratt/precedence-climbing expression parser.
    /// `min_bp`: minimum binding power for the left side of the next infix op.
    fn parse_expr_bp(&mut self, min_bp: u8) -> Result<Expression, ParseError> {
        let mut lhs = self.parse_prefix()?;

        loop {
            let Some((l_bp, r_bp, op)) = self.peek_infix_op() else { break };
            if l_bp < min_bp {
                break;
            }
            self.bump(); // consume the operator token
            let rhs = self.parse_expr_bp(r_bp)?;
            lhs = Expression::BinaryOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }

        Ok(lhs)
    }

    /// Binding powers for infix operators. Returns (left_bp, right_bp, op).
    /// Left-associative: l_bp == r_bp - 1.
    fn peek_infix_op(&self) -> Option<(u8, u8, BinOpKind)> {
        match self.peek_kind() {
            TokenKind::PipePipe => Some((1, 2, BinOpKind::Or)),
            TokenKind::AmpAmp   => Some((3, 4, BinOpKind::And)),
            TokenKind::EqEq     => Some((5, 6, BinOpKind::Eq)),
            TokenKind::BangEq   => Some((5, 6, BinOpKind::NotEq)),
            TokenKind::Lt       => Some((7, 8, BinOpKind::Lt)),
            TokenKind::Gt       => Some((7, 8, BinOpKind::Gt)),
            TokenKind::LtEq     => Some((7, 8, BinOpKind::LtEq)),
            TokenKind::GtEq     => Some((7, 8, BinOpKind::GtEq)),
            TokenKind::Plus     => Some((9, 10, BinOpKind::Add)),
            TokenKind::Minus    => Some((9, 10, BinOpKind::Sub)),
            TokenKind::Star     => Some((11, 12, BinOpKind::Mul)),
            TokenKind::Slash    => Some((11, 12, BinOpKind::Div)),
            TokenKind::Percent  => Some((11, 12, BinOpKind::Rem)),
            _                   => None,
        }
    }

    /// Parse prefix / atom expressions (nud).
    fn parse_prefix(&mut self) -> Result<Expression, ParseError> {
        match self.peek_kind() {
            // Unary minus
            TokenKind::Minus => {
                self.bump();
                let operand = self.parse_expr_bp(13)?;
                Ok(Expression::UnaryOp { op: UnaryOpKind::Neg, operand: Box::new(operand) })
            }
            // Logical not
            TokenKind::Bang => {
                self.bump();
                let operand = self.parse_expr_bp(13)?;
                Ok(Expression::UnaryOp { op: UnaryOpKind::Not, operand: Box::new(operand) })
            }
            // Grouped expression
            TokenKind::LParen => {
                self.bump();
                let inner = self.parse_expr_bp(0)?;
                self.consume(&TokenKind::RParen)?;
                Ok(inner)
            }
            // Function expression
            TokenKind::Fn => {
                self.bump();
                self.parse_fn_body()
            }
            // Literals
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
            TokenKind::True  => { self.bump(); Ok(Expression::Bool(true)) }
            TokenKind::False => { self.bump(); Ok(Expression::Bool(false)) }
            TokenKind::Null  => { self.bump(); Ok(Expression::Null) }
            TokenKind::LBracket => self.parse_array(),
            TokenKind::Ident(_) => self.parse_ident_leading_expression(),
            _ => Err(self.err("Unexpected token in expression")),
        }
    }

    /// Parse `(params) { body }` — the part of a function after the `fn` keyword (and optional name).
    fn parse_fn_body(&mut self) -> Result<Expression, ParseError> {
        self.consume(&TokenKind::LParen)?;
        let mut params = Vec::new();
        if !matches!(self.peek_kind(), TokenKind::RParen) {
            loop {
                params.push(self.expect_ident()?);
                if !self.try_consume(&TokenKind::Comma) {
                    break;
                }
                if matches!(self.peek_kind(), TokenKind::RParen) {
                    break;
                }
            }
        }
        self.consume(&TokenKind::RParen)?;
        let body = self.parse_block_statement()?;
        Ok(Expression::Function { params, body })
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

    /// Parse an expression that starts with an identifier.
    ///
    /// Disambiguates:
    /// - `Ident '.' Ident '(' args ')' ('{' body)?` → component expression with head call
    /// - `Ident '(' args ')'`                        → free call expression
    /// - `Ident '{' body`                            → component expression, no head call
    /// - `Ident`                                     → bare identifier
    fn parse_ident_leading_expression(&mut self) -> Result<Expression, ParseError> {
        let ident = self.expect_ident()?;

        // `Type.method(args) ...` → component expression with head call
        if self.try_consume(&TokenKind::Dot) {
            let method = self.expect_ident()?;
            self.consume(&TokenKind::LParen)?;
            let args = self.parse_call_args()?;
            let constructor = Some(ConstructorCall { method, args });
            let body = if self.try_consume(&TokenKind::LBrace) {
                self.parse_component_body()?
            } else {
                vec![]
            };
            return Ok(Expression::Component(ComponentExpression {
                component_type: ident,
                constructor,
                body,
            }));
        }

        // `ident(args)` → free call expression
        if self.try_consume(&TokenKind::LParen) {
            let args = self.parse_call_args()?;
            return Ok(Expression::Call(CallExpression { callee: ident, args }));
        }

        // `ident { body }` → component expression, no head call.
        // Convention: component type names always start uppercase; lowercase = variable.
        // This prevents `if flag { ... }` from consuming `flag {` as a component expression.
        let is_component_type = ident.0.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
        if is_component_type && self.try_consume(&TokenKind::LBrace) {
            let body = self.parse_component_body()?;
            return Ok(Expression::Component(ComponentExpression {
                component_type: ident,
                constructor: None,
                body,
            }));
        }

        // bare identifier
        Ok(Expression::Identifier(ident))
    }

    /// Parse the inside of a component body, up to and including the closing `}`.
    /// Called after the `{` has already been consumed.
    fn parse_component_body(&mut self) -> Result<Vec<ComponentBodyItem>, ParseError> {
        let mut body = Vec::new();

        loop {
            if self.try_consume(&TokenKind::RBrace) {
                break;
            }
            if self.is_eof() {
                return Err(self.err("Unterminated component body"));
            }
            if self.try_consume(&TokenKind::Comma) || self.try_consume(&TokenKind::Semicolon) {
                continue;
            }

            match self.peek_kind() {
                TokenKind::Ident(_) => {
                    let save = self.pos;
                    let leading = self.expect_ident()?;

                    // `Type.method(args) ...` → child component with head call
                    if self.try_consume(&TokenKind::Dot) {
                        let method = self.expect_ident()?;
                        self.consume(&TokenKind::LParen)?;
                        let args = self.parse_call_args()?;
                        let constructor = Some(ConstructorCall { method, args });
                        let child_body = if self.try_consume(&TokenKind::LBrace) {
                            self.parse_component_body()?
                        } else {
                            vec![]
                        };
                        body.push(ComponentBodyItem::Child(ComponentExpression {
                            component_type: leading,
                            constructor,
                            body: child_body,
                        }));
                        continue;
                    }

                    // `ident = expr` → named assignment
                    if self.try_consume(&TokenKind::Eq) {
                        let value = self.parse_expression()?;
                        body.push(ComponentBodyItem::NamedAssignment { name: leading, value });
                        continue;
                    }

                    // `ident(args)` → builder call
                    if self.try_consume(&TokenKind::LParen) {
                        let args = self.parse_call_args()?;
                        body.push(ComponentBodyItem::Call(CallExpression {
                            callee: leading,
                            args,
                        }));
                        continue;
                    }

                    // `ident { body }` → child component, no head call
                    if self.try_consume(&TokenKind::LBrace) {
                        let child_body = self.parse_component_body()?;
                        body.push(ComponentBodyItem::Child(ComponentExpression {
                            component_type: leading,
                            constructor: None,
                            body: child_body,
                        }));
                        continue;
                    }

                    // bare identifier → positional; rewind and re-parse as expression
                    self.pos = save;
                    let expr = self.parse_expression()?;
                    body.push(ComponentBodyItem::Positional(expr));
                }
                TokenKind::String(_)
                | TokenKind::Number(_)
                | TokenKind::True
                | TokenKind::False
                | TokenKind::Null
                | TokenKind::LBracket
                | TokenKind::Minus => {
                    let expr = self.parse_expression()?;
                    body.push(ComponentBodyItem::Positional(expr));
                }
                _ => {
                    return Err(self.err("Unexpected token in component body"));
                }
            }
        }

        Ok(body)
    }

    fn parse_call_args(&mut self) -> Result<Vec<Expression>, ParseError> {
        let mut args = Vec::new();
        if self.try_consume(&TokenKind::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_expression()?);
            if self.try_consume(&TokenKind::Comma) {
                if self.try_consume(&TokenKind::RParen) {
                    break;
                }
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
            Err(self.err(&format!("Expected {:?}", kind)))
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
            span: crate::meow_meow::ast::Span::new(0, 0),
        });
        self.pos += 1;
        t
    }

    fn peek_kind(&self) -> &TokenKind {
        self.tokens.get(self.pos).map(|t| &t.kind).unwrap_or(&TokenKind::Eof)
    }

    fn is_eof(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    fn err(&self, message: &str) -> ParseError {
        ParseError { message: message.to_string(), token_index: self.pos }
    }
}
