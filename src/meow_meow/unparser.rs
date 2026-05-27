// MMS AST → source text.
//
// Pure function over the AST defined in `ast.rs`. No I/O, no eval.
// Used by the component serialization pipeline (subtree → CE AST → text)
// and by the REPL `dump` command. Round-trip property:
//
//     parse(src) == parse(unparse(parse(src)))
//
// Exact textual round-trip is NOT a goal — comments and whitespace are not
// preserved by the parser, so they can't be reproduced. Structural equality
// of the AST after a re-parse is the invariant.

use crate::meow_meow::ast::{
    AssignmentStatement, BinOpKind, BlockStatement, CallExpression,
    ComponentExpression, ConstructorCall, ElseBranch, Expression, Ident, IfStatement, ImportItem,
    ReturnStatement, Statement, UnaryOpKind,
};
use crate::meow_meow::token::Unit;

const INDENT_STEP: usize = 4;

pub fn unparse_program(stmts: &[Statement]) -> String {
    let mut out = String::new();
    let mut p = Printer { out: &mut out, indent: 0 };
    for (i, s) in stmts.iter().enumerate() {
        if i > 0 {
            p.out.push('\n');
        }
        p.write_indent();
        p.statement(s);
        p.out.push('\n');
    }
    out
}

pub fn unparse_expression(e: &Expression) -> String {
    let mut out = String::new();
    let mut p = Printer { out: &mut out, indent: 0 };
    p.expression(e);
    out
}

pub fn unparse_component(ce: &ComponentExpression) -> String {
    let mut out = String::new();
    let mut p = Printer { out: &mut out, indent: 0 };
    p.component(ce);
    out
}

struct Printer<'a> {
    out: &'a mut String,
    indent: usize,
}

impl<'a> Printer<'a> {
    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.out.push(' ');
        }
    }

    // ---- statements -----------------------------------------------------

    fn statement(&mut self, s: &Statement) {
        match s {
            Statement::Assignment(a) => self.assignment(a),
            Statement::Reassign { name, value } => {
                self.out.push_str(&name.0);
                self.out.push_str(" = ");
                self.expression(value);
            }
            Statement::Return(ReturnStatement { value }) => {
                self.out.push_str("return");
                if let Some(v) = value {
                    self.out.push(' ');
                    self.expression(v);
                }
            }
            Statement::If(i) => self.if_stmt(i),
            Statement::Block(b) => self.block(b),
            Statement::Expression(e) => self.expression(e),
            Statement::ForIn { binding, iterable, body } => {
                self.out.push_str("for ");
                self.out.push_str(&binding.0);
                self.out.push_str(" in ");
                self.expression(iterable);
                self.out.push(' ');
                self.block(body);
            }
            Statement::While { condition, body } => {
                self.out.push_str("while ");
                self.expression(condition);
                self.out.push(' ');
                self.block(body);
            }
            Statement::Break => self.out.push_str("break"),
            Statement::Continue => self.out.push_str("continue"),
            Statement::Import { items, path } => {
                self.out.push_str("import { ");
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.out.push_str(", ");
                    }
                    self.import_item(item);
                }
                self.out.push_str(" } from ");
                self.string_literal(path);
            }
        }
    }

    fn assignment(&mut self, a: &AssignmentStatement) {
        if a.exported {
            self.out.push_str("export ");
        }
        // `fn name(...) { ... }` is the natural form when assigning a Function.
        if let Expression::Function { params, body } = &a.value {
            self.out.push_str("fn ");
            self.out.push_str(&a.name.0);
            self.params_and_body(params, body);
            return;
        }
        self.out.push_str("let ");
        self.out.push_str(&a.name.0);
        self.out.push_str(" = ");
        self.expression(&a.value);
    }

    fn if_stmt(&mut self, i: &IfStatement) {
        self.out.push_str("if ");
        self.expression(&i.condition);
        self.out.push(' ');
        self.block(&i.then_branch);
        if let Some(e) = &i.else_branch {
            self.out.push_str(" else ");
            self.else_branch(e);
        }
    }

    fn else_branch(&mut self, e: &ElseBranch) {
        match e {
            ElseBranch::Block(block) => self.block(block),
            ElseBranch::If(next_if) => self.if_stmt(next_if),
        }
    }

    fn import_item(&mut self, item: &ImportItem) {
        match item {
            ImportItem::Named(n) => self.out.push_str(&n.0),
            ImportItem::NamedAlias { name, alias } => {
                self.out.push_str(&name.0);
                self.out.push_str(" as ");
                self.out.push_str(&alias.0);
            }
            ImportItem::PositionalAlias { index, alias } => {
                self.out.push_str(&index.to_string());
                self.out.push_str(" as ");
                self.out.push_str(&alias.0);
            }
        }
    }

    fn block(&mut self, b: &BlockStatement) {
        if b.statements.is_empty() {
            self.out.push_str("{}");
            return;
        }
        self.out.push('{');
        self.out.push('\n');
        self.indent += INDENT_STEP;
        for s in &b.statements {
            self.write_indent();
            self.statement(s);
            self.out.push('\n');
        }
        self.indent -= INDENT_STEP;
        self.write_indent();
        self.out.push('}');
    }

    fn params_and_body(&mut self, params: &[Ident], body: &BlockStatement) {
        self.out.push('(');
        for (i, p) in params.iter().enumerate() {
            if i > 0 {
                self.out.push_str(", ");
            }
            self.out.push_str(&p.0);
        }
        self.out.push_str(") ");
        self.block(body);
    }

    // ---- expressions ----------------------------------------------------

    fn expression(&mut self, e: &Expression) {
        match e {
            Expression::String(s) => self.string_literal(s),
            Expression::Number(n) => self.number(*n),
            Expression::Dimension(n, unit) => {
                self.number(*n);
                self.out.push_str(unit_suffix(*unit));
            }
            Expression::Bool(b) => self.out.push_str(if *b { "true" } else { "false" }),
            Expression::Null => self.out.push_str("null"),
            Expression::Identifier(Ident(name)) => self.out.push_str(name),
            Expression::Array(items) => {
                self.out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.out.push_str(", ");
                    }
                    self.expression(item);
                }
                self.out.push(']');
            }
            Expression::Call(c) => self.call(c),
            Expression::Component(c) => self.component(c),
            Expression::BinaryOp { op, lhs, rhs } => self.binop(op, lhs, rhs),
            Expression::UnaryOp { op, operand } => {
                let sym = match op {
                    UnaryOpKind::Neg => "-",
                    UnaryOpKind::Not => "!",
                };
                self.out.push_str(sym);
                self.atom(operand);
            }
            Expression::Function { params, body } => {
                self.out.push_str("fn");
                self.params_and_body(params, body);
            }
        }
    }

    /// Print an expression as an atom: parenthesize if it could parse as
    /// multiple tokens at the top level (binops, unary, function literal).
    fn atom(&mut self, e: &Expression) {
        match e {
            Expression::BinaryOp { .. }
            | Expression::UnaryOp { .. }
            | Expression::Function { .. } => {
                self.out.push('(');
                self.expression(e);
                self.out.push(')');
            }
            _ => self.expression(e),
        }
    }

    fn call(&mut self, c: &CallExpression) {
        // Method-call form: callee is `lhs . method_ident`.
        if let Expression::BinaryOp { op: BinOpKind::Dot, lhs, rhs } = &*c.callee {
            if let Expression::Identifier(Ident(method)) = &**rhs {
                self.atom(lhs);
                self.out.push('.');
                self.out.push_str(method);
                self.args(&c.args);
                return;
            }
        }
        self.atom(&c.callee);
        self.args(&c.args);
    }

    fn args(&mut self, args: &[Expression]) {
        self.out.push('(');
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                self.out.push_str(", ");
            }
            self.expression(a);
        }
        self.out.push(')');
    }

    fn binop(&mut self, op: &BinOpKind, lhs: &Expression, rhs: &Expression) {
        if matches!(op, BinOpKind::Dot) {
            self.atom(lhs);
            self.out.push('.');
            self.atom(rhs);
            return;
        }
        let sym = binop_sym(op);
        self.atom(lhs);
        self.out.push(' ');
        self.out.push_str(sym);
        self.out.push(' ');
        self.atom(rhs);
    }

    fn component(&mut self, ce: &ComponentExpression) {
        self.out.push_str(&ce.component_type.0);
        for ctor in &ce.constructors {
            self.constructor(ctor);
        }
        // Empty body: omit braces when there's at least one constructor
        // (e.g. `R.cube()`). With no constructors and no body, still emit
        // `Name {}` so the re-parse keeps it as a ComponentExpression rather
        // than a bare Identifier.
        if ce.body.statements.is_empty() {
            if ce.constructors.is_empty() {
                self.out.push_str(" {}");
            }
            return;
        }
        self.out.push(' ');
        self.block(&ce.body);
    }

    fn constructor(&mut self, c: &ConstructorCall) {
        self.out.push('.');
        self.out.push_str(&c.method.0);
        self.args(&c.args);
    }

    // ---- literals -------------------------------------------------------

    fn number(&mut self, n: f64) {
        if !n.is_finite() {
            // NaN / Inf can't round-trip through the tokenizer. Emit a
            // best-effort form; tests should never produce these.
            self.out.push_str(&format!("{n}"));
            return;
        }
        let s = format!("{n}");
        // Force `.0` so the literal stays unambiguously a float and lines
        // up with the style used in examples (`1.0`, `0.85`).
        if !s.contains('.') && !s.contains('e') && !s.contains('E') {
            self.out.push_str(&s);
            self.out.push_str(".0");
        } else {
            self.out.push_str(&s);
        }
    }

    fn string_literal(&mut self, s: &str) {
        self.out.push('"');
        for ch in s.chars() {
            match ch {
                '\\' => self.out.push_str("\\\\"),
                '"' => self.out.push_str("\\\""),
                '\n' => self.out.push_str("\\n"),
                '\r' => self.out.push_str("\\r"),
                '\t' => self.out.push_str("\\t"),
                c => self.out.push(c),
            }
        }
        self.out.push('"');
    }
}

fn binop_sym(op: &BinOpKind) -> &'static str {
    match op {
        BinOpKind::Add => "+",
        BinOpKind::Sub => "-",
        BinOpKind::Mul => "*",
        BinOpKind::Div => "/",
        BinOpKind::Rem => "%",
        BinOpKind::Eq => "==",
        BinOpKind::NotEq => "!=",
        BinOpKind::Lt => "<",
        BinOpKind::Gt => ">",
        BinOpKind::LtEq => "<=",
        BinOpKind::GtEq => ">=",
        BinOpKind::And => "&&",
        BinOpKind::Or => "||",
        BinOpKind::Pipe => "|>",
        BinOpKind::Query => "->",
        BinOpKind::Dot => ".",
    }
}

fn unit_suffix(u: Unit) -> &'static str {
    match u {
        Unit::Percent => "%",
        Unit::GlyphUnits => "gu",
        Unit::WorldUnits => "wu",
        Unit::Degrees => "deg",
        Unit::Radians => "rad",
    }
}

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meow_meow::parser::MeowMeowParser;
    use crate::meow_meow::tokenizer::MeowMeowTokenizer;

    fn parse(src: &str) -> Vec<Statement> {
        let tokens = MeowMeowTokenizer::new(src)
            .tokenize()
            .expect("tokenize failed");
        MeowMeowParser::new(tokens)
            .parse_program()
            .expect("parse failed")
    }

    fn round_trip(src: &str) {
        let ast1 = parse(src);
        let text = unparse_program(&ast1);
        let ast2 = match std::panic::catch_unwind(|| parse(&text)) {
            Ok(a) => a,
            Err(_) => panic!("re-parse panicked. Unparsed text was:\n{text}"),
        };
        assert_eq!(
            ast1, ast2,
            "AST changed after round trip.\n--- original ---\n{src}\n--- unparsed ---\n{text}\n"
        );
    }

    #[test]
    fn round_trip_cat_mms() {
        let src = include_str!("../../examples/cat.mms");
        round_trip(src);
    }

    #[test]
    fn round_trip_minimal_component() {
        round_trip("T {}");
    }

    #[test]
    fn round_trip_builder_chain() {
        round_trip("T.position(0.0, 1.0, -2.5).scale(0.5, 0.5, 0.5) { R.cube() }");
    }

    #[test]
    fn round_trip_let_and_arith() {
        round_trip("let x = 1.0 + 2.0 * 3.0\nlet y = -x");
    }

    #[test]
    fn round_trip_function() {
        round_trip("fn double(n) { return n * 2.0 }\nlet four = double(2.0)");
    }

    #[test]
    fn round_trip_for_loop() {
        round_trip("let sum = 0.0\nfor i in [1.0, 2.0, 3.0] { sum = sum + i }");
    }

    #[test]
    fn round_trip_if_else() {
        round_trip("if true { let a = 1.0 } else { let b = 2.0 }");
    }

    #[test]
    fn round_trip_import() {
        round_trip("import { foo, bar as baz, 0 as cat } from \"cat.mms\"");
    }

    #[test]
    fn round_trip_dimensions() {
        round_trip("let a = 50%\nlet b = 20gu\nlet c = 30deg\nlet d = 0.5rad");
    }

    #[test]
    fn round_trip_strings_with_escapes() {
        round_trip("let s = \"hello \\\"world\\\"\\nnext line\"");
    }

    #[test]
    fn negative_number_idempotent() {
        // `-0.22` may parse as UnaryOp(Neg, 0.22). Round-trip should be stable.
        round_trip("T.position(-0.22, 1.38, 0.52) {}");
    }
}
