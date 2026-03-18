use assert_cmd::Command;
use std::io::Write;

#[test]
fn run_executes_expression_pipeline_chaining() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create tmp file");
    write!(
        tmp,
        "func inc(x: Int) -> Int {{ return x + 1 }}\nfunc mul(a: Int, b: Int) -> Int {{ return a * b }}\nprintln(10 |> inc |> mul(2));\n"
    )
    .expect("write script");

    let path = tmp.path().to_str().expect("tmp path utf8");
    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("run").arg(path);

    let output = cmd.output().expect("run art run");
    assert!(output.status.success(), "run command should exit successfully");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("22"), "expected pipeline expression result in stdout");
}

#[test]
fn run_executes_expression_pipeline_with_call_arguments() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create tmp file");
    write!(
        tmp,
        "func add(a: Int, b: Int) -> Int {{ return a + b }}\nprintln(5 |> add(7));\n"
    )
    .expect("write script");

    let path = tmp.path().to_str().expect("tmp path utf8");
    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("run").arg(path);

    let output = cmd.output().expect("run art run");
    assert!(output.status.success(), "run command should exit successfully");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("12"), "expected pipeline call result in stdout");
}
