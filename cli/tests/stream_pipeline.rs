use assert_cmd::Command;
use std::io::Write;

#[test]
fn run_executes_lazy_stream_pipeline_collect_and_count() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create tmp file");
    write!(
        tmp,
        "func inc(x: Int) -> Int {{ return x + 1 }}\nfunc is_even(x: Int) -> Bool {{ return ((x / 2) * 2) == x }}\nlet data = [1, 2, 3, 4, 5] |> stream |> map(inc) |> filter(is_even) |> collect;\nprintln(data);\nprintln([1, 2, 3, 4, 5] |> stream |> map(inc) |> filter(is_even) |> count);\n"
    )
    .expect("write script");

    let path = tmp.path().to_str().expect("tmp path utf8");
    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("run").arg(path);

    let output = cmd.output().expect("run art run");
    assert!(output.status.success(), "run command should exit successfully");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("[2, 4, 6]"), "expected collected stream output");
    assert!(stdout.contains("3"), "expected stream count output");
}

#[test]
fn run_allows_for_iteration_over_stream_pipeline() {
    let mut tmp = tempfile::NamedTempFile::new().expect("create tmp file");
    write!(
        tmp,
        "func inc(x: Int) -> Int {{ return x + 1 }}\nfunc is_even(x: Int) -> Bool {{ return ((x / 2) * 2) == x }}\nfor n in [1, 2, 3, 4, 5] |> stream |> map(inc) |> filter(is_even) {{\n  println(n);\n}}\n"
    )
    .expect("write script");

    let path = tmp.path().to_str().expect("tmp path utf8");
    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("run").arg(path);

    let output = cmd.output().expect("run art run");
    assert!(output.status.success(), "run command should exit successfully");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("2"), "expected first stream value in loop output");
    assert!(stdout.contains("4"), "expected second stream value in loop output");
    assert!(stdout.contains("6"), "expected third stream value in loop output");

    let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
    assert!(
        !stderr.contains("Cannot iterate over unsupported type"),
        "for-loop over stream should not emit unsupported-iteration diagnostic"
    );
}
