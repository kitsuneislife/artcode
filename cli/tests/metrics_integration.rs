use assert_cmd::Command;
// predicates not required for this test
use serde_json::Value;
use std::io::Write;

// Integration test: run `art metrics --json` on a small script and validate JSON
#[test]
fn metrics_json_includes_arena_and_finalized_maps() {
    // include the example source and write to a temp file to avoid path issues
    let example = include_str!("../examples/00_hello.art");
    let mut tmp = tempfile::NamedTempFile::new().expect("create tmp file");
    write!(tmp, "{}", example).expect("write script");
    let path = tmp.path().to_str().unwrap();

    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("metrics").arg("--json").arg(path);
    let output = cmd.output().expect("run art");
    assert!(output.status.success(), "art exited non-zero");
    let s = String::from_utf8(output.stdout).expect("utf8");
    // find first JSON object start to tolerate program output before metrics JSON
    let start = s.find('{').expect("no json object start");
    let json_part = &s[start..];
    let v: Value = serde_json::from_str(json_part).expect("valid json");
    // basic presence
    let a_alloc = v
        .get("arena_alloc_count")
        .expect("missing arena_alloc_count");
    let a_final = v
        .get("objects_finalized_per_arena")
        .expect("missing objects_finalized_per_arena");
    let a_prom = v
        .get("finalizer_promotions_per_arena")
        .expect("missing finalizer_promotions_per_arena");

    // ensure top-level numeric fields exist and are non-negative
    if let Some(n) = v.get("handled_errors") {
        assert!(n.as_u64().is_some(), "handled_errors must be an integer");
    }
    if let Some(n) = v.get("executed_statements") {
        assert!(
            n.as_u64().is_some(),
            "executed_statements must be an integer"
        );
    }
    if let Some(c) = v.get("crash_free") {
        assert!(
            c.as_f64().is_some() || c.as_u64().is_some(),
            "crash_free must be numeric"
        );
    }

    // arena maps should be objects mapping numeric string keys to numeric values
    let validate_map = |m: &serde_json::Value, name: &str| {
        assert!(m.is_object(), "{} should be a JSON object", name);
        for (k, v) in m.as_object().unwrap() {
            // keys should parse as u32
            k.parse::<u32>()
                .expect(&format!("{} key '{}' not a u32", name, k));
            assert!(
                v.as_u64().is_some(),
                "{} value for key {} must be integer",
                name,
                k
            );
            // value non-negative by definition of unsigned
        }
    };

    validate_map(a_alloc, "arena_alloc_count");
    validate_map(a_final, "objects_finalized_per_arena");
    validate_map(a_prom, "finalizer_promotions_per_arena");
}
