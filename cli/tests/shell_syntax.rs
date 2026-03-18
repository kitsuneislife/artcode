use assert_cmd::Command;
use std::io::Write;

#[test]
fn run_executes_shell_syntax_statement() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create tmp file");
    write!(tmp, "$ echo cli_shell_ok;\n").expect("write script");

    let path = tmp.path().to_str().expect("tmp path utf8");
    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("run").arg(path);

    let output = cmd.output().expect("run art run");
    assert!(output.status.success(), "run command should exit successfully");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(
        stdout.contains("cli_shell_ok"),
        "expected shell command output in stdout"
    );
}
