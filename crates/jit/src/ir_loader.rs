use std::path::Path;
use std::fs;
use crate::ir_analyzer::IrAnalysis;
use ir::{Function, Instr, Type};

/// Parse a textual IR file into an `IrAnalysis` using `ir::Function` representations.
/// This parser is intentionally permissive: it supports the documented subset and
/// falls back to the textual analyzer when the file looks too small/simple.
pub fn parse_ir_file(path: &Path) -> Option<IrAnalysis> {
    let s = fs::read_to_string(path).ok()?;
    // If file doesn't contain interesting tokens, bail out to let the lighter
    // text analyzer handle it (avoids over-parsing trivial files).
    if !s.contains("call") && !s.contains("gc_alloc") && !s.contains("arena_alloc") {
        return None;
    }

    // Start a permissive parse
    let mut fname = String::new();
    let mut ret_ty: Option<Type> = None;
    let mut body: Vec<Instr> = Vec::new();

    for raw in s.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        // header: func @name(...) -> typ {
        if line.starts_with("func @") {
            // attempt to extract name and return type
            if let Some(at) = line.split_whitespace().nth(1) {
                // at is like @name(params) -> ret {
                if let Some(rest) = at.strip_prefix("@") {
                    // name possibly with params; take until '(' or '{'
                    let name = rest.split('(').next().unwrap_or(rest).trim_end_matches('{').to_string();
                    fname = name;
                }
            }
            if line.contains("->") {
                if let Some(ret) = line.split("->").nth(1) {
                    let part = ret.trim().trim_end_matches('{').trim();
                    match part.split_whitespace().next().unwrap_or("") {
                        "i64" => ret_ty = Some(Type::I64),
                        "f64" => ret_ty = Some(Type::F64),
                        _ => ret_ty = None,
                    }
                }
            }
            continue;
        }
        if line == "}" { break; }

        // label
        if line.ends_with(':') {
            let lbl = line.trim_end_matches(':').to_string();
            body.push(Instr::Label(lbl));
            continue;
        }

        // br / br_cond
        if line.starts_with("br ") {
            let arg = line[3..].trim();
            body.push(Instr::Br(arg.to_string()));
            continue;
        }
        if line.starts_with("br_cond") || line.starts_with("br_cond ") {
            // formats: br_cond pred, if_true, if_false
            let rest = line.trim_start_matches("br_cond").trim();
            let parts: Vec<&str> = rest.split(',').map(|s| s.trim()).collect();
            if parts.len() >= 3 {
                body.push(Instr::BrCond(parts[0].to_string(), parts[1].to_string(), parts[2].to_string()));
                continue;
            }
        }

        // ret
        if line == "ret" || line.starts_with("ret ") {
            let maybe = line.split_whitespace().nth(1).map(|s| s.to_string());
            body.push(Instr::Ret(maybe));
            continue;
        }

        // assignment-style instructions: dest = opcode ...
        if let Some(eq) = line.find('=') {
            let (lhs, rhs) = line.split_at(eq);
            let dest = lhs.trim().to_string();
            let rhs = rhs.trim_start_matches('=').trim();
            // const
            if rhs.starts_with("const ") && rhs.contains("i64") {
                if let Some(vstr) = rhs.split_whitespace().last() {
                    if let Ok(v) = vstr.parse::<i64>() {
                        body.push(Instr::ConstI64(dest, v));
                        continue;
                    }
                }
            }
            // arithmetic
            if rhs.starts_with("add ") || rhs.contains(" add ") {
                // pattern: add i64 a, b
                let parts: Vec<&str> = rhs.split_whitespace().collect();
                if parts.len() >= 4 {
                    let a = parts[2].trim_end_matches(',').to_string();
                    let b = parts[3].to_string();
                    body.push(Instr::Add(dest, a, b));
                    continue;
                }
            }
            if rhs.starts_with("sub ") || rhs.contains(" sub ") {
                let parts: Vec<&str> = rhs.split_whitespace().collect();
                if parts.len() >= 4 {
                    let a = parts[2].trim_end_matches(',').to_string();
                    let b = parts[3].to_string();
                    body.push(Instr::Sub(dest, a, b));
                    continue;
                }
            }
            if rhs.starts_with("mul ") || rhs.contains(" mul ") {
                let parts: Vec<&str> = rhs.split_whitespace().collect();
                if parts.len() >= 4 {
                    let a = parts[2].trim_end_matches(',').to_string();
                    let b = parts[3].to_string();
                    body.push(Instr::Mul(dest, a, b));
                    continue;
                }
            }
            if rhs.starts_with("div ") || rhs.contains(" div ") {
                let parts: Vec<&str> = rhs.split_whitespace().collect();
                if parts.len() >= 4 {
                    let a = parts[2].trim_end_matches(',').to_string();
                    let b = parts[3].to_string();
                    body.push(Instr::Div(dest, a, b));
                    continue;
                }
            }
            // call pattern: call name(args) or call fn(args)
            if rhs.contains("call") {
                // simplistic: find 'call ' and parse name and args inside parentheses
                if let Some(pos) = rhs.find("call") {
                    let after = rhs[pos + 4..].trim();
                    // e.g. sum( a, b ) or @sum(a)
                    let fnname = after.split('(').next().unwrap_or(after).trim().to_string();
                    let args = if let Some(start) = after.find('(') {
                        let inner = &after[start + 1..];
                        if let Some(end) = inner.find(')') {
                            inner[..end].split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
                        } else { vec![] }
                    } else { vec![] };
                    body.push(Instr::Call(dest, fnname, args));
                    continue;
                }
            }
            // fallback: treat as a call-like to capture heavy ops like gc_alloc
            if rhs.contains("gc_alloc") || rhs.contains("arena_alloc") {
                let fnname = if rhs.contains("gc_alloc") { "gc_alloc" } else { "arena_alloc" };
                body.push(Instr::Call(dest, fnname.to_string(), vec![]));
                continue;
            }
            // unknown assignment -> treat as generic instruction via dest = add (fallback)
            body.push(Instr::Add(dest.clone(), "0".to_string(), "0".to_string()));
            continue;
        }

        // non-assignment instructions: e.g., 'ret' handled earlier; otherwise ignore
    }

    // Build Function and compute metrics
    let func = Function { name: fname, params: Vec::new(), ret: ret_ty, body };
    // Count blocks and instruction kinds
    let mut instr_count = 0usize;
    let mut block_count = 0usize;
    let mut call_count = 0usize;
    let mut alloc_count = 0usize;
    for instr in &func.body {
        match instr {
            Instr::Label(_) => block_count += 1,
            Instr::Call(_, name, _) => {
                instr_count += 1;
                call_count += 1;
                if name.contains("gc_alloc") || name.contains("arena_alloc") { alloc_count += 1; }
            }
            Instr::ConstI64(_, _) | Instr::Add(_,_,_) | Instr::Sub(_,_,_) | Instr::Mul(_,_,_) | Instr::Div(_,_,_) | Instr::Br(_) | Instr::BrCond(_,_,_) | Instr::Phi(_,_,_) | Instr::Ret(_) => {
                instr_count += 1;
            }
        }
    }

    if block_count == 0 && !func.body.is_empty() { block_count = 1; }

    // Weighted metric
    let weighted = instr_count + call_count * 5 + alloc_count * 10 + block_count * 2;
    Some(IrAnalysis { instr_count: weighted, block_count })
}

