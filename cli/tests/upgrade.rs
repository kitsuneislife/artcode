use assert_cmd::Command;
use std::io::Write;

#[test]
fn upgrade_check_reports_legacy_builtins() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create tmp file");
    write!(
        tmp,
        "let x = __weak(obj);\nlet y = __unowned_get(ptr);\nprintln(x);\n"
    )
    .expect("write script");

    let path = tmp.path().to_str().expect("tmp path utf8");
    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("upgrade").arg("--check").arg(path);

    let output = cmd.output().expect("run art upgrade --check");
    assert!(output.status.success(), "upgrade command should succeed");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("art upgrade report"));
    assert!(stdout.contains("__weak("));
    assert!(stdout.contains("weak("));
    assert!(stdout.contains("__unowned_get("));
    assert!(stdout.contains("unowned_get("));
}
