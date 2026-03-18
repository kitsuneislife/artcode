#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Float,
    Bool,
    String,
    None,
    Array(Box<Type>),
    Struct(String),
    Enum(String),
    EnumInstance(String, Vec<Type>),
    GenericParam(String),
    Function(Vec<Type>, Box<Type>),
    Tuple(Vec<Type>),
    Unknown,
}

impl Type {
    pub fn name(&self) -> String {
        match self {
            Type::Int => "Int".into(),
            Type::Float => "Float".into(),
            Type::Bool => "Bool".into(),
            Type::String => "String".into(),
            Type::None => "None".into(),
            Type::Array(inner) => format!("[{}]", inner.name()),
            Type::Struct(s) => s.clone(),
            Type::Enum(e) => e.clone(),
            Type::EnumInstance(n, params) => {
                let ps: Vec<String> = params.iter().map(|p| p.name()).collect();
                format!("{}<{}>", n, ps.join(","))
            }
            Type::GenericParam(g) => g.clone(),
            Type::Function(params, ret) => {
                let ps: Vec<String> = params.iter().map(|p| p.name()).collect();
                format!("fn({}) -> {}", ps.join(", "), ret.name())
            }
            Type::Tuple(types) => {
                let ts: Vec<String> = types.iter().map(|t| t.name()).collect();
                format!("({})", ts.join(", "))
            }
            Type::Unknown => "_".into(),
        }
    }
}
