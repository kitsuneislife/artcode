use std::fs;
use std::path::Path;

fn load_plan(path: &Path) -> Result<serde_json::Value, String> {
    let s = fs::read_to_string(path).map_err(|e| format!("read plan: {}", e))?;
    serde_json::from_str(&s).map_err(|e| format!("parse plan: {}", e))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: aot_consumer <aot_plan.json>");
        std::process::exit(2);
    }
    let plan_path = Path::new(&args[1]);
    let plan = match load_plan(plan_path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
    };
    // Minimal action: list inline candidates and pretend to schedule compile order by score
    if let Some(arr) = plan.get("inline_candidates").and_then(|v| v.as_array()) {
        let mut vec = arr.clone();
        vec.sort_by(|a, b| {
            let sa = a.get("score").and_then(|s| s.as_i64()).unwrap_or(0);
            let sb = b.get("score").and_then(|s| s.as_i64()).unwrap_or(0);
            sb.cmp(&sa)
        });
        eprintln!("Scheduled JIT compile order:");
        for item in vec {
            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("<unknown>");
            let score = item.get("score").and_then(|v| v.as_i64()).unwrap_or(0);
            eprintln!("- {} (score={})", name, score);
        }
    } else {
        eprintln!("no inline_candidates found");
    }
}
