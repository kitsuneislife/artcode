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
    Add(String, String, String), // dest, a, b
    Sub(String, String, String),
    Mul(String, String, String),
    Div(String, String, String),
    Call(String, String, Vec<String>), // dest, fn, args
    Label(String),
    Br(String),
    BrCond(String, String, String), // pred, if_true, if_false
    Phi(String, Type, Vec<(String, String)>), // dest, type, [(val, bb)]
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
pub mod ssa;

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
        // Build body text first
        let mut body = String::new();
        let mut printed_label = false;
        for instr in &self.body {
            match instr {
                Instr::Label(l) => {
                    body.push_str(&format!("{}:\n", l));
                    printed_label = true;
                }
                Instr::ConstI64(name, v) => body.push_str(&format!("  {} = const i64 {}\n", name, v)),
                Instr::Add(dest,a,b) => body.push_str(&format!("  {} = add i64 {}, {}\n", dest, a, b)),
                Instr::Sub(dest,a,b) => body.push_str(&format!("  {} = sub i64 {}, {}\n", dest, a, b)),
                Instr::Mul(dest,a,b) => body.push_str(&format!("  {} = mul i64 {}, {}\n", dest, a, b)),
                Instr::Div(dest,a,b) => body.push_str(&format!("  {} = div i64 {}, {}\n", dest, a, b)),
                Instr::Call(dest,fnname,args) => body.push_str(&format!("  {} = call {}({})\n", dest, fnname, args.join(", "))),
                Instr::Br(label) => body.push_str(&format!("  br {}\n", label)),
                Instr::BrCond(pred,t,f) => body.push_str(&format!("  br_cond {}, {}, {}\n", pred, t, f)),
                Instr::Phi(dest, ty, pairs) => {
                    let parts: Vec<String> = pairs.iter().map(|(v,bb)| format!("[ {}, {} ]", v, bb)).collect();
                    body.push_str(&format!("  {} = phi {} {}\n", dest, ty, parts.join(", ")))
                }
                Instr::Ret(Some(v)) => body.push_str(&format!("  ret {}\n", v)),
                Instr::Ret(None) => body.push_str("  ret\n"),
            }
        }

    // header string used for function emission
    let header = format!("func @{}({}) -> {} {{\n", self.name, params.join(", "), self.ret.as_ref().map(|t| t.to_string()).unwrap_or_else(|| "void".to_string()));
        if printed_label {
            out.push_str(&header);
            out.push_str(&body);
            out.push_str("}\n");
            return out;
        }
        // No label emitted: add default entry label before body
        out.push_str(&header);
        out.push_str("  entry:\n");
        out.push_str(&body);
        out.push_str("}\n");
        out
    }
}
