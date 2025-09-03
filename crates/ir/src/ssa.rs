use std::collections::{HashMap, HashSet};
use crate::{Function, Instr};

/// Two-pass SSA renamer for the IR.
///
/// Collects lowering-local temps of the form "%<fname>_..." in order of
/// appearance and assigns them stable names "%t0","%t1", ... then rewrites
/// operand uses to the new names. Parameters, labels and non-local names are
/// preserved.
pub fn rename_temps(func: &mut Function) {
    let fname_prefix = func.name.replace("@", "");
    let local_prefix = format!("%{}_", fname_prefix);

    let is_renamed = |s: &str| -> bool {
        if !s.starts_with("%t") { return false; }
        s[2..].chars().all(|c| c.is_ascii_digit())
    };
    let is_candidate = |s: &str| -> bool {
        s.starts_with('%') && s.starts_with(&local_prefix) && !is_renamed(s)
    };

    // Pass 1: collect defs in order
    let mut defs: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for instr in func.body.iter() {
        match instr {
            Instr::ConstI64(name, _) => {
                if is_candidate(name) && seen.insert(name.clone()) {
                    defs.push(name.clone());
                }
            }
            Instr::Add(dest, _, _) | Instr::Sub(dest, _, _) | Instr::Mul(dest, _, _) | Instr::Div(dest, _, _) => {
                if is_candidate(dest) && seen.insert(dest.clone()) {
                    defs.push(dest.clone());
                }
            }
            Instr::Call(dest, _, _) | Instr::Phi(dest, _, _) => {
                if is_candidate(dest) && seen.insert(dest.clone()) {
                    defs.push(dest.clone());
                }
            }
            _ => {}
        }
    }

    // Build mapping
    let mut map: HashMap<String, String> = HashMap::new();
    for (i, name) in defs.iter().enumerate() {
        map.insert(name.clone(), format!("%t{}", i));
    }

    // helper
    let replace = |s: &str, map: &HashMap<String, String>| -> String {
        if let Some(n) = map.get(s) { n.clone() } else { s.to_string() }
    };

    // Pass 2: rewrite operands
    for instr in func.body.iter_mut() {
        match instr {
            Instr::ConstI64(name, _) => {
                *name = replace(name, &map);
            }
            Instr::Add(dest,a,b) | Instr::Sub(dest,a,b) | Instr::Mul(dest,a,b) | Instr::Div(dest,a,b) => {
                *dest = replace(dest, &map);
                *a = replace(a, &map);
                *b = replace(b, &map);
            }
            Instr::Call(dest, _fn, args) => {
                *dest = replace(dest, &map);
                for a in args.iter_mut() { *a = replace(a, &map); }
            }
            Instr::Label(_) | Instr::Br(_) => {}
            Instr::BrCond(pred, _t, _f) => {
                *pred = replace(pred, &map);
            }
            Instr::Phi(dest, _ty, pairs) => {
                *dest = replace(dest, &map);
                for (v, _bb) in pairs.iter_mut() { *v = replace(v, &map); }
            }
            Instr::Ret(opt) => {
                if let Some(v) = opt { *v = replace(v, &map); }
            }
        }
    }
}

/// Conservative phi-insertion pass.
///
/// Splits the function body into basic blocks by labels, computes predecessors,
/// and where a block has multiple predecessors with differing last-defined
/// temps, inserts a Phi instruction at the block entry and rewrites local
/// uses to consume the phi result. This is intentionally simplistic.
pub fn insert_phi_nodes(func: &mut Function) {
    use crate::Instr;

    // Split into blocks
    let mut blocks: Vec<(String, Vec<Instr>)> = Vec::new();
    let mut cur_label = "entry".to_string();
    let mut cur_block: Vec<Instr> = Vec::new();
    for instr in func.body.drain(..) {
        match &instr {
            Instr::Label(l) => {
                // emit previous
                blocks.push((cur_label.clone(), cur_block));
                cur_label = l.clone();
                cur_block = Vec::new();
                cur_block.push(instr);
            }
            _ => cur_block.push(instr),
        }
    }
    blocks.push((cur_label.clone(), cur_block));

    // index map
    let mut idx_of: std::collections::HashMap<String, usize> = HashMap::new();
    for (i, (lbl, _)) in blocks.iter().enumerate() {
        idx_of.insert(lbl.clone(), i);
    }

    // successors
    let mut succs: Vec<Vec<String>> = vec![Vec::new(); blocks.len()];
    for (i, (_lbl, b)) in blocks.iter().enumerate() {
        if let Some(last) = b.iter().rev().find(|_| true) {
            match last {
                Instr::Br(t) => succs[i].push(t.clone()),
                Instr::BrCond(_, t, f) => { succs[i].push(t.clone()); succs[i].push(f.clone()); }
                _ => {}
            }
        }
    }

    // predecessors
    let mut preds: Vec<Vec<String>> = vec![Vec::new(); blocks.len()];
    for (i, s) in succs.iter().enumerate() {
        for target in s.iter() {
            if let Some(&j) = idx_of.get(target) { preds[j].push(blocks[i].0.clone()); }
        }
    }

    // diagnostics removed in final pass

    // helper: determine local candidate names for this function (same rule as rename_temps)
    let fname_prefix = func.name.replace("@", "");
    let local_prefix = format!("%{}_", fname_prefix);
    let is_renamed = |s: &str| -> bool {
        if !s.starts_with("%t") { return false; }
        s[2..].chars().all(|c| c.is_ascii_digit())
    };
    let is_candidate = |s: &str| -> bool { s.starts_with('%') && s.starts_with(&local_prefix) && !is_renamed(s) };

    // helper: last def in a block, but only return local candidate names
    let last_def = |b: &Vec<Instr>| -> Option<String> {
        for instr in b.iter().rev() {
            match instr {
                Instr::ConstI64(name, _) => if is_candidate(name) { return Some(name.clone()) } else { continue },
                Instr::Add(dest, _, _) | Instr::Sub(dest, _, _) | Instr::Mul(dest, _, _) | Instr::Div(dest, _, _) | Instr::Call(dest, _, _) | Instr::Phi(dest, _, _) => {
                    if is_candidate(dest) { return Some(dest.clone()) } else { continue }
                }
                _ => {}
            }
        }
        None
    };

    // process blocks with multiple preds
    for i in 0..blocks.len() {
        if preds[i].len() <= 1 { continue; }

    let mut incoming: Vec<(String, String)> = Vec::new();
        for p in preds[i].iter() {
            if let Some(&pi) = idx_of.get(p) {
                let last = last_def(&blocks[pi].1);
                eprintln!("[ssa] pred '{}' -> idx {} last_def={:?}", p, pi, last);
                if let Some(v) = last { incoming.push((v, blocks[pi].0.clone())); }
            }
        }
    // debug removed
        if incoming.is_empty() { continue; }
        let all_same = incoming.windows(2).all(|w| w[0].0 == w[1].0);
    // debug removed
        if all_same { continue; }

        let phi_dest = format!("%phi_{}_{}", func.name.replace("@", ""), i);
        let pairs = incoming.clone();
        let ty = crate::Type::I64;
    let phi_instr = Instr::Phi(phi_dest.clone(), ty, pairs);

    // inserted phi: keep silent in normal runs

        // insert phi at start (after label if present)
        let mut new_block: Vec<Instr> = Vec::new();
        if let Some(first) = blocks[i].1.first() {
            match first {
                Instr::Label(_) => {
                    new_block.push(first.clone());
                    new_block.push(phi_instr);
                    for instr in blocks[i].1.iter().skip(1) { new_block.push(instr.clone()); }
                }
                _ => {
                    new_block.push(crate::Instr::Label(blocks[i].0.clone()));
                    new_block.push(phi_instr);
                    for instr in blocks[i].1.iter() { new_block.push(instr.clone()); }
                }
            }
        } else { continue; }

    // rewrite uses inside block: replace incoming temps with phi_dest conservatively
    // Only rewrite operands that are candidate locals to avoid touching params or globals.
    for instr in new_block.iter_mut() {
            match instr {
                Instr::Add(_d, a, b) | Instr::Sub(_d, a, b) | Instr::Mul(_d, a, b) | Instr::Div(_d, a, b) => {
            if is_candidate(a) && incoming.iter().any(|(v, _)| v == a) { *a = phi_dest.clone(); }
            if is_candidate(b) && incoming.iter().any(|(v, _)| v == b) { *b = phi_dest.clone(); }
                }
                Instr::Call(_d, _f, args) => {
            for a in args.iter_mut() { if is_candidate(a) && incoming.iter().any(|(v, _)| v == a) { *a = phi_dest.clone(); } }
                }
                Instr::BrCond(pred, _t, _f) => {
            if is_candidate(pred) && incoming.iter().any(|(v, _)| v == pred) { *pred = phi_dest.clone(); }
                }
                _ => {}
            }
        }

        blocks[i].1 = new_block;
    }

    // flatten
    let mut out: Vec<Instr> = Vec::new();
    for (_lbl, b) in blocks.into_iter() { for instr in b.into_iter() { out.push(instr); } }
    func.body = out;
}
