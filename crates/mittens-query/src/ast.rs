#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QueryAst {
    pub selector_groups: Vec<SelectorSequence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectorSequence {
    pub segments: Vec<SelectorSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorSegment {
    pub combinator: Option<Combinator>,
    pub compound: CompoundSelector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combinator {
    Descendant,
    Child,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompoundSelector {
    pub simple_selectors: Vec<SimpleSelector>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimpleSelector {
    Universal,
    Type(String),
    Id(String),
    Guid(String),
    Name(String),
    Class(String),
    Attribute(AttributeSelector),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeSelector {
    pub name: String,
    pub value: Option<String>,
}
