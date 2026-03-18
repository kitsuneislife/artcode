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

#[test]
fn run_executes_shell_pipeline_syntax_statement() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create tmp file");
    write!(tmp, "$ echo cli_pipe_ok |> tr a-z A-Z;\n").expect("write script");

    let path = tmp.path().to_str().expect("tmp path utf8");
    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("run").arg(path);

    let output = cmd.output().expect("run art run");
    assert!(output.status.success(), "run command should exit successfully");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(
        stdout.contains("CLI_PIPE_OK"),
        "expected piped shell command output in stdout"
    );
}

#[test]
fn run_exposes_shell_result_for_pattern_matching_ok() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create tmp file");
    write!(
        tmp,
        "$ echo shell_typed_ok;\nmatch shell_result {{\n  case .Ok(out): println(f\"OK={{out}}\")\n  case .Err(err): println(f\"ERR={{err}}\")\n}}\n"
    )
    .expect("write script");

    let path = tmp.path().to_str().expect("tmp path utf8");
    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("run").arg(path);

    let output = cmd.output().expect("run art run");
    assert!(output.status.success(), "run command should exit successfully");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("OK=shell_typed_ok"), "expected Result.Ok payload");
}

#[test]
fn run_exposes_shell_result_for_pattern_matching_err() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create tmp file");
    write!(
        tmp,
        "$ sh -c \"echo shell_typed_err 1>&2; exit 7\";\nmatch shell_result {{\n  case .Ok(out): println(f\"OK={{out}}\")\n  case .Err(err): println(f\"ERR={{err}}\")\n}}\n"
    )
    .expect("write script");

    let path = tmp.path().to_str().expect("tmp path utf8");
    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("run").arg(path);

    let output = cmd.output().expect("run art run");
    assert!(output.status.success(), "run command should exit successfully");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("ERR=shell_typed_err"), "expected Result.Err payload");
}
