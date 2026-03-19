use assert_cmd::Command;
use std::io::Write;

#[test]
fn run_reuses_arena_with_callback() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create tmp file");
    write!(
        tmp,
        "let aid = arena_new();\nfunc work() {{\n  let _x = [1, 2, 3];\n  println(\"tick\");\n}}\narena_with(aid, work);\narena_with(aid, work);\nprintln(f\"done={{aid}}\");\n"
    )
    .expect("write script");

    let path = tmp.path().to_str().expect("tmp path utf8");
    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("run").arg(path);

    let output = cmd.output().expect("run art run");
    assert!(output.status.success(), "run command should exit successfully");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert_eq!(stdout.matches("tick").count(), 2);
    assert!(stdout.contains("done="));
}

#[test]
fn run_reusable_arena_release_reports_invalid_id() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create tmp file");
    write!(
        tmp,
        "let ok = arena_release(9999);\nprintln(f\"ok={{ok}}\")\n"
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
