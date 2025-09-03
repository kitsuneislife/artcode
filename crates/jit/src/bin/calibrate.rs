use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
struct Profile {
    functions: HashMap<String, u64>,
    _edges_map: Option<HashMap<String, u64>>,
}

#[derive(Deserialize)]
struct AotPlan {
    inline_candidates: Vec<InlineCandidate>,
}
#[derive(Deserialize)]
struct InlineCandidate {
    name: String,
    #[serde(flatten)]
    _other: serde_json::Value,
}

#[derive(Serialize)]
struct CalibrationSuggestion {
    instr_weight: usize,
    call_weight: usize,
    alloc_weight: usize,
    block_weight: usize,
    note: String,
}

fn analyze_ir(path: &Path) -> Option<(usize, usize, usize, usize)> {
    if let Some(a) = jit::ir_loader::parse_ir_file(path) {
        // parse_ir_file now returns raw counts
        return Some((a.instr_count, a.block_count, a.call_count, a.alloc_count));
    }
    None
}

// simple projected gradient descent for non-negative least-squares with tiny L2 regularization
fn nnls_projected_gradient(
    x: &Vec<Vec<f64>>,
    y: &Vec<f64>,
    reg: f64,
    iters: usize,
    lr: f64,
) -> Vec<f64> {
    let n = x[0].len();
    let m = x.len();
    let mut w = vec![0.1f64; n];
    for _ in 0..iters {
        let mut grad = vec![0f64; n];
        for i in 0..m {
            let mut pred = 0f64;
            for j in 0..n {
                pred += x[i][j] * w[j];
            }
            let err = pred - y[i];
            for j in 0..n {
                grad[j] += 2.0 * x[i][j] * err;
            }
        }
        for j in 0..n {
            grad[j] += 2.0 * reg * w[j];
        }
        for j in 0..n {
            w[j] = (w[j] - lr * grad[j]).max(0.0);
        }
    }
    w
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

    let mut x: Vec<Vec<f64>> = Vec::new();
    let mut y: Vec<f64> = Vec::new();
    for c in plan.inline_candidates.iter() {
        let irp = ir_dir.join(format!("{}.ir", c.name));
        if irp.exists() {
            if let Some((instr, blocks, calls, allocs)) = analyze_ir(&irp) {
                x.push(vec![
                    instr as f64,
                    calls as f64,
                    allocs as f64,
                    blocks as f64,
                ]);
                let score = profile.functions.get(&c.name).cloned().unwrap_or(0) as f64;
                y.push(score);
            }
        }
    }
    if x.is_empty() {
        eprintln!("no candidates with ir found");
        std::process::exit(1);
    }

    // normalize columns to avoid scale issues
    let n = x[0].len();
    let m = x.len();
    let mut scales = vec![1.0f64; n];
    for j in 0..n {
        let mut s = 0f64;
        for i in 0..m {
            s += x[i][j].abs();
        }
        if s > 0.0 {
            scales[j] = s / (m as f64);
        }
    }
    let mut xn = x.clone();
    for i in 0..m {
        for j in 0..n {
            xn[i][j] = xn[i][j] / scales[j];
        }
    }

    // run NNLS-like optimizer
    let reg = 1e-3;
    let sol = nnls_projected_gradient(&xn, &y, reg, 5000, 1e-4);
    // scale back
    let mut sol_scaled = vec![0f64; n];
    for j in 0..n {
        sol_scaled[j] = sol[j] / scales[j];
    }

    eprintln!(
        "calibrated (float) weights: instr={:.4}, call={:.4}, alloc={:.4}, block={:.4}",
        sol_scaled[0], sol_scaled[1], sol_scaled[2], sol_scaled[3]
    );

    // map to integer suggestions respecting minimums
    let instr_w = (sol_scaled[0].max(0.0)).max(1.0);
    let call_w = (sol_scaled[1].max(0.0)).max(1.0);
    let alloc_w = (sol_scaled[2].max(0.0)).max(1.0);
    let block_w = (sol_scaled[3].max(0.0)).max(1.0);

    let suggestion = CalibrationSuggestion {
        instr_weight: instr_w.round() as usize,
        call_weight: call_w.round() as usize,
        alloc_weight: alloc_w.round() as usize,
        block_weight: block_w.round() as usize,
        note: "Suggested weights produced by constrained calibrator (non-negative, L2-regularized). Review before applying.".to_string(),
    };

    let out = serde_json::to_string_pretty(&suggestion).expect("serialize suggestion");
    let out_path = Path::new("calibration_suggestion.json");
    fs::write(out_path, out.as_bytes()).expect("write suggestion");

    eprintln!(
        "wrote calibration_suggestion.json ({} {})",
        suggestion.instr_weight, suggestion.call_weight
    );
    eprintln!("note: do not auto-apply; open PR or manual review recommended");
}
