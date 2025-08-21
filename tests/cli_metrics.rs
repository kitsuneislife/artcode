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
    // Basic presence checks
    assert!(parsed.get("handled_errors").is_some(), "missing handled_errors");
    assert!(parsed.get("executed_statements").is_some(), "missing executed_statements");
    assert!(parsed.get("finalizer_promotions").is_some(), "missing finalizer_promotions");

    // Numeric sanity checks
    let executed = parsed["executed_statements"].as_u64().expect("executed_statements should be a positive integer");
    assert!(executed > 0, "executed_statements should be > 0");

    let crash_free = parsed["crash_free"].as_f64().expect("crash_free should be a number");
    assert!(crash_free >= 0.0 && crash_free <= 100.0, "crash_free should be between 0 and 100");

    let finalizer_promotions = parsed["finalizer_promotions"].as_u64().expect("finalizer_promotions should be a number");
    assert!(finalizer_promotions >= 0, "finalizer_promotions must be >= 0");

    // Expect no handled errors for the demo example
    let handled = parsed["handled_errors"].as_u64().expect("handled_errors must be numeric");
    assert_eq!(handled, 0, "expected handled_errors == 0 for demo example");

    // Memory counters should be present and non-negative
    for k in &["weak_created", "weak_upgrades", "weak_dangling", "unowned_created", "unowned_dangling", "cycle_reports_run"] {
        let v = parsed.get(*k).and_then(|vv| vv.as_u64()).expect(&format!("{} missing or not numeric", k));
        assert!(v <= 1_000_000, "{} seems unreasonably large: {}", k, v);
    }
}
