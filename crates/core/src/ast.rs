use crate::environment::Environment;
use crate::Token;
use std::cell::RefCell;
use std::fmt;
use std::sync::Arc;
use std::rc::Rc;

pub type Program = Vec<Stmt>;

#[derive(Debug, Clone)]
pub enum Stmt {
    Expression(Expr),
    Let {
        name: Token,
        ty: Option<String>,
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
        body: Rc<Stmt>,
    },
    Return {
        value: Option<Expr>,
    },
// ...existing code...
}

#[derive(Debug, Clone)]
pub struct FunctionParam {
    pub name: Token,
    pub ty: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
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
        name: Option<Token>,
        variant: Token,
        values: Vec<Expr>,
    },
    FieldAccess {
        object: Box<Expr>,
        field: Token,
    },
    Try(Box<Expr>),
    Array(Vec<Expr>),
    Cast {
        object: Box<Expr>,
        target_type: String,
    },
    InterpolatedString(Vec<InterpolatedPart>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum InterpolatedPart {
    Literal(String),
    Expr(Box<Expr>),
}

#[derive(Clone)]
pub struct Function {
    pub name: Option<String>,
    pub params: Vec<FunctionParam>,
    pub body: Rc<Stmt>,
    pub closure: Rc<RefCell<Environment>>,
}

impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self.name.as_deref().unwrap_or("<anonymous>");
        write!(f, "<fn {}>", name)
    }
}

impl PartialEq for Function {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.body, &other.body)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArtValue {
    Int(i64),
    Float(f64),
    String(Arc<str>),
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
    Function(Rc<Function>),
    Builtin(BuiltinFn),
}

#[derive(Clone)]
pub enum BuiltinFn {
    Println,
}

impl fmt::Debug for BuiltinFn { fn fmt(&self, f:&mut fmt::Formatter<'_>)->fmt::Result { match self { BuiltinFn::Println => write!(f, "<builtin println>") } } }

impl PartialEq for BuiltinFn { fn eq(&self, other:&Self)->bool { matches!((self,other),(BuiltinFn::Println,BuiltinFn::Println)) } }

impl fmt::Display for ArtValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArtValue::Int(n) => write!(f, "{}", n),
            ArtValue::Float(n) => write!(f, "{}", n),
            ArtValue::String(s) => write!(f, "{}", s),
            ArtValue::Bool(b) => write!(f, "{}", b),
            ArtValue::Optional(opt) => match &**opt {
                Some(val) => write!(f, "Some({})", val),
                None => write!(f, "None"),
            },
            ArtValue::Array(arr) => {
                let elems: Vec<String> = arr.iter().map(|item| item.to_string()).collect();
                write!(f, "[{}]", elems.join(", "))
            }
            ArtValue::StructInstance { struct_name, fields } => {
                let field_strs: Vec<String> =
                    fields.iter().map(|(k, v)| format!("{}: {}", k, v)).collect();
                write!(f, "{} {{ {} }}", struct_name, field_strs.join(", "))
            }
            ArtValue::EnumInstance { enum_name, variant, values } => {
                if values.is_empty() {
                    write!(f, "{}.{}", enum_name, variant)
                } else {
                    let value_strs: Vec<String> = values.iter().map(|v| v.to_string()).collect();
                    write!(f, "{}.{}({})", enum_name, variant, value_strs.join(", "))
                }
            }
            ArtValue::Function(func) => {
                let name = func.name.as_deref().unwrap_or("<anonymous>");
                write!(f, "<fn {}>", name)
            }
            ArtValue::Builtin(b) => match b { BuiltinFn::Println => write!(f, "<builtin println>") },
        }
    }
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

impl ArtValue {
    #[inline]
    pub fn none() -> ArtValue { ArtValue::Optional(Box::new(None)) }
}

#[derive(Debug, Clone)]
pub enum MatchPattern {
    EnumVariant {
        variant: Token,
        params: Option<Vec<MatchPattern>>,
    },
    Literal(ArtValue),
    Variable(Token),
    Binding(Token),
    Wildcard,
}