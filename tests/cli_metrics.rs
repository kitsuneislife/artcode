use std::process::Command;
use std::fs;

#[test]
fn cli_metrics_json_output() {
    // Ensure the binary exists; CI builds it in advance, locally we assume target/debug/art exists.
    let bin = "target/debug/art";
    assert!(std::path::Path::new(bin).exists(), "CLI binary not found at {}", bin);

    let example = "cli/examples/99_weak_unowned_demo.art";
    assert!(std::path::Path::new(example).exists(), "Example script not found: {}", example);

    let out = Command::new(bin)
        .arg("metrics")
        .arg("--json")
        .arg(example)
        .output()
        .expect("failed to run cli metrics");

    let stdout = String::from_utf8_lossy(&out.stdout);
    // stdout should be a single JSON object
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("output was not valid JSON");
    assert!(parsed.get("handled_errors").is_some(), "missing handled_errors");
    assert!(parsed.get("executed_statements").is_some(), "missing executed_statements");
    assert!(parsed.get("finalizer_promotions").is_some(), "missing finalizer_promotions");
}
