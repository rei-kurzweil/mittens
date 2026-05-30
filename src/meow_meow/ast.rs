// All AST types in one place to avoid circular module dependencies.
// `Expression::Function` contains `BlockStatement` which contains `Vec<Statement>` which
// contains `Expression` — putting them in separate files would create a true circular import.

#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ident(pub String);

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    String(String),
    Number(f64),
    /// Numeric literal with a unit suffix in source (e.g. `50%`, `20gu`,
    /// `30deg`). Evaluator materializes as `Value::Dimension`; consumers
    /// like the Style setters convert to `SizeDimension`.
    Dimension(f64, crate::meow_meow::token::Unit),
    Bool(bool),
    Null,
    Identifier(Ident),
    Array(Vec<Expression>),
    Index { base: Box<Expression>, index: Box<Expression> },
    Call(CallExpression),
    Component(ComponentExpression),
    BinaryOp { op: BinOpKind, lhs: Box<Expression>, rhs: Box<Expression> },
    UnaryOp { op: UnaryOpKind, operand: Box<Expression> },
    Function { params: Vec<Ident>, body: BlockStatement },
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinOpKind {
    Add, Sub, Mul, Div, Rem,
    Eq, NotEq, Lt, Gt, LtEq, GtEq,
    And, Or,
    Pipe,  // |> forward pipe (function application)
    Query, // -> component query / dispatch
    Dot,   // obj.method(args) — method receiver
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOpKind {
    Neg,
    Not,
}

/// A free-standing function call: `foo(a, b)` or a method call `obj.method(a, b)`.
///
/// For plain calls the callee is `Expression::Identifier`.
/// For method calls the callee is `Expression::BinaryOp { op: BinOpKind::Dot, .. }`.
#[derive(Debug, Clone, PartialEq)]
pub struct CallExpression {
    pub callee: Box<Expression>,
    pub args: Vec<Expression>,
}

/// A constructor or chained builder call on a component expression header.
///
/// `T.position(x, y, z).scale(a, b, c)` produces two `ConstructorCall`s:
/// `[{ method: "position", args: [x,y,z] }, { method: "scale", args: [a,b,c] }]`
///
/// The first entry is the "primary" constructor (selects the component variant).
/// Subsequent entries are chained builder calls applied after creation.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstructorCall {
    pub method: Ident,
    pub args: Vec<Expression>,
}

/// A component expression: the declarative tree-building form.
///
/// `ComponentType.method(args)[.method2(args2)...] { body }`
///
/// - `T { ... }` — no constructor, has body
/// - `R.cube()` — one constructor, no body
/// - `T.position(x,y,z).scale(a,b,c) { C {} }` — two constructors + body
///
/// The body is a plain `BlockStatement`: all MMS language features are
/// available inside it. CE emissions inside the body become children of this
/// node; builder calls (identifiers not in env) configure this component.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentExpression {
    pub component_type: Ident,
    /// All chained constructor calls from the header (before `{`), in order.
    /// Empty when there is no `.method(...)` on the type name.
    pub constructors: Vec<ConstructorCall>,
    pub body: BlockStatement,
}

// ---------------------------------------------------------------------------
// Statements
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct BlockStatement {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Assignment(AssignmentStatement),
    /// `x = expr` — mutate an existing binding. Distinct from `Assignment` (`let x = expr`).
    ///
    /// Scope note (v1): reassignment updates the binding in the current block's local env.
    /// It does NOT propagate outward to enclosing scopes — that requires a scope chain
    /// (deferred). Inside a `for` loop body the loop's accumulated env is used, so
    /// accumulator patterns (`sum = sum + i`) work correctly within the loop.
    Reassign { name: Ident, value: Expression },
    Return(ReturnStatement),
    If(IfStatement),
    Block(BlockStatement),
    Expression(Expression),
    ForIn { binding: Ident, iterable: Expression, body: BlockStatement },
    While { condition: Expression, body: BlockStatement },
    Break,
    Continue,
    Import { items: Vec<ImportItem>, path: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssignmentStatement {
    pub name: Ident,
    pub value: Expression,
    /// `true` when declared with `export let` / `export fn`.
    pub exported: bool,
}

/// One item in an `import { ... } from "..."` list.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportItem {
    /// `{ name }` — import a named export.
    Named(Ident),
    /// `{ name as alias }` — import a named export under a different local name.
    NamedAlias { name: Ident, alias: Ident },
    /// `{ 0 as alias }` — import the Nth root CE emit, bound to `alias`.
    PositionalAlias { index: usize, alias: Ident },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReturnStatement {
    pub value: Option<Expression>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfStatement {
    pub condition: Expression,
    pub then_branch: BlockStatement,
    pub else_branch: Option<ElseBranch>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ElseBranch {
    Block(BlockStatement),
    If(Box<IfStatement>),
}
