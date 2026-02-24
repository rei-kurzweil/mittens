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

/// A function call expression like `with_occlusion_and_lighting()`.
#[derive(Debug, Clone, PartialEq)]
pub struct CallExpression {
    pub callee: Ident,
    pub args: Vec<Expression>,
}

/// A component expression is the declarative tree-building form.
///
/// It intentionally separates the three “kinds of nodes” you described:
/// - `parameters`: named attributes on the component header (`name="..."`, `guid="..."`)
/// - `calls`: function calls that run during component creation (`with_fps_rotation()`)
/// - `children`: nested component expressions
///
/// Plus one extra bucket:
/// - `positional`: unnamed / sugary parameters inside the component body (`TXT { "a", "b" }`, `QUAD_2D`)
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentExpression {
    pub component_type: Ident,
    pub parameters: Vec<Parameter>,
    pub positional: Vec<Expression>,
    pub calls: Vec<CallExpression>,
    pub children: Vec<ComponentExpression>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: Ident,
    pub value: Expression,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}
