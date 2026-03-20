#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ident(pub String);

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

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}
