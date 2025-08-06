use crate::Token;

pub type Program = Vec<Stmt>;

#[derive(Debug, Clone)]
pub enum Stmt {
    Expression(Expr),
    Let {
        name: Token,
        initializer: Expr,
    },
    Block {
        statements: Vec<Stmt>,
    },
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    StructDecl {
        name: Token,
        fields: Vec<(Token, String)>,
    },
    EnumDecl {
        name: Token,
        variants: Vec<(Token, Option<Vec<String>>)>,
    },
    Match {
        expr: Expr,
        cases: Vec<(MatchPattern, Stmt)>,
    },
    Function {
        name: Token,
        params: Vec<FunctionParam>,
        return_type: Option<String>,
        body: Box<Stmt>,
    },
    Return {
        value: Option<Expr>,
    },
}

#[derive(Debug, Clone)]
pub struct FunctionParam {
    pub name: Token,
    pub ty: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Binary {
        left: Box<Expr>,
        operator: Token,
        right: Box<Expr>,
    },
    Logical {
        left: Box<Expr>,
        operator: Token,
        right: Box<Expr>,
    },
    Unary {
        operator: Token,
        right: Box<Expr>,
    },
    Grouping {
        expression: Box<Expr>,
    },
    Literal(ArtValue),
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
    },
    Variable {
        name: Token,
    },
    StructInit {
        name: Token,
        fields: Vec<(Token, Expr)>,
    },
    EnumInit {
        name: Token,
        variant: Token,
        values: Vec<Expr>,
    },
    FieldAccess {
        object: Box<Expr>,
        field: Token,
    },
    Try(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArtValue {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Optional(Box<Option<ArtValue>>),
    Array(Vec<ArtValue>),
    StructInstance {
        struct_name: String,
        fields: std::collections::HashMap<String, ArtValue>,
    },
    EnumInstance {
        enum_name: String,
        variant: String,
        values: Vec<ArtValue>,
    },
}

impl From<bool> for ArtValue {
    fn from(b: bool) -> Self {
        ArtValue::Bool(b)
    }
}

impl From<f64> for ArtValue {
    fn from(n: f64) -> Self {
        ArtValue::Float(n)
    }
}

impl From<i64> for ArtValue {
    fn from(n: i64) -> Self {
        ArtValue::Int(n)
    }
}

#[derive(Debug, Clone)]
pub enum MatchPattern {
    EnumVariant {
        variant: Token,
        params: Option<Vec<Token>>,
    },
    Literal(ArtValue),
    Variable(Token),
    Wildcard,
}