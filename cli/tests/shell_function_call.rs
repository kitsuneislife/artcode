use assert_cmd::Command;
use std::io::Write;

#[test]
fn run_executes_function_style_shell_call_and_matches_result() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create tmp file");
    write!(
        tmp,
        "let r = echo(\"cli_func_shell_ok\");\nmatch r {{\n  case .Ok(out): println(f\"OK={{out}}\")\n  case .Err(err): println(f\"ERR={{err}}\")\n}}\n"
    )
    .expect("write script");

    let path = tmp.path().to_str().expect("tmp path utf8");
    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("run").arg(path);

    let output = cmd.output().expect("run art run");
    assert!(output.status.success(), "run command should exit successfully");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("OK=cli_func_shell_ok"));
}

#[test]
fn run_function_style_shell_call_non_zero_exit_returns_err() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create tmp file");
    write!(
        tmp,
        "let r = sh(\"-c\", \"echo cli_func_shell_err 1>&2; exit 7\");\nmatch r {{\n  case .Ok(out): println(f\"OK={{out}}\")\n  case .Err(err): println(f\"ERR={{err}}\")\n}}\n"
    )
    .expect("write script");

    let path = tmp.path().to_str().expect("tmp path utf8");
    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("run").arg(path);

    let output = cmd.output().expect("run art run");
    assert!(output.status.success(), "run command should exit successfully");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("ERR=cli_func_shell_err"));
}
