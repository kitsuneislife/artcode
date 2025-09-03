use serde_json;
use std::path::Path;

/// Generate a simple AOT plan JSON from a profile string and write to out_path.
pub fn generate_aot_plan_from_profile_str(profile_str: &str, out_path: &Path) -> Result<(), String> {
    let parsed: serde_json::Value = serde_json::from_str(profile_str).map_err(|e| format!("parse error: {}", e))?;
    // collect functions map
    let mut func_map: Vec<(String, u64)> = Vec::new();
    if let Some(funcs) = parsed.get("functions") {
        if let Some(map) = funcs.as_object() {
            for (k, v) in map.iter() {
                if let Some(n) = v.as_u64() {
                    func_map.push((k.clone(), n));
                }
            }
        }
    }
    let mut callers_by_callee: std::collections::HashMap<String, Vec<(String, u64)>> = std::collections::HashMap::new();
    if let Some(edges_val) = parsed.get("edges") {
        if edges_val.is_array() {
            if let Some(arr) = edges_val.as_array() {
                for e in arr.iter() {
                    if let (Some(caller), Some(callee), Some(cnt)) = (
                        e.get("caller").and_then(|v| v.as_str()),
                        e.get("callee").and_then(|v| v.as_str()),
                        e.get("count").and_then(|v| v.as_u64()),
                    ) {
                        callers_by_callee.entry(callee.to_string()).or_default().push((caller.to_string(), cnt));
                    }
                }
            }
        } else if edges_val.is_object() {
            if let Some(map) = edges_val.as_object() {
                for (k, v) in map.iter() {
                    if let Some(cnt) = v.as_u64() {
                        if let Some(pos) = k.find("->") {
                            let caller = &k[..pos];
                            let callee = &k[pos + 2..];
                            callers_by_callee.entry(callee.to_string()).or_default().push((caller.to_string(), cnt));
                        }
                    }
                }
            }
        }
    }
    let mut callee_score: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    for (name, cnt) in func_map.iter() {
        let edge_contrib = callers_by_callee.get(name).map(|v| v.iter().map(|(_,c)| *c).sum::<u64>()).unwrap_or(0);
        let score = *cnt + 2 * edge_contrib;
        callee_score.insert(name.clone(), score);
    }
    let mut scored: Vec<(String, u64)> = callee_score.into_iter().collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1));
    let mut plan = serde_json::Map::new();
    let mut inline: Vec<serde_json::Value> = Vec::new();
    for (name, score) in scored.iter().take(50) {
        if inline.len() >= 10 { break; }
        if *score < 3 { break; }
        let mut is_recursive = false;
        if let Some(callers) = callers_by_callee.get(name) {
            for (cname, _) in callers.iter() { if cname == name { is_recursive = true; break; } }
        }
        if is_recursive { continue; }
        let caller_examples = callers_by_callee.get(name).map(|v| {
            let mut vv = v.clone();
            vv.sort_by(|a,b| b.1.cmp(&a.1));
            vv.iter().take(3).map(|(c,n)| serde_json::json!({"caller": c, "count": *n})).collect::<Vec<_>>()
        }).unwrap_or_else(|| Vec::new());
        inline.push(serde_json::json!({"name": name, "score": *score, "caller_examples": caller_examples}));
    }
    plan.insert("inline_candidates".to_string(), serde_json::Value::Array(inline));
    std::fs::write(out_path, serde_json::to_string_pretty(&serde_json::Value::Object(plan)).map_err(|e| format!("serialize error: {}", e))?).map_err(|e| format!("write error: {}", e))?;
    Ok(())
}

/// Write a minimal AOT artifact JSON next to the plan. This is a small helper
/// used by the CLI to produce a consumable artifact file for downstream steps.
pub fn write_minimal_aot_artifact(plan_path: &Path, out_artifact: &Path) -> Result<(), String> {
    // Read plan and produce a tiny artifact that includes the plan plus a
    // metadata header.
    let plan_str = std::fs::read_to_string(plan_path).map_err(|e| format!("read plan: {}", e))?;
    let plan_json: serde_json::Value = serde_json::from_str(&plan_str).map_err(|e| format!("parse plan: {}", e))?;
    let artifact = serde_json::json!({
        "format_version": 1,
        "source": plan_path.file_name().and_then(|n| n.to_str()).unwrap_or("aot_plan.json"),
        "plan": plan_json,
    });
    std::fs::write(out_artifact, serde_json::to_string_pretty(&artifact).map_err(|e| format!("serialize artifact: {}", e))?).map_err(|e| format!("write artifact: {}", e))?;
    // Optional: if ART_BUILD_PACKAGE=1 environment variable is set, attempt to
    // create a tar.gz package from a sibling directory named `<plan>.artifact_files/`.
    if std::env::var("ART_BUILD_PACKAGE").unwrap_or_default() == "1" {
        if let Some(plan_stem) = plan_path.file_stem().and_then(|s| s.to_str()) {
            let pkg_dir = plan_path.with_file_name(format!("{}.artifact_files", plan_stem));
            if pkg_dir.exists() && pkg_dir.is_dir() {
                let tar_name = out_artifact.with_extension("tar.gz");
                // call system `tar` to avoid adding tar crate dependency
                let status = std::process::Command::new("tar")
                    .arg("-czf")
                    .arg(&tar_name)
                    .arg("-C")
                    .arg(pkg_dir.to_str().unwrap())
                    .arg(".")
                    .status();
                match status {
                    Ok(s) if s.success() => {
                        // compute sha256
                        if let Ok(mut f) = std::fs::read(&tar_name) {
                            use sha2::{Digest, Sha256};
                            let mut hasher = Sha256::new();
                            hasher.update(&f);
                            let sum = hasher.finalize();
                            let hex = hex::encode(sum);
                            // update artifact to include package reference
                            let mut artifact_map = artifact.as_object().cloned().unwrap_or_default();
                            artifact_map.insert("package".to_string(), serde_json::json!({"path": tar_name.file_name().and_then(|n| n.to_str()).unwrap_or("artifact.tar.gz"), "sha256": hex}));
                            std::fs::write(out_artifact, serde_json::to_string_pretty(&serde_json::Value::Object(artifact_map)).map_err(|e| format!("serialize artifact: {}", e))?).map_err(|e| format!("write artifact: {}", e))?;
                        }
                    }
                    _ => {
                        // best-effort: don't fail the whole operation if tar not available
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn generate_aot_plan_from_sample_profile() {
        let tmp = env::temp_dir().join("art_profile_sample.json");
        let out = env::temp_dir().join("aot_plan_sample.json");
        let profile = serde_json::json!({
            "functions": {"foo": 5, "bar": 2, "baz": 1},
            "edges": [{"caller":"<root>", "callee":"foo", "count":3}, {"caller":"foo","callee":"bar","count":4}]
        });
        fs::write(&tmp, serde_json::to_string(&profile).unwrap()).unwrap();
        let res = generate_aot_plan_from_profile_str(&fs::read_to_string(&tmp).unwrap(), &out.as_path());
        assert!(res.is_ok());
        let s = fs::read_to_string(&out).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert!(v.get("inline_candidates").is_some());
        let _ = fs::remove_file(&tmp);
        let _ = fs::remove_file(&out);
    }
}
