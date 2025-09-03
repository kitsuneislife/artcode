use std::path::Path;
use std::fs;
use crate::ir_analyzer::IrAnalysis;
use ir::{Function, Instr, Type};

/// Parse a textual IR file into an `IrAnalysis` using `ir::Function` representations.
/// This parser is permissive but attempts to map the real textual IR emitted by
/// `crates/ir::Function::emit_text()` into `ir::Function` and its `Instr`s.
pub fn parse_ir_file(path: &Path) -> Option<IrAnalysis> {
    let s = fs::read_to_string(path).ok()?;
    // only attempt a full parse when file looks like a function emission
    if !s.contains("func @") {
        return None;
    }

    let mut fname = String::new();
    let mut ret_ty: Option<Type> = None;
    let mut body: Vec<Instr> = Vec::new();
    let mut params: Vec<(String, Type)> = Vec::new();
    let mut seen_header = false;
    let mut seen_closing = false;

    for raw in s.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        // function header: func @name(params) -> typ {
        if line.starts_with("func @") {
            seen_header = true;
            // name
            if let Some(start) = line.find('@') {
                if let Some(rest) = line[start + 1..].split_whitespace().next() {
                    // rest might be like "name(params)"
                    let name = rest.split('(').next().unwrap_or(rest).trim_end_matches('{').to_string();
                    fname = name;
                }
            }
            // params
            if let Some(lp) = line.find('(') {
                if let Some(rp_rel) = line[lp..].find(')') {
                    let inside = &line[lp + 1..lp + rp_rel];
                    for part in inside.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                        let mut toks = part.split_whitespace();
                        if let Some(ty) = toks.next() {
                            if let Some(name) = toks.next() {
                                let t = match ty {
                                    "i64" => Type::I64,
                                    "f64" => Type::F64,
                                    _ => Type::I64,
                                };
                                params.push((name.to_string(), t));
                            }
                        }
                    }
                }
            }
            // return type
            if let Some(idx) = line.find("->") {
                let after = &line[idx + 2..];
                let part = after.trim().trim_end_matches('{').trim();
                match part.split_whitespace().next().unwrap_or("") {
                    "i64" => ret_ty = Some(Type::I64),
                    "f64" => ret_ty = Some(Type::F64),
                    _ => ret_ty = None,
                }
            }
            continue;
        }

    if line == "}" { seen_closing = true; break; }

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
        if line.starts_with("br_cond") {
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

            // const i64
            if rhs.starts_with("const ") && rhs.contains("i64") {
                if let Some(vstr) = rhs.split_whitespace().last() {
                    if let Ok(v) = vstr.parse::<i64>() {
                        body.push(Instr::ConstI64(dest, v));
                        continue;
                    }
                }
            }

            // arithmetic: add/sub/mul/div
            if rhs.starts_with("add ") || rhs.contains(" add ") {
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

            // phi: phi TYPE [ v, bb ], [ v2, bb2 ]
            if rhs.starts_with("phi ") {
                let parts: Vec<&str> = rhs.split_whitespace().collect();
                if parts.len() >= 2 {
                    let ty_tok = parts[1];
                    let ty = match ty_tok {
                        "i64" => Type::I64,
                        "f64" => Type::F64,
                        _ => Type::I64,
                    };
                    let mut pairs: Vec<(String, String)> = Vec::new();
                    let mut rest = rhs.to_string();
                    while let Some(lbr) = rest.find('[') {
                        if let Some(rbr) = rest[lbr..].find(']') {
                            let inner = &rest[lbr + 1..lbr + rbr];
                            let kv: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
                            if kv.len() >= 2 {
                                pairs.push((kv[0].to_string(), kv[1].to_string()));
                            }
                            rest = rest[lbr + rbr + 1..].to_string();
                        } else { break; }
                    }
                    body.push(Instr::Phi(dest, ty, pairs));
                    continue;
                }
            }

            // call pattern: call name(args)
            if rhs.starts_with("call") || rhs.contains("= call") || rhs.contains(" call ") {
                if let Some(pos) = rhs.find("call") {
                    let after = rhs[pos + 4..].trim();
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

            // fallback: detect allocation intrinsics
            if rhs.contains("gc_alloc") || rhs.contains("arena_alloc") {
                let fnname = if rhs.contains("gc_alloc") { "gc_alloc" } else { "arena_alloc" };
                body.push(Instr::Call(dest, fnname.to_string(), vec![]));
                continue;
            }

            // unknown assignment -> in strict mode, fail parse
            return None;
        }

        // non-assignment instructions (none else for now)
    }

    // Require a valid header and closing brace
    if !seen_header { return None; }
    if !seen_closing { return None; }
    if fname.is_empty() { return None; }

    // Build Function and compute metrics
    let func = Function { name: fname, params, ret: ret_ty, body };

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

    // Weighted metric: calls and allocs heavier, plus slight block penalty
    let weighted = instr_count + call_count * 5 + alloc_count * 10 + block_count * 2;
    Some(IrAnalysis { instr_count: weighted, block_count })
}

