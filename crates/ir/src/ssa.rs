use crate::{Function, Instr};
use std::collections::{HashMap, HashSet};

/// Two-pass SSA renamer for the IR.
///
/// Collects lowering-local temps of the form "%<fname>_..." in order of
/// appearance and assigns them stable names "%t0","%t1", ... then rewrites
/// operand uses to the new names. Parameters, labels and non-local names are
/// preserved.
pub fn rename_temps(func: &mut Function) {
    let fname_prefix = func.name.replace("@", "");
    let local_prefix = format!("%{}_", fname_prefix);

    let is_candidate = |s: &str| -> bool {
        s.starts_with('%') && s.starts_with(&local_prefix) && !s.starts_with("%t")
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
            Instr::Add(dest, _, _)
            | Instr::Sub(dest, _, _)
            | Instr::Mul(dest, _, _)
            | Instr::Div(dest, _, _) => {
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
        if let Some(n) = map.get(s) {
            n.clone()
        } else {
            s.to_string()
        }
    };

    // Pass 2: rewrite operands
    for instr in func.body.iter_mut() {
        match instr {
            Instr::ConstI64(name, _) => {
                *name = replace(name, &map);
            }
            Instr::Add(dest, a, b)
            | Instr::Sub(dest, a, b)
            | Instr::Mul(dest, a, b)
            | Instr::Div(dest, a, b) => {
                *dest = replace(dest, &map);
                *a = replace(a, &map);
                *b = replace(b, &map);
            }
            Instr::Call(dest, _fn, args) => {
                *dest = replace(dest, &map);
                for a in args.iter_mut() {
                    *a = replace(a, &map);
                }
            }
            Instr::Label(_) | Instr::Br(_) => {}
            Instr::BrCond(pred, _t, _f) => {
                *pred = replace(pred, &map);
            }
            Instr::Phi(dest, _ty, pairs) => {
                *dest = replace(dest, &map);
                for (v, _bb) in pairs.iter_mut() {
                    *v = replace(v, &map);
                }
            }
            Instr::Ret(opt) => {
                if let Some(v) = opt {
                    *v = replace(v, &map);
                }
            }
        }
    }
}

/// Insert conservative phi nodes at block entries where multiple predecessors
/// provide different last-defined temps. This is a minimal, local pass used to
/// make textual IR more robust for downstream passes and tests.
pub fn insert_phi_nodes(func: &mut Function) {
    use crate::Instr;

    // First, split body into basic blocks (label -> Vec<Instr>) preserving order.
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
    // push last
    blocks.push((cur_label.clone(), cur_block));

    // Build label -> index map
    let mut idx_of: std::collections::HashMap<String, usize> = HashMap::new();
    for (i, (lbl, _)) in blocks.iter().enumerate() {
        idx_of.insert(lbl.clone(), i);
    }

    // Compute successors for each block
    let mut succs: Vec<Vec<String>> = vec![Vec::new(); blocks.len()];
    for (i, (_lbl, b)) in blocks.iter().enumerate() {
        if let Some(last) = b.iter().rev().find(|_| true) {
            match last {
                Instr::Br(t) => succs[i].push(t.clone()),
                Instr::BrCond(_, t, f) => {
                    succs[i].push(t.clone());
                    succs[i].push(f.clone());
                }
                _ => {}
            }
        }
    }

    // Compute predecessors
    let mut preds: Vec<Vec<String>> = vec![Vec::new(); blocks.len()];
    for (i, s) in succs.iter().enumerate() {
        for target in s.iter() {
            if let Some(&j) = idx_of.get(target) {
                preds[j].push(blocks[i].0.clone());
            }
        }
    }

    // Helper to find last defined dest in a block (if any)
    let last_def = |b: &Vec<Instr>| -> Option<String> {
        for instr in b.iter().rev() {
            match instr {
                Instr::ConstI64(name, _) => return Some(name.clone()),
                Instr::Add(dest, _, _)
                | Instr::Sub(dest, _, _)
                | Instr::Mul(dest, _, _)
                | Instr::Div(dest, _, _)
                | Instr::Call(dest, _, _)
                | Instr::Phi(dest, _, _) => return Some(dest.clone()),
                _ => {}
            }
        }
        None
    };

    // For each block with multiple predecessors, consider inserting phi nodes.
    for i in 0..blocks.len() {
        if preds[i].len() <= 1 {
            continue;
        }

        // Collect last defs from each predecessor
        let mut incoming: Vec<(String, String)> = Vec::new();
        for p in preds[i].iter() {
            if let Some(&pi) = idx_of.get(p) {
                if let Some(v) = last_def(&blocks[pi].1) {
                    incoming.push((v, blocks[pi].0.clone()));
                }
            }
        }

        // If incoming values are non-empty and not all equal, create a phi
        if incoming.is_empty() {
            continue;
        }
        let all_same = incoming.windows(2).all(|w| w[0].0 == w[1].0);
        if all_same {
            continue; // no merge needed
        }

        // Create a fresh dest name for the phi
        let phi_dest = format!("%phi_{}_{}", func.name.replace("@", ""), i);
        // Build the Phi instruction
        let pairs = incoming.clone();
        let ty = crate::Type::I64; // conservative default
        let phi_instr = Instr::Phi(phi_dest.clone(), ty, pairs);

        // Insert phi at start of block (after label if present)
        let mut new_block: Vec<Instr> = Vec::new();
        if let Some(first) = blocks[i].1.first() {
            match first {
                Instr::Label(_) => {
                    new_block.push(first.clone());
                    new_block.push(phi_instr);
                    for instr in blocks[i].1.iter().skip(1) {
                        new_block.push(instr.clone());
                    }
                }
                _ => {
                    // no label: create an entry label and then phi
                    new_block.push(crate::Instr::Label(blocks[i].0.clone()));
                    new_block.push(phi_instr);
                    for instr in blocks[i].1.iter() {
                        new_block.push(instr.clone());
                    }
                }
            }
        } else {
            // empty block - shouldn't happen, keep as-is
            continue;
        }

        // Replace uses of incoming temps inside the block with phi_dest
        for instr in new_block.iter_mut() {
            match instr {
                Instr::Add(_dest, a, b)
                | Instr::Sub(_dest, a, b)
                | Instr::Mul(_dest, a, b)
                | Instr::Div(_dest, a, b) => {
                    if incoming.iter().any(|(v, _)| v == a) {
                        *a = phi_dest.clone();
                    }
                    if incoming.iter().any(|(v, _)| v == b) {
                        *b = phi_dest.clone();
                    }
                }
                Instr::Call(_dest, _f, args) => {
                    for a in args.iter_mut() {
                        if incoming.iter().any(|(v, _)| v == a) {
                            *a = phi_dest.clone();
                        }
                    }
                }
                Instr::BrCond(pred, _t, _f) => {
                    if incoming.iter().any(|(v, _)| v == pred) {
                        *pred = phi_dest.clone();
                    }
                }
                Instr::Phi(_, _, _) | Instr::Label(_) | Instr::Br(_) | Instr::Ret(_) | Instr::ConstI64(_, _) => {}
            }
        }

        // replace block
        blocks[i].1 = new_block;
    }

    // Flatten blocks back into func.body
    let mut out: Vec<Instr> = Vec::new();
    for (_lbl, b) in blocks.into_iter() {
        for instr in b.into_iter() {
            out.push(instr);
        }
    }
    func.body = out;
}
