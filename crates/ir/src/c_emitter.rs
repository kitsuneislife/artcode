use crate::{Function, Instr};

/// C keywords that must not be used as function names.
const C_KEYWORDS: &[&str] = &[
    "auto", "break", "case", "char", "const", "continue", "default", "do",
    "double", "else", "enum", "extern", "float", "for", "goto", "if",
    "inline", "int", "long", "register", "restrict", "return", "short",
    "signed", "sizeof", "static", "struct", "switch", "typedef", "union",
    "unsigned", "void", "volatile", "while", "_Bool", "_Complex", "_Imaginary",
];

fn sanitize_fname(raw: &str) -> String {
    let name = raw.replace("@", "");
    // Rename the Artcode `main` function to avoid collision with C entrypoint.
    if name == "main" {
        return "_art_main".to_string();
    }
    // Prefix any C keyword collision with `_art_`
    if C_KEYWORDS.contains(&name.as_str()) {
        return format!("_art_{}", name);
    }
    name
}

pub fn emit_c_program(funcs: &[Function], entry_func: &str) -> String {
    let mut out = String::new();

    out.push_str("#include <stdint.h>\n");
    out.push_str("#include <stdio.h>\n");
    out.push_str("#include <stdlib.h>\n\n");

    // Forward declarations
    for f in funcs {
        let ret_type = match &f.ret {
            Some(crate::Type::I64) => "int64_t",
            Some(crate::Type::F64) => "double",
            _ => "void",
        };

        let fname = sanitize_fname(&f.name);
        out.push_str(&format!("{} {}(", ret_type, fname));
        if f.params.is_empty() {
            out.push_str("void");
        } else {
            let params_str = f
                .params
                .iter()
                .map(|(pname, pty)| {
                    let cty = match pty {
                        crate::Type::I64 => "int64_t",
                        crate::Type::F64 => "double",
                        _ => "void*",
                    };
                    format!("{} {}", cty, pname.replace("%", "p_"))
                })
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&params_str);
        }
        out.push_str(");\n");
    }
    out.push_str("\n");

    for f in funcs {
        out.push_str(&emit_c_function(f));
        out.push_str("\n");
    }

    if funcs.iter().any(|f| f.name.replace("@", "") == entry_func) {
        let call_name = if entry_func == "main" { "_art_main" } else { entry_func };
        out.push_str("int main(void) {\n");
        out.push_str(&format!("    int64_t result = {}();\n", call_name));
        out.push_str("    printf(\"%lld\\n\", (long long)result);\n");
        out.push_str("    return 0;\n");
        out.push_str("}\n");
    }

    out
}

fn emit_c_function(f: &Function) -> String {
    let mut out = String::new();

    let ret_type = match &f.ret {
        Some(crate::Type::I64) => "int64_t",
        Some(crate::Type::F64) => "double",
        _ => "void",
    };

    let fname = sanitize_fname(&f.name);
    out.push_str(&format!("{} {}(", ret_type, fname));

    if f.params.is_empty() {
        out.push_str("void");
    } else {
        let params_str = f
            .params
            .iter()
            .map(|(pname, pty)| {
                let cty = match pty {
                    crate::Type::I64 => "int64_t",
                    crate::Type::F64 => "double",
                    _ => "void*",
                };
                format!("{} {}", cty, pname.replace("%", "p_"))
            })
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&params_str);
    }
    out.push_str(") {\n");

    let mut locals: std::collections::HashSet<String> = std::collections::HashSet::new();
    let sanitize = |s: &str| -> String { s.replace("%", "v_").replace(".", "_") };
    let sanitize_lbl = |s: &str| -> String { s.replace(".", "_") };

    // Register all variables to declare them at function scope
    for instr in &f.body {
        match instr {
            Instr::ConstI64(dest, _) => { locals.insert(sanitize(dest)); }
            Instr::Add(dest, _, _) | Instr::Sub(dest, _, _) | Instr::Mul(dest, _, _) | Instr::Div(dest, _, _) => { locals.insert(sanitize(dest)); }
            Instr::Call(dest, _, _) => { locals.insert(sanitize(dest)); }
            Instr::Phi(dest, _, _) => { locals.insert(sanitize(dest)); }
            _ => {}
        }
    }

    for loc in &locals {
        out.push_str(&format!("    int64_t {} = 0;\n", loc));
    }

    // We use a custom string-based previous block tracker to satisfy SSA Phi semantics trivially
    out.push_str("    const char* _prev_block = \"\";\n");
    out.push_str("    const char* _curr_block = \"entry\";\n");

    let resolve = |s: &str| -> String {
        let san = sanitize(s);
        if f.params.iter().any(|(p, _)| p == s) {
            s.replace("%", "p_")
        } else {
            san
        }
    };

    for instr in &f.body {
        match instr {
            Instr::Label(l) => {
                let s_lbl = sanitize_lbl(l);
                out.push_str(&format!("    _prev_block = _curr_block;\n"));
                out.push_str(&format!("    _curr_block = \"{}\";\n", s_lbl));
                out.push_str(&format!("L_{}:\n", s_lbl));
            }
            Instr::ConstI64(dest, val) => {
                out.push_str(&format!("    {} = {}LL;\n", resolve(dest), val));
            }
            Instr::Add(dest, a, b) => {
                out.push_str(&format!("    {} = {} + {};\n", resolve(dest), resolve(a), resolve(b)));
            }
            Instr::Sub(dest, a, b) => {
                out.push_str(&format!("    {} = {} - {};\n", resolve(dest), resolve(a), resolve(b)));
            }
            Instr::Mul(dest, a, b) => {
                out.push_str(&format!("    {} = {} * {};\n", resolve(dest), resolve(a), resolve(b)));
            }
            Instr::Div(dest, a, b) => {
                out.push_str(&format!("    if ({} == 0) {{ printf(\"div by zero\\n\"); exit(1); }}\n", resolve(b)));
                out.push_str(&format!("    {} = {} / {};\n", resolve(dest), resolve(a), resolve(b)));
            }
            Instr::Call(dest, target, args) => {
                let args_str = args.iter().map(|a| resolve(a)).collect::<Vec<_>>().join(", ");
                let ct = sanitize_fname(target);
                out.push_str(&format!("    {} = {}({});\n", resolve(dest), ct, args_str));
            }
            Instr::Br(target) => {
                out.push_str(&format!("    goto L_{};\n", sanitize_lbl(target)));
            }
            Instr::BrCond(pred, t, f_lbl) => {
                out.push_str(&format!("    if ({}) goto L_{}; else goto L_{};\n", resolve(pred), sanitize_lbl(t), sanitize_lbl(f_lbl)));
            }
            Instr::Phi(dest, _, pairs) => {
                // Simulate Phi by checking `_prev_block` against the incoming labels
                for (i, (val, bb)) in pairs.iter().enumerate() {
                    let cmd = if i == 0 { "if" } else { "else if" };
                    out.push_str(&format!("    {} (_prev_block == \"{}\") {{ {} = {}; }}\n", cmd, sanitize_lbl(bb), resolve(dest), resolve(val)));
                }
            }
            Instr::Ret(Some(v)) => {
                out.push_str(&format!("    return {};\n", resolve(v)));
            }
            Instr::Ret(None) => {
                out.push_str("    return;\n");
            }
            Instr::Deopt => {
                out.push_str("    printf(\"[AOT] DEOPT triggered! Exiting...\\n\");\n");
                out.push_str("    exit(1);\n");
            }
        }
    }

    out.push_str("}\n");
    out
}
