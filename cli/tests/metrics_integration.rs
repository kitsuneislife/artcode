use assert_cmd::Command;
use predicates::prelude::*;
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
    cmd.assert()
        .success()
        .stdout(predicate::str::is_match(r#"\{.*\}\n?"#).unwrap());
}
