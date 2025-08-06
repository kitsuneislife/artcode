
use crate::Token;

pub type Program = Vec<Stmt>;

#[derive(Debug, Clone)]
pub enum Stmt {
    Expression(Expr),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Binary {
        left: Box<Expr>,
        operator: Token,
        right: Box<Expr>,
    },
    Grouping {
        expression: Box<Expr>,
    },
    Literal(LiteralValue),
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
    },
    Variable {
        name: Token,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    String(String),
    Number(f64),
}
