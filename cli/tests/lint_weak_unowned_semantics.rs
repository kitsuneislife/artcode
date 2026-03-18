use assert_cmd::Command;
use std::io::Write;

#[test]
fn lint_reports_misuse_of_weak_unowned_postfix_operators() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create tmp file");
    write!(
        tmp,
        "let arr = [1, 2, 3];\nlet bad_weak = arr?;\nlet bad_unowned = arr!;\n"
    )
    .expect("write script");

    let path = tmp.path().to_str().expect("tmp path utf8");
    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("lint").arg(path);

    let output = cmd.output().expect("run art lint");
    assert!(output.status.success(), "lint command should exit successfully");

    let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
    assert!(
        stderr.contains("Weak upgrade misuse: postfix `?` expects a weak reference expression."),
        "expected weak misuse lint warning"
    );
    assert!(
        stderr.contains("Unowned access misuse: postfix `!` expects an unowned reference expression."),
        "expected unowned misuse lint warning"
    );
}
