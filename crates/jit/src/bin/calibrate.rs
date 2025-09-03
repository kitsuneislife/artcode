use std::path::Path;
use std::fs;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct Profile { functions: HashMap<String,u64>, _edges_map: Option<HashMap<String,u64>> }

#[derive(Deserialize)]
struct AotPlan { inline_candidates: Vec<InlineCandidate> }
#[derive(Deserialize)]
struct InlineCandidate { name: String, _score: i64 }

fn analyze_ir(path: &Path) -> Option<(usize, usize, usize, usize)> {
    // reuse parse_ir_file
        if let Some(a) = jit::ir_loader::parse_ir_file(path) {
        // parse_ir_file encodes instr_count weighted; we need raw counts but we'll approximate
        // For now return (instr_count, block_count, call_count=0, alloc_count=0) since loader tracks weighted
        return Some((a.instr_count, a.block_count, 0, 0));
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: calibrate <profile.json> <aot_plan.json> <ir_dir>");
        std::process::exit(2);
    }
    let prof_s = fs::read_to_string(&args[1]).expect("read profile");
    let plan_s = fs::read_to_string(&args[2]).expect("read plan");
    let ir_dir = Path::new(&args[3]);
    let profile: Profile = serde_json::from_str(&prof_s).expect("parse profile");
    let plan: AotPlan = serde_json::from_str(&plan_s).expect("parse plan");

    let mut x = Vec::new();
    let mut y = Vec::new();
    for c in plan.inline_candidates.iter() {
        let irp = ir_dir.join(format!("{}.ir", c.name));
        if irp.exists() {
            if let Some((instr, blocks, calls, allocs)) = analyze_ir(&irp) {
                x.push(vec![instr as f64, calls as f64, allocs as f64, blocks as f64]);
                let score = profile.functions.get(&c.name).cloned().unwrap_or(0) as f64;
                y.push(score);
            }
        }
    }
    if x.is_empty() {
        eprintln!("no candidates with ir found");
        std::process::exit(1);
    }

    // Simple linear fit using normal equations: solve W in XW = Y (least squares)
    let m = x.len();
    let n = 4;
    // build XtX and XtY
    let mut xtx = vec![vec![0f64; n]; n];
    let mut xty = vec![0f64; n];
    for i in 0..m {
        for a in 0..n {
            for b in 0..n {
                xtx[a][b] += x[i][a] * x[i][b];
            }
            xty[a] += x[i][a] * y[i];
        }
    }
    // solve via Gaussian elimination (n small)
    // augment
    for i in 0..n {
        xtx[i].push(xty[i]);
    }
    // Gaussian elim
    for i in 0..n {
        // pivot
        let mut pivot = i;
        for r in i..n { if xtx[r][i].abs() > xtx[pivot][i].abs() { pivot = r; } }
        if pivot != i { xtx.swap(i, pivot); }
        let div = xtx[i][i];
        if div.abs() < 1e-12 { continue; }
        for j in i..=n { xtx[i][j] /= div; }
        for r in 0..n { if r!=i { let factor = xtx[r][i]; for c in i..=n { xtx[r][c] -= factor * xtx[i][c]; } } }
    }
    let mut sol = vec![0f64; n];
    for i in 0..n { sol[i] = xtx[i][n]; }

    eprintln!("calibrated weights: instr={:.4}, call={:.4}, alloc={:.4}, block={:.4}", sol[0], sol[1], sol[2], sol[3]);

    // heuristically map to analyzer constants: prefer call weight > instr weight, alloc higher
    // We'll compute ratios and pick sensible ints
    let call_w = (sol[1].max(0.0) / sol[0].max(1e-6) * 1.0).max(1.0);
    let alloc_w = (sol[2].max(0.0) / sol[0].max(1e-6) * 1.0).max(1.0);
    let block_w = (sol[3].max(0.0) / sol[0].max(1e-6) * 1.0).max(1.0);

    eprintln!("suggested integer weights: call ~ {:.0}, alloc ~ {:.0}, block ~ {:.0}", call_w, alloc_w, block_w);
    eprintln!("NOTE: calibrator will only suggest weights; manual review required before updating analyzer.");
}
