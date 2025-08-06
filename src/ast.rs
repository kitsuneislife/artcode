
use crate::lexer::Token;

pub type Program = Vec<Stmt>;

#[derive(Debug, Clone)]
pub enum Stmt {
    Expression(Expr),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(LiteralValue),
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
    },
    Variable {
        name: Token,
    },
}

#[derive(Debug, Clone)]
pub enum LiteralValue {
    String(String),
}
