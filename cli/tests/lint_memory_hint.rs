use assert_cmd::Command;
use std::io::Write;

#[test]
fn lint_reports_allocation_hotspot_hint_for_loops() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create tmp file");
    write!(
        tmp,
        "for i in ([0, 1, 2]) {{ let pair = (i, [i, i + 1]); println(pair); }}"
    )
    .expect("write script");

    let path = tmp.path().to_str().expect("tmp path utf8");
    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("lint").arg(path);

    let output = cmd.output().expect("run art lint");
    assert!(output.status.success(), "lint command should exit successfully");

    let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
    assert!(
        stderr.contains("Potential allocation hotspot in loop body"),
        "expected allocation hotspot hint in lint diagnostics"
    );
}
