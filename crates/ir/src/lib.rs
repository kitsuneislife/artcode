//! Minimal IR crate for Artcode
//! Provides a tiny textual emitter for a subset of IR used by the RFC.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    I64,
    F64,
    Void,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instr {
    ConstI64(String, i64), // name, value
    Add(String, String), // %a + %b
    Sub(String, String),
    Mul(String, String),
    Div(String, String),
    Ret(Option<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub ret: Option<Type>,
    pub body: Vec<Instr>,
}

pub mod lowering;

// Keep existing name `lower_stmt` exported; if the module implements fallback
// we re-export the top-level dispatcher.
pub use lowering::lower_stmt;

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::I64 => write!(f, "i64"),
            Type::F64 => write!(f, "f64"),
            Type::Void => write!(f, "void"),
        }
    }
}

impl Function {
    pub fn emit_text(&self) -> String {
        let mut out = String::new();
        let params: Vec<String> = self.params.iter().map(|(n,t)| format!("{} {}", t, n)).collect();
    let header = format!("func @{}({}) -> {} {{\n", self.name, params.join(", "), self.ret.as_ref().map(|t| t.to_string()).unwrap_or_else(|| "void".to_string()));
        out.push_str(&header);
        out.push_str("  entry:\n");
        for instr in &self.body {
            match instr {
                Instr::ConstI64(name, v) => out.push_str(&format!("    const i64 {} {}\n", name, v)),
                Instr::Add(a,b) => out.push_str(&format!("    add i64 {}, {}\n", a, b)),
                Instr::Sub(a,b) => out.push_str(&format!("    sub i64 {}, {}\n", a, b)),
                Instr::Mul(a,b) => out.push_str(&format!("    mul i64 {}, {}\n", a, b)),
                Instr::Div(a,b) => out.push_str(&format!("    div i64 {}, {}\n", a, b)),
                Instr::Ret(Some(v)) => out.push_str(&format!("    ret {}\n", v)),
                Instr::Ret(None) => out.push_str("    ret\n"),
            }
        }
        out.push_str("}\n");
        out
    }
}
