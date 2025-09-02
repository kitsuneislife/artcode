use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, serde::Deserialize)]
struct Profile {
    functions: HashMap<String, u64>,
    edges: Option<Vec<EdgeRecord>>,
    edges_map: Option<HashMap<String, u64>>,
}

#[derive(Debug, serde::Deserialize)]
struct EdgeRecord {
    caller: String,
    callee: String,
    count: u64,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct AotPlan {
    inline_candidates: Vec<InlineCandidate>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct InlineCandidate {
    name: String,
    score: i64,
    #[serde(default)]
    caller_examples: Vec<CallerExample>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct CallerExample {
    caller: String,
    count: u64,
}

fn load_profile(path: &Path) -> Result<Profile, String> {
    let s = fs::read_to_string(path).map_err(|e| format!("read profile: {}", e))?;
    serde_json::from_str(&s).map_err(|e| format!("parse profile json: {}", e))
}

fn load_plan(path: &Path) -> Result<AotPlan, String> {
    let s = fs::read_to_string(path).map_err(|e| format!("read plan: {}", e))?;
    serde_json::from_str(&s).map_err(|e| format!("parse plan json: {}", e))
}

fn normalize_plan(mut plan: AotPlan) -> AotPlan {
    // Simple normalizations:
    // - ensure score >= 1
    // - cap score to reasonable upper bound (1_000_000)
    // - dedupe caller_examples by caller name summing counts
    for c in plan.inline_candidates.iter_mut() {
        if c.score < 1 {
            c.score = 1;
        }
        if c.score > 1_000_000 {
            c.score = 1_000_000;
        }
        let mut map: HashMap<String, u64> = HashMap::new();
        for ex in c.caller_examples.drain(..) {
            *map.entry(ex.caller).or_insert(0) += ex.count;
        }
        c.caller_examples = map
            .into_iter()
            .map(|(caller, count)| CallerExample { caller, count })
            .collect();
    }
    // sort candidates by score desc to make output deterministic
    plan
        .inline_candidates
        .sort_by(|a, b| b.score.cmp(&a.score));
    plan
}

fn validate_consistency(profile: &Profile, plan: &AotPlan) -> Vec<String> {
    let mut errs = Vec::new();
    // All inline candidate names should exist in profile.functions
    for c in &plan.inline_candidates {
        if !profile.functions.contains_key(&c.name) {
            errs.push(format!("candidate '{}' missing in profile.functions", c.name));
        }
        for ex in &c.caller_examples {
            if !profile.functions.contains_key(&ex.caller) && ex.caller != "<root>" {
                errs.push(format!("caller '{}' (for {}) missing in profile.functions", ex.caller, c.name));
            }
        }
    }
    // Check edges_map presence for quick lookups
    if profile.edges_map.is_none() {
        errs.push("profile missing edges_map; edges lookups will be slower".to_string());
    }
    errs
}

fn write_normalized(path: &Path, plan: &AotPlan) -> Result<(), String> {
    let out = serde_json::to_string_pretty(plan).map_err(|e| format!("serialize plan: {}", e))?;
    fs::write(path, out).map_err(|e| format!("write plan: {}", e))
}

fn print_summary(profile: &Profile, plan: &AotPlan) {
    eprintln!("Profile: {} functions", profile.functions.len());
    let total_calls: u64 = profile.functions.values().sum();
    eprintln!("Total calls (sum of counters): {}", total_calls);
    eprintln!("AOT inline candidates: {}", plan.inline_candidates.len());
    for c in &plan.inline_candidates {
        eprintln!("- {} (score {}) - {} callers", c.name, c.score, c.caller_examples.len());
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: aot_inspect <profile.json> <aot_plan.json>");
        std::process::exit(2);
    }
    let profile_path = Path::new(&args[1]);
    let plan_path = Path::new(&args[2]);

    let profile = match load_profile(profile_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
    };
    let plan = match load_plan(plan_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
    };

    let mut out = normalize_plan(plan);
    let issues = validate_consistency(&profile, &out);
    print_summary(&profile, &out);
    if !issues.is_empty() {
        eprintln!("issues:");
        for it in &issues {
            eprintln!(" - {}", it);
        }
    }

    let out_path = plan_path.with_file_name("aot_plan.normalized.json");
    if let Err(e) = write_normalized(&out_path, &out) {
        eprintln!("failed to write normalized plan: {}", e);
        std::process::exit(1);
    }
    eprintln!("wrote normalized plan to {:?}", out_path);
}
