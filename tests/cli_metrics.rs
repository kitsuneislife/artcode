use std::process::Command;
use std::fs;

#[test]
fn cli_metrics_json_output() {
    // Ensure the binary exists; CI builds it in advance, locally we assume target/debug/art exists.
    let bin = "target/debug/art";
    assert!(std::path::Path::new(bin).exists(), "CLI binary not found at {}", bin);

    let example = "cli/examples/99_weak_unowned_demo.art";
    assert!(std::path::Path::new(example).exists(), "Example script not found: {}", example);

    let out = match Command::new(bin).arg("metrics").arg("--json").arg(example).output() {
        Ok(o) => o,
        Err(e) => panic!("failed to run cli metrics: {:?}", e),
    };

    let stdout = String::from_utf8_lossy(&out.stdout);
    // stdout should be a single JSON object
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => panic!("output was not valid JSON: {:?}; stdout='{}'", e, stdout),
    };
    // Basic presence checks
    assert!(parsed.get("handled_errors").is_some(), "missing handled_errors");
    assert!(parsed.get("executed_statements").is_some(), "missing executed_statements");
    assert!(parsed.get("finalizer_promotions").is_some(), "missing finalizer_promotions");

    // Numeric sanity checks
    let executed = match parsed["executed_statements"].as_u64() {
        Some(v) => v,
        None => panic!("executed_statements should be a positive integer, got: {:?}", parsed["executed_statements"]),
    };
    assert!(executed > 0, "executed_statements should be > 0");

    let crash_free = match parsed["crash_free"].as_f64() {
        Some(v) => v,
        None => panic!("crash_free should be a number, got: {:?}", parsed["crash_free"]),
    };
    assert!(crash_free >= 0.0 && crash_free <= 100.0, "crash_free should be between 0 and 100");

    let finalizer_promotions = match parsed["finalizer_promotions"].as_u64() {
        Some(v) => v,
        None => panic!("finalizer_promotions should be a number, got: {:?}", parsed["finalizer_promotions"]),
    };
    assert!(finalizer_promotions >= 0, "finalizer_promotions must be >= 0");

    // Expect no handled errors for the demo example
    let handled = match parsed["handled_errors"].as_u64() {
        Some(v) => v,
        None => panic!("handled_errors must be numeric, got: {:?}", parsed["handled_errors"]),
    };
    assert_eq!(handled, 0, "expected handled_errors == 0 for demo example");

    // Memory counters should be present and non-negative
    for k in &["weak_created", "weak_upgrades", "weak_dangling", "unowned_created", "unowned_dangling", "cycle_reports_run"] {
        let v = match parsed.get(*k).and_then(|vv| vv.as_u64()) {
            Some(v) => v,
            None => panic!("{} missing or not numeric: {:?}", k, parsed.get(*k)),
        };
        assert!(v <= 1_000_000, "{} seems unreasonably large: {}", k, v);
    }
}
