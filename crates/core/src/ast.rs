use crate::Token;
use crate::environment::Environment;
use std::cell::RefCell;
use std::fmt;
use std::rc::{Rc, Weak};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct ValueEnvelope {
    pub sender: Option<u32>,
    pub payload: ArtValue,
    pub priority: i32,
}

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
        cases: Vec<(MatchPattern, Option<Expr>, Stmt)>, // (pattern, guard, body)
    },
    Function {
        name: Token,
        params: Vec<FunctionParam>,
        return_type: Option<String>,
        body: Rc<Stmt>,
        method_owner: Option<String>, // novo: tipo ao qual o método pertence
    },
    Performant {
        statements: Vec<Stmt>,
    }, // Note: `Performant` será tratado no interpretador como um bloco léxico que cria uma arena
    // de memória; por ora é apenas parseable e no interpretador cria um identificador de arena
    // para alocações dentro do bloco.
    Return {
        value: Option<Expr>,
    },
    SpawnActor {
        body: Vec<Stmt>,
    },
    Import {
        path: Vec<Token>,
    },
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
    Weak(Box<Expr>),          // açúcar: weak expr -> builtin weak()
    Unowned(Box<Expr>),       // açúcar: unowned expr -> builtin unowned()
    WeakUpgrade(Box<Expr>),   // açúcar: expr?  (onde expr avalia para WeakRef)
    UnownedAccess(Box<Expr>), // açúcar: expr! (onde expr avalia para UnownedRef)
}

#[derive(Debug, Clone, PartialEq)]
pub enum InterpolatedPart {
    Literal(String),
    Expr {
        expr: Box<Expr>,
        format: Option<String>,
    },
}

#[derive(Clone)]
pub struct Function {
    pub name: Option<String>,
    pub params: Vec<FunctionParam>,
    pub body: Rc<Stmt>,
    // Ambiente léxico capturado (Weak para evitar ciclo Environment -> Function -> Environment)
    pub closure: Weak<RefCell<Environment>>,
    // Retentor opcional para ambientes criados ao "bindar" métodos (garante que não cai no nada imediatamente)
    pub retained_env: Option<Rc<RefCell<Environment>>>,
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
    // Novo: wrapper para objetos compostos alocados no heap (fase de transição ARC real)
    HeapComposite(ObjHandle),
    Function(Rc<Function>),
    Builtin(BuiltinFn),
    // Fase 8 (protótipo): referências não-fortes
    WeakRef(ObjHandle),    // id para registro global
    UnownedRef(ObjHandle), // id para registro global (não mantém vivo)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjHandle(pub u64);

#[derive(Clone)]
pub enum BuiltinFn {
    Println,
    Len,
    TypeOf,
    WeakNew,    // __weak(x)
    WeakGet,    // __weak_get(w)
    UnownedNew, // __unowned(x)
    UnownedGet, // __unowned_get(u)
    OnFinalize, // __on_finalize(comp, fn)
    EnvelopeNew, // envelope(sender, payload, priority)
    ActorSend,   // actor_send(actor, value)
    ActorReceive, // actor_receive()
    ActorReceiveEnvelope, // actor_receive_envelope()
    ActorYield, // actor_yield()
    ActorSetMailboxLimit, // actor_set_mailbox_limit(actor, limit)
}

impl fmt::Debug for BuiltinFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuiltinFn::Println => write!(f, "<builtin println>"),
            BuiltinFn::Len => write!(f, "<builtin len>"),
            BuiltinFn::TypeOf => write!(f, "<builtin type_of>"),
            BuiltinFn::WeakNew => write!(f, "<builtin __weak>"),
            BuiltinFn::WeakGet => write!(f, "<builtin __weak_get>"),
            BuiltinFn::UnownedNew => write!(f, "<builtin __unowned>"),
            BuiltinFn::UnownedGet => write!(f, "<builtin __unowned_get>"),
            BuiltinFn::OnFinalize => write!(f, "<builtin __on_finalize>"),
            BuiltinFn::EnvelopeNew => write!(f, "<builtin envelope>"),
            BuiltinFn::ActorSend => write!(f, "<builtin actor_send>"),
                BuiltinFn::ActorReceive => write!(f, "<builtin actor_receive>"),
                BuiltinFn::ActorReceiveEnvelope => write!(f, "<builtin actor_receive_envelope>"),
                BuiltinFn::ActorYield => write!(f, "<builtin actor_yield>"),
                BuiltinFn::ActorSetMailboxLimit => write!(f, "<builtin actor_set_mailbox_limit>"),
        }
    }
}

impl PartialEq for BuiltinFn {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

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
            ArtValue::StructInstance {
                struct_name,
                fields,
            } => {
                let field_strs: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                write!(f, "{} {{ {} }}", struct_name, field_strs.join(", "))
            }
            ArtValue::EnumInstance {
                enum_name,
                variant,
                values,
            } => {
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
            ArtValue::Builtin(b) => match b {
                BuiltinFn::Println => write!(f, "<builtin println>"),
                BuiltinFn::Len => write!(f, "<builtin len>"),
                BuiltinFn::TypeOf => write!(f, "<builtin type_of>"),
                BuiltinFn::WeakNew => write!(f, "<builtin __weak>"),
                BuiltinFn::WeakGet => write!(f, "<builtin __weak_get>"),
                BuiltinFn::UnownedNew => write!(f, "<builtin __unowned>"),
                BuiltinFn::UnownedGet => write!(f, "<builtin __unowned_get>"),
                BuiltinFn::OnFinalize => write!(f, "<builtin __on_finalize>"),
                BuiltinFn::EnvelopeNew => write!(f, "<builtin envelope>"),
                BuiltinFn::ActorSend => write!(f, "<builtin actor_send>"),
                BuiltinFn::ActorReceive => write!(f, "<builtin actor_receive>"),
                BuiltinFn::ActorReceiveEnvelope => write!(f, "<builtin actor_receive_envelope>"),
                BuiltinFn::ActorYield => write!(f, "<builtin actor_yield>"),
                BuiltinFn::ActorSetMailboxLimit => write!(f, "<builtin actor_set_mailbox_limit>"),
            },
            ArtValue::WeakRef(_) => write!(f, "<weak ref>"),
            ArtValue::UnownedRef(_) => write!(f, "<unowned ref>"),
            ArtValue::HeapComposite(_) => write!(f, "<composite>"),
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
    pub fn none() -> ArtValue {
        ArtValue::Optional(Box::new(None))
    }
}

#[derive(Debug, Clone)]
pub enum MatchPattern {
    EnumVariant {
        enum_name: Option<Token>, // Nome qualificado do enum (opcional para compatibilidade)
        variant: Token,
        params: Option<Vec<MatchPattern>>,
    },
    Literal(ArtValue),
    Variable(Token),
    Binding(Token),
    Wildcard,
}
