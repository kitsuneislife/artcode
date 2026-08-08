//! Minimal IR crate for Artcode
//! Provides a tiny textual emitter for a subset of IR used by the RFC.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    I64,
    F64,
    Void,
}

/// Integer comparison predicates used by [`Instr::ICmp`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpPred {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

impl CmpPred {
    /// Symbol used in the textual IR (`emit_text`).
    pub fn symbol(self) -> &'static str {
        match self {
            CmpPred::Lt => "lt",
            CmpPred::Le => "le",
            CmpPred::Gt => "gt",
            CmpPred::Ge => "ge",
            CmpPred::Eq => "eq",
            CmpPred::Ne => "ne",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instr {
    ConstI64(String, i64),       // name, value
    Add(String, String, String), // dest, a, b
    Sub(String, String, String),
    Mul(String, String, String),
    Div(String, String, String),
    ICmp(String, CmpPred, String, String), // dest (0/1), pred, a, b
    Call(String, String, Vec<String>),     // dest, fn, args
    Alloca(String),                        // slot — stack-allocated i64 cell
    Load(String, String),                  // dest, slot
    Store(String, String),                 // slot, value
    Label(String),
    Br(String),
    BrCond(String, String, String), // pred, if_true, if_false
    Phi(String, Type, Vec<(String, String)>), // dest, type, [(val, bb)]
    Ret(Option<String>),
    Deopt,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub ret: Option<Type>,
    pub body: Vec<Instr>,
}

pub mod c_emitter;
pub mod llvm_emitter;
pub mod lower_fn;
pub mod lowering;
pub mod ssa;

// AOT tooling, absorbed from the former `jit` crate. These operate on emitted
// textual IR rather than on the AST, so they belong next to the emitters.
pub mod analyzer;
pub mod cache;
pub mod loader;
pub mod trampolines;

// Keep existing name `lower_stmt` exported; if the module implements fallback
// we re-export the top-level dispatcher.
pub use cache::ArtCache;
pub use lowering::lower_stmt;
pub use trampolines::{call_jit_fn, Sig};

/// Parses the signature out of textual IR: `func @name(params) -> ret`.
///
/// Returns the parameter count and the return type. Callers use it to check
/// that generated code matches the native ABI they are about to call through,
/// which prevents transmuting a function pointer to an incompatible prototype.
pub fn parse_ir_signature(ir_text: &str) -> Result<(usize, String), String> {
    let idx = ir_text.find("func @").ok_or("missing 'func @' prefix")?;
    let after = &ir_text[idx + "func @".len()..];

    let open = after.find('(').ok_or("missing '(' in signature")?;
    if after[..open].trim().is_empty() {
        return Err("empty function name".to_string());
    }

    let rest = &after[open + 1..];
    let close = rest.find(')').ok_or("missing ')' in signature")?;
    let params = rest[..close].trim();
    let param_count = if params.is_empty() {
        0
    } else {
        params.split(',').count()
    };

    let after_close = &rest[close + 1..];
    let arrow = after_close.find("->").ok_or("missing '->' return type")?;
    let ret_ty = after_close[arrow + 2..]
        .split_whitespace()
        .next()
        .ok_or("missing return type")?;

    Ok((param_count, ret_ty.to_string()))
}

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
        let params: Vec<String> = self
            .params
            .iter()
            .map(|(n, t)| format!("{} {}", t, n))
            .collect();
        // Build body text first
        let mut body = String::new();
        let mut printed_label = false;
        for instr in &self.body {
            match instr {
                Instr::Label(l) => {
                    body.push_str(&format!("{}:\n", l));
                    printed_label = true;
                }
                Instr::ConstI64(name, v) => {
                    body.push_str(&format!("  {} = const i64 {}\n", name, v))
                }
                Instr::Add(dest, a, b) => {
                    body.push_str(&format!("  {} = add i64 {}, {}\n", dest, a, b))
                }
                Instr::Sub(dest, a, b) => {
                    body.push_str(&format!("  {} = sub i64 {}, {}\n", dest, a, b))
                }
                Instr::Mul(dest, a, b) => {
                    body.push_str(&format!("  {} = mul i64 {}, {}\n", dest, a, b))
                }
                Instr::Div(dest, a, b) => {
                    body.push_str(&format!("  {} = div i64 {}, {}\n", dest, a, b))
                }
                Instr::ICmp(dest, pred, a, b) => body.push_str(&format!(
                    "  {} = icmp {} i64 {}, {}\n",
                    dest,
                    pred.symbol(),
                    a,
                    b
                )),
                Instr::Alloca(slot) => body.push_str(&format!("  {} = alloca i64\n", slot)),
                Instr::Load(dest, slot) => {
                    body.push_str(&format!("  {} = load i64 {}\n", dest, slot))
                }
                Instr::Store(slot, val) => {
                    body.push_str(&format!("  store i64 {}, {}\n", val, slot))
                }
                Instr::Call(dest, fnname, args) => body.push_str(&format!(
                    "  {} = call {}({})\n",
                    dest,
                    fnname,
                    args.join(", ")
                )),
                Instr::Br(label) => body.push_str(&format!("  br {}\n", label)),
                Instr::BrCond(pred, t, f) => {
                    body.push_str(&format!("  br_cond {}, {}, {}\n", pred, t, f))
                }
                Instr::Phi(dest, ty, pairs) => {
                    let parts: Vec<String> = pairs
                        .iter()
                        .map(|(v, bb)| format!("[ {}, {} ]", v, bb))
                        .collect();
                    body.push_str(&format!("  {} = phi {} {}\n", dest, ty, parts.join(", ")))
                }
                Instr::Ret(Some(v)) => body.push_str(&format!("  ret {}\n", v)),
                Instr::Ret(None) => body.push_str("  ret\n"),
                Instr::Deopt => body.push_str("  deopt\n"),
            }
        }

        // header string used for function emission
        let header = format!(
            "func @{}({}) -> {} {{\n",
            self.name,
            params.join(", "),
            self.ret
                .as_ref()
                .map(|t| t.to_string())
                .unwrap_or_else(|| "void".to_string())
        );
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
