use super::expression::{Expression, Ident};

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Assignment(AssignmentStatement),
    Return(ReturnStatement),
    If(IfStatement),
    Block(BlockStatement),
    Expression(Expression),
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssignmentStatement {
    pub name: Ident,
    pub value: Expression,
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

#[derive(Debug, Clone, PartialEq)]
pub struct BlockStatement {
    pub statements: Vec<Statement>,
}
