use std::collections::HashMap;
use crate::{Function, Instr};

/// Very small SSA renaming pass: maps per-function temps with pattern `%<name>_<n>` to
/// normalized names `%t0`, `%t1`, ... in order of first definition. This keeps golden
/// outputs stable and prevents accidental name leaks when lowering generates ad-hoc temps.
pub fn rename_temps(func: &mut Function) {
    let mut map: HashMap<String, String> = HashMap::new();
    let mut next: usize = 0;

    // helper to map a value if it looks like a temp (starts with '%')
    let mut map_name = |s: &str| -> String {
        if !s.starts_with('%') {
            return s.to_string();
        }
        if let Some(m) = map.get(s) {
            return m.clone();
        }
        let new = format!("%t{}", next);
        next += 1;
        map.insert(s.to_string(), new.clone());
        new
    };

    // Walk instructions and rewrite in place
    for instr in func.body.iter_mut() {
        match instr {
            Instr::ConstI64(name, _) => {
                let n = map_name(name);
                *name = n;
            }
            Instr::Add(dest,a,b) | Instr::Sub(dest,a,b) | Instr::Mul(dest,a,b) | Instr::Div(dest,a,b) => {
                let nd = map_name(dest);
                let na = map_name(a);
                let nb = map_name(b);
                *dest = nd; *a = na; *b = nb;
            }
            Instr::Call(dest, _fnn, args) => {
                let nd = map_name(dest);
                let mut narr: Vec<String> = Vec::new();
                for a in args.iter() { narr.push(map_name(a)); }
                *dest = nd; *args = narr;
                // fn name left unchanged
            }
            Instr::Label(_) => { /* labels keep original names */ }
            Instr::Br(_) => { /* branch labels unchanged */ }
            Instr::BrCond(pred, _t, _f) => {
                let np = map_name(pred);
                *pred = np; // targets unchanged
                // t and f are labels, keep them
            }
            Instr::Phi(dest, _ty, pairs) => {
                let nd = map_name(dest);
                for (v, _bb) in pairs.iter_mut() {
                    let nv = map_name(v);
                    *v = nv;
                }
                *dest = nd;
            }
            Instr::Ret(opt) => {
                if let Some(v) = opt {
                    let nv = map_name(v);
                    *v = nv;
                }
            }
        }
    }
}
