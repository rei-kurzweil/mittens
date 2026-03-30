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
    Bool(bool),
    Null,
    Identifier(Ident),
    Array(Vec<Expression>),
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
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOpKind {
    Neg,
    Not,
}

/// A free-standing function call: `foo(a, b)`.
#[derive(Debug, Clone, PartialEq)]
pub struct CallExpression {
    pub callee: Ident,
    pub args: Vec<Expression>,
}

/// The optional `.method(args)` immediately after the component type name.
///
/// Selects a named constructor or initial configuration for the component:
/// `T.with_scale(1, 2, 3) { ... }` → `constructor = Some(ConstructorCall { method: "with_scale", args: [...] })`
/// `Renderable.cube()` → `constructor = Some(ConstructorCall { method: "cube", args: [] })`
#[derive(Debug, Clone, PartialEq)]
pub struct ConstructorCall {
    pub method: Ident,
    pub args: Vec<Expression>,
}

/// A single item inside a component body, in source order.
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentBodyItem {
    /// `name = expr` — sets a named property on the component being constructed.
    NamedAssignment { name: Ident, value: Expression },
    /// `ident(args)` — a builder/method call applied to the component.
    Call(CallExpression),
    /// A nested component expression — becomes a child in the tree.
    Child(ComponentExpression),
    /// A bare literal, identifier, or array — positional argument.
    Positional(Expression),
}

/// A component expression: the declarative tree-building form.
///
/// `ComponentType.head_method(args) { body_items... }`
///
/// The braces and head call are both optional:
/// - `T { ... }` — no head call, has body
/// - `Color.rgba(1, 0, 0, 1)` — head call, no braces
/// - `T.with_scale(1, 2, 3) { C {} }` — head call + body
/// - `TransformMapTranslation {}` — no head call, empty body
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentExpression {
    pub component_type: Ident,
    pub constructor: Option<ConstructorCall>,
    pub body: Vec<ComponentBodyItem>,
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
    pub else_branch: Option<BlockStatement>,
}
