use crate::meow_meow::ast::{
    AssignmentStatement, BinOpKind, BlockStatement, CallExpression, ComponentExpression,
    ConstructorCall, ElseBranch, Expression, Ident, IfStatement, ImportItem, ReturnStatement, Span,
    Statement, UnaryOpKind,
};
use crate::meow_meow::token::{Token, TokenKind};

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub token_index: usize,
    pub span: Span,
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
                Ok(Statement::Assignment(AssignmentStatement {
                    name,
                    value,
                    exported: false,
                }))
            }
            TokenKind::Fn => {
                self.bump(); // consume `fn`
                // `fn name(params) { body }` — named function sugar for `let name = fn(params) { body }`
                if matches!(self.peek_kind(), TokenKind::Ident(_)) {
                    let name = self.expect_ident()?;
                    let func = self.parse_fn_body()?;
                    self.try_consume(&TokenKind::Semicolon);
                    Ok(Statement::Assignment(AssignmentStatement {
                        name,
                        value: func,
                        exported: false,
                    }))
                } else {
                    // anonymous fn in statement position — unusual but valid
                    let func = self.parse_fn_body()?;
                    self.try_consume(&TokenKind::Semicolon);
                    Ok(Statement::Expression(func))
                }
            }
            TokenKind::Export => {
                self.bump(); // consume 'export'
                let exported = true;
                match self.peek_kind() {
                    TokenKind::Let => {
                        self.bump();
                        let name = self.expect_ident()?;
                        self.consume(&TokenKind::Eq)?;
                        let value = self.parse_expression()?;
                        self.try_consume(&TokenKind::Semicolon);
                        Ok(Statement::Assignment(AssignmentStatement {
                            name,
                            value,
                            exported,
                        }))
                    }
                    TokenKind::Fn => {
                        self.bump();
                        let name = self.expect_ident()?;
                        let func = self.parse_fn_body()?;
                        self.try_consume(&TokenKind::Semicolon);
                        Ok(Statement::Assignment(AssignmentStatement {
                            name,
                            value: func,
                            exported,
                        }))
                    }
                    _ => Err(self.err("Expected 'let' or 'fn' after 'export'")),
                }
            }
            TokenKind::Import => {
                self.bump(); // consume 'import'
                self.consume(&TokenKind::LBrace)?;
                let mut items = Vec::new();
                if !self.try_consume(&TokenKind::RBrace) {
                    loop {
                        match self.peek_kind().clone() {
                            TokenKind::Number(n) => {
                                self.bump();
                                let index = n as usize;
                                self.consume(&TokenKind::As)?;
                                let alias = self.expect_ident()?;
                                items.push(ImportItem::PositionalAlias { index, alias });
                            }
                            TokenKind::Ident(_) => {
                                let name = self.expect_ident()?;
                                if self.try_consume(&TokenKind::As) {
                                    let alias = self.expect_ident()?;
                                    items.push(ImportItem::NamedAlias { name, alias });
                                } else {
                                    items.push(ImportItem::Named(name));
                                }
                            }
                            _ => {
                                return Err(
                                    self.err("Expected identifier or number in import list")
                                );
                            }
                        }
                        if !self.try_consume(&TokenKind::Comma) {
                            break;
                        }
                        if matches!(self.peek_kind(), TokenKind::RBrace) {
                            break; // trailing comma
                        }
                    }
                    self.consume(&TokenKind::RBrace)?;
                }
                self.consume(&TokenKind::From)?;
                let path = match self.peek_kind().clone() {
                    TokenKind::String(s) => {
                        self.bump();
                        s
                    }
                    _ => return Err(self.err("Expected string path after 'from'")),
                };
                self.try_consume(&TokenKind::Semicolon);
                Ok(Statement::Import { items, path })
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
            TokenKind::If => Ok(Statement::If(self.parse_if_statement()?)),
            TokenKind::For => {
                self.consume(&TokenKind::For)?;
                let binding = self.expect_ident()?;
                self.consume(&TokenKind::In)?;
                let iterable = self.parse_expression()?;
                let body = self.parse_block_statement()?;
                Ok(Statement::ForIn {
                    binding,
                    iterable,
                    body,
                })
            }
            TokenKind::While => {
                self.consume(&TokenKind::While)?;
                let condition = self.parse_expression()?;
                let body = self.parse_block_statement()?;
                Ok(Statement::While { condition, body })
            }
            TokenKind::Break => {
                self.bump();
                self.try_consume(&TokenKind::Semicolon);
                Ok(Statement::Break)
            }
            TokenKind::Continue => {
                self.bump();
                self.try_consume(&TokenKind::Semicolon);
                Ok(Statement::Continue)
            }
            TokenKind::LBrace => Ok(Statement::Block(self.parse_block_statement()?)),
            _ => {
                // `ident = expr` reassignment — two-token lookahead to avoid
                // consuming the start of a comparison expression like `x == y`.
                if matches!(self.peek_kind(), TokenKind::Ident(_))
                    && self
                        .tokens
                        .get(self.pos + 1)
                        .map(|t| matches!(t.kind, TokenKind::Eq))
                        .unwrap_or(false)
                {
                    let name = self.expect_ident()?;
                    self.consume(&TokenKind::Eq)?;
                    let value = self.parse_expression()?;
                    self.try_consume(&TokenKind::Semicolon);
                    return Ok(Statement::Reassign { name, value });
                }
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

    fn parse_if_statement(&mut self) -> Result<IfStatement, ParseError> {
        self.consume(&TokenKind::If)?;
        // No parentheses: `if condition { }` — condition is everything up to `{`
        let condition = self.parse_expression()?;
        let then_branch = self.parse_block_statement()?;
        let else_branch = if self.try_consume(&TokenKind::Else) {
            if matches!(self.peek_kind(), TokenKind::If) {
                Some(ElseBranch::If(Box::new(self.parse_if_statement()?)))
            } else {
                Some(ElseBranch::Block(self.parse_block_statement()?))
            }
        } else {
            None
        };
        Ok(IfStatement {
            condition,
            then_branch,
            else_branch,
        })
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
            if self.try_consume(&TokenKind::LBracket) {
                let index = self.parse_expression()?;
                self.consume(&TokenKind::RBracket)?;
                lhs = Expression::Index {
                    base: Box::new(lhs),
                    index: Box::new(index),
                };
                continue;
            }

            let Some((l_bp, r_bp, op)) = self.peek_infix_op() else {
                break;
            };
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
            TokenKind::Arrow => Some((0, 1, BinOpKind::Query)),
            TokenKind::PipeGt => Some((2, 3, BinOpKind::Pipe)),
            TokenKind::PipePipe => Some((4, 5, BinOpKind::Or)),
            TokenKind::AmpAmp => Some((6, 7, BinOpKind::And)),
            TokenKind::EqEq => Some((8, 9, BinOpKind::Eq)),
            TokenKind::BangEq => Some((8, 9, BinOpKind::NotEq)),
            TokenKind::Lt => Some((10, 11, BinOpKind::Lt)),
            TokenKind::Gt => Some((10, 11, BinOpKind::Gt)),
            TokenKind::LtEq => Some((10, 11, BinOpKind::LtEq)),
            TokenKind::GtEq => Some((10, 11, BinOpKind::GtEq)),
            TokenKind::Plus => Some((12, 13, BinOpKind::Add)),
            TokenKind::Minus => Some((12, 13, BinOpKind::Sub)),
            TokenKind::Star => Some((14, 15, BinOpKind::Mul)),
            TokenKind::Slash => Some((14, 15, BinOpKind::Div)),
            TokenKind::Percent => Some((14, 15, BinOpKind::Rem)),
            _ => None,
        }
    }

    /// Parse prefix / atom expressions (nud).
    fn parse_prefix(&mut self) -> Result<Expression, ParseError> {
        match self.peek_kind() {
            // Unary minus
            TokenKind::Minus => {
                self.bump();
                let operand = self.parse_expr_bp(17)?;
                Ok(Expression::UnaryOp {
                    op: UnaryOpKind::Neg,
                    operand: Box::new(operand),
                })
            }
            // Logical not
            TokenKind::Bang => {
                self.bump();
                let operand = self.parse_expr_bp(17)?;
                Ok(Expression::UnaryOp {
                    op: UnaryOpKind::Not,
                    operand: Box::new(operand),
                })
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
            TokenKind::Dimension(_, _) => {
                if let TokenKind::Dimension(n, unit) = self.bump().kind {
                    Ok(Expression::Dimension(n, unit))
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
    /// - Uppercase `Type.method(args)[.method2(args2)...] [{body}]` → ComponentExpression
    /// - Lowercase `obj.method(args)`                               → Call with Dot callee
    /// - `ident(args)`                                              → free CallExpression
    /// - `UpperType { body }`                                       → ComponentExpression, no ctor
    /// - `ident`                                                    → bare Identifier
    fn parse_ident_leading_expression(&mut self) -> Result<Expression, ParseError> {
        let ident = self.expect_ident()?;
        let is_component_type = ident
            .0
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false);

        if self.try_consume(&TokenKind::Dot) {
            let method = self.expect_ident()?;
            self.consume(&TokenKind::LParen)?;
            let args = self.parse_call_args()?;

            if is_component_type {
                // `Type.method(args)[.chain(args)...] [{body}]` → ComponentExpression
                let mut constructors = vec![ConstructorCall { method, args }];
                while self.try_consume(&TokenKind::Dot) {
                    let chained = self.expect_ident()?;
                    self.consume(&TokenKind::LParen)?;
                    let chained_args = self.parse_call_args()?;
                    constructors.push(ConstructorCall {
                        method: chained,
                        args: chained_args,
                    });
                }
                let body = if matches!(self.peek_kind(), TokenKind::LBrace) {
                    self.parse_block_statement()?
                } else {
                    BlockStatement { statements: vec![] }
                };
                return Ok(Expression::Component(ComponentExpression {
                    component_type: ident,
                    constructors,
                    body,
                }));
            } else {
                // `obj.method(args)` → Call { callee: BinaryOp(Dot, obj, method) }
                let callee = Box::new(Expression::BinaryOp {
                    op: BinOpKind::Dot,
                    lhs: Box::new(Expression::Identifier(ident)),
                    rhs: Box::new(Expression::Identifier(method)),
                });
                return Ok(Expression::Call(CallExpression { callee, args }));
            }
        }

        // `ident(args)` → free call expression
        if self.try_consume(&TokenKind::LParen) {
            let args = self.parse_call_args()?;
            return Ok(Expression::Call(CallExpression {
                callee: Box::new(Expression::Identifier(ident)),
                args,
            }));
        }

        // `ident { body }` → component expression, no constructor.
        // Convention: component type names always start uppercase; lowercase = variable.
        // This prevents `if flag { ... }` from consuming `flag {` as a component expression.
        let is_component_type = ident
            .0
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false);
        if is_component_type && matches!(self.peek_kind(), TokenKind::LBrace) {
            let body = self.parse_block_statement()?;
            return Ok(Expression::Component(ComponentExpression {
                component_type: ident,
                constructors: vec![],
                body,
            }));
        }

        // bare identifier
        Ok(Expression::Identifier(ident))
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
        self.tokens
            .get(self.pos)
            .map(|t| &t.kind)
            .unwrap_or(&TokenKind::Eof)
    }

    fn is_eof(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    fn err(&self, message: &str) -> ParseError {
        let span = self
            .tokens
            .get(self.pos)
            .map(|t| t.span.clone())
            .unwrap_or(Span::new(0, 0));
        ParseError {
            message: message.to_string(),
            token_index: self.pos,
            span,
        }
    }
}
