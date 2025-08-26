use assert_cmd::Command;
use tempfile::NamedTempFile;
use std::fs;

#[test]
fn cli_build_with_profile_end_to_end() {
    // create temp files for profile and output
    let profile_tmp = NamedTempFile::new().expect("create tmp profile");
    let out_tmp = NamedTempFile::new().expect("create tmp out");
    let profile_path = profile_tmp.path().to_path_buf();
    let out_path = out_tmp.path().to_path_buf();

    // build a minimal profile using Interpreter internals
    let mut interp = interpreter::interpreter::Interpreter::new();
    interp.call_counters.insert("foo".to_string(), 5);
    interp.call_counters.insert("bar".to_string(), 2);
    interp.edge_counters.insert("<root>->foo".to_string(), 3);
    interp.edge_counters.insert("foo->bar".to_string(), 4);
    interp.write_profile(&profile_path).expect("write profile");

    // invoke the CLI binary
    let mut cmd = Command::cargo_bin("art").expect("binary exists");
    cmd.arg("build")
        .arg("--with-profile")
        .arg(profile_path.to_str().unwrap())
        .arg("--out")
        .arg(out_path.to_str().unwrap());
    cmd.assert().success();

    // validate output file exists and has inline_candidates
    let s = fs::read_to_string(&out_path).expect("read aot plan");
    let v: serde_json::Value = serde_json::from_str(&s).expect("parse aot plan json");
    assert!(v.get("inline_candidates").is_some());
}
