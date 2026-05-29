//! Textual LLVM IR backend for the Artcode IR.
//!
//! Emits valid LLVM IR (`.ll`) from a slice of [`Function`]s. The output is
//! consumed by `clang`/`llc` to produce native binaries — this keeps the AOT
//! pipeline free of the heavyweight `inkwell` dependency and decoupled from any
//! specific LLVM C-API version (works with whichever `clang` is on the PATH).
//!
//! The IR produced by the lowering pass is in SSA form for the currently
//! supported constructs (arithmetic, calls, `if`, `match`), so temporaries map
//! 1:1 onto LLVM SSA registers and `Instr::Phi` maps directly onto LLVM `phi`.

use crate::{Function, Instr, Type};
use std::collections::HashSet;
use std::fmt::Write as _;

/// C/LLVM-reserved entrypoint name collision is handled by renaming Artcode
/// `main` so the emitted module can define its own C-ABI `main` wrapper.
fn sanitize_fname(raw: &str) -> String {
    let name = raw.replace('@', "");
    if name == "main" {
        return "_art_main".to_string();
    }
    name
}

fn llvm_type(t: &Option<Type>) -> &'static str {
    match t {
        Some(Type::F64) => "double",
        Some(Type::Void) | None => "i64",
        _ => "i64",
    }
}

/// Render an operand: SSA temps keep their `%name`, integer literals are emitted
/// verbatim, and bare identifiers (function parameters) are prefixed with `%`.
fn operand(s: &str) -> String {
    // SSA temps already carry `%`; integer literals are emitted verbatim; bare
    // identifiers (function parameters) get a `%` prefix.
    if s.starts_with('%') || s.parse::<i64>().is_ok() {
        s.to_string()
    } else {
        format!("%{}", s)
    }
}

/// Emit a complete LLVM IR module for `funcs`. When `entry_func` matches one of
/// the functions, a C-ABI `main` wrapper is appended that calls it and prints
/// the result (mirroring the C backend's observable behaviour).
pub fn emit_llvm_module(funcs: &[Function], entry_func: &str) -> String {
    let mut out = String::new();
    out.push_str("; ModuleID = 'artcode'\n");
    out.push_str("@.fmt = private unnamed_addr constant [6 x i8] c\"%lld\\0A\\00\"\n");
    out.push_str("declare i32 @printf(ptr, ...)\n");
    out.push_str("declare void @llvm.trap()\n\n");

    // Forward-declare any callee that is not defined in this module.
    let defined: HashSet<String> = funcs.iter().map(|f| sanitize_fname(&f.name)).collect();
    let mut declared: HashSet<String> = HashSet::new();
    for f in funcs {
        for instr in &f.body {
            if let Instr::Call(_, target, args) = instr {
                let name = sanitize_fname(target);
                if !defined.contains(&name) && declared.insert(name.clone()) {
                    let params = vec!["i64"; args.len()].join(", ");
                    let _ = writeln!(out, "declare i64 @{}({})", name, params);
                }
            }
        }
    }
    if !declared.is_empty() {
        out.push('\n');
    }

    for f in funcs {
        out.push_str(&emit_llvm_function(f));
        out.push('\n');
    }

    if let Some(entry) = funcs.iter().find(|f| f.name.replace('@', "") == entry_func) {
        let callee = sanitize_fname(&entry.name);
        out.push_str("define i32 @main() {\n");
        out.push_str("entry:\n");
        let _ = writeln!(out, "  %r = call i64 @{}()", callee);
        out.push_str(
            "  %p = getelementptr inbounds [6 x i8], ptr @.fmt, i64 0, i64 0\n",
        );
        out.push_str("  call i32 (ptr, ...) @printf(ptr %p, i64 %r)\n");
        out.push_str("  ret i32 0\n");
        out.push_str("}\n");
    }

    out
}

fn emit_llvm_function(f: &Function) -> String {
    let mut out = String::new();
    let ret_ty = llvm_type(&f.ret);
    let fname = sanitize_fname(&f.name);

    let params: Vec<String> = f
        .params
        .iter()
        .map(|(pname, pty)| {
            let cty = match pty {
                Type::F64 => "double",
                _ => "i64",
            };
            format!("{} %{}", cty, pname)
        })
        .collect();
    let _ = writeln!(out, "define {} @{}({}) {{", ret_ty, fname, params.join(", "));

    // The lowering may begin with straight-line instructions before any label;
    // those form the entry block. If the body opens with an explicit label we
    // use it as-is, otherwise we synthesise `entry:`.
    let starts_with_label = matches!(f.body.first(), Some(Instr::Label(_)));
    if !starts_with_label {
        out.push_str("entry:\n");
    }

    // Counter for fresh temps needed to widen i64 predicates to i1.
    let mut cmp_id: usize = 0;

    for instr in &f.body {
        match instr {
            Instr::Label(l) => {
                let _ = writeln!(out, "{}:", l);
            }
            Instr::ConstI64(dest, val) => {
                // Materialise a constant via `add i64 0, val` so it occupies an
                // SSA register (LLVM has no standalone const-define instruction).
                let _ = writeln!(out, "  {} = add i64 0, {}", operand(dest), val);
            }
            Instr::Add(dest, a, b) => {
                let _ = writeln!(out, "  {} = add i64 {}, {}", operand(dest), operand(a), operand(b));
            }
            Instr::Sub(dest, a, b) => {
                let _ = writeln!(out, "  {} = sub i64 {}, {}", operand(dest), operand(a), operand(b));
            }
            Instr::Mul(dest, a, b) => {
                let _ = writeln!(out, "  {} = mul i64 {}, {}", operand(dest), operand(a), operand(b));
            }
            Instr::Div(dest, a, b) => {
                let _ = writeln!(out, "  {} = sdiv i64 {}, {}", operand(dest), operand(a), operand(b));
            }
            Instr::Call(dest, target, args) => {
                let arg_str = args
                    .iter()
                    .map(|a| format!("i64 {}", operand(a)))
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(
                    out,
                    "  {} = call i64 @{}({})",
                    operand(dest),
                    sanitize_fname(target),
                    arg_str
                );
            }
            Instr::Br(target) => {
                let _ = writeln!(out, "  br label %{}", target);
            }
            Instr::BrCond(pred, t, fl) => {
                let cmp = format!("%cmp.{}", cmp_id);
                cmp_id += 1;
                let _ = writeln!(out, "  {} = icmp ne i64 {}, 0", cmp, operand(pred));
                let _ = writeln!(out, "  br i1 {}, label %{}, label %{}", cmp, t, fl);
            }
            Instr::Phi(dest, ty, pairs) => {
                let arms = pairs
                    .iter()
                    .map(|(v, bb)| format!("[ {}, %{} ]", operand(v), bb))
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(out, "  {} = phi {} {}", operand(dest), ty, arms);
            }
            Instr::Ret(Some(v)) => {
                let _ = writeln!(out, "  ret {} {}", ret_ty, operand(v));
            }
            Instr::Ret(None) => {
                out.push_str("  ret void\n");
            }
            Instr::Deopt => {
                // No runtime deopt in AOT: trap deterministically.
                out.push_str("  call void @llvm.trap()\n");
                out.push_str("  unreachable\n");
            }
        }
    }

    out.push_str("}\n");
    out
}
