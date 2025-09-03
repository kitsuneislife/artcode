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
