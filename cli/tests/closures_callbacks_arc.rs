use assert_cmd::Command;
use std::io::Write;

#[test]
fn returned_closure_keeps_captured_environment() {
    let script = r#"
func make_adder(base) {
    func add(v) {
        return v + base
    }
    return add
}

let plus_two = make_adder(2)
println(plus_two(41))
"#;

    let mut tmp = tempfile::NamedTempFile::new().expect("tmp file");
    write!(tmp, "{}", script).expect("write script");
    let path = tmp.path().to_str().expect("utf8 path");

    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("run").arg(path);
    let output = cmd.output().expect("run art script");

    assert!(output.status.success(), "script should run successfully");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
    assert!(stdout.contains("43"), "expected closure result in stdout");
    assert!(
        !stderr.contains("Dangling closure environment"),
        "closure environment should stay valid after escaping"
    );
}

#[test]
fn callback_parameter_uses_captured_state() {
    let script = r#"
func apply_twice(cb, value) {
    return cb(cb(value))
}

let delta = 3
func add_delta(n) {
    return n + delta
}

println(apply_twice(add_delta, 10))
"#;

    let mut tmp = tempfile::NamedTempFile::new().expect("tmp file");
    write!(tmp, "{}", script).expect("write script");
    let path = tmp.path().to_str().expect("utf8 path");

    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("run").arg(path);
    let output = cmd.output().expect("run art script");

    assert!(output.status.success(), "script should run successfully");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("16"), "expected callback result in stdout");
}
