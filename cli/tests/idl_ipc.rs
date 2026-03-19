use assert_cmd::Command;
use std::io::Write;

#[test]
fn run_idl_ipc_schema_and_validation() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create tmp file");
    write!(
        tmp,
        "struct BootMsg {{ service: String, retries: Int }}\nlet m = BootMsg {{ service: \"nexus\", retries: 3 }}\nlet ok = idl_validate(m, \"BootMsg\")\nprintln(f\"ok={{ok}}\")\n"
    )
    .expect("write script");

    let path = tmp.path().to_str().expect("tmp path utf8");
    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("run").arg(path);

    let output = cmd.output().expect("run art run");
    assert!(output.status.success(), "run command should exit successfully");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("ok=true"));
}

#[test]
fn run_idl_ipc_detects_invalid_payload_type() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create tmp file");
    write!(
        tmp,
        "struct BootMsg {{ service: String, retries: Int }}\nlet bad = BootMsg {{ service: \"nexus\", retries: \"oops\" }}\nlet ok = idl_validate(bad, \"BootMsg\")\nprintln(f\"ok={{ok}}\")\n"
    )
    .expect("write script");

    let path = tmp.path().to_str().expect("tmp path utf8");
    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("run").arg(path);

    let output = cmd.output().expect("run art run");
    assert!(output.status.success(), "run command should exit successfully");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("ok=false"));
}
