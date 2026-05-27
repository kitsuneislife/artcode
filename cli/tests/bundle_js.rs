use assert_cmd::Command;
use tempfile::TempDir;

#[test]
fn bundle_hello_contains_runtime_and_entry() {
    let work = TempDir::new().expect("tempdir");
    let out_dir = work.path().join("dist");
    let script = work.path().join("hello.art");

    std::fs::write(&script, "println(\"Hello, Artcode!\");").expect("write script");

    let mut cmd = Command::cargo_bin("art").expect("binary");
    cmd.args(["build", script.to_str().unwrap(), "--target", "js", "--bundle", "--out", out_dir.to_str().unwrap()]);
    cmd.assert().success();

    let js = std::fs::read_to_string(out_dir.join("hello.js")).expect("read output");
    assert!(js.contains("const println"), "runtime preamble missing");
    assert!(js.contains("console.log"), "println runtime definition missing");
    assert!(js.contains("println(\"Hello, Artcode!\")"), "entry code missing");
}

#[test]
fn bundle_inlines_imported_module() {
    let work = TempDir::new().expect("tempdir");
    let out_dir = work.path().join("dist");

    std::fs::write(work.path().join("lib.art"), "func greet(n) { return n; }").expect("write lib");
    std::fs::write(work.path().join("main.art"), "import lib;\nprintln(greet(\"ok\"));").expect("write main");

    let mut cmd = Command::cargo_bin("art").expect("binary");
    cmd.args(["build", work.path().join("main.art").to_str().unwrap(),
              "--target", "js", "--bundle", "--out", out_dir.to_str().unwrap()]);
    cmd.assert().success();

    let js = std::fs::read_to_string(out_dir.join("main.js")).expect("read output");
    assert!(js.contains("function greet"), "inlined module function missing");
    assert!(!js.contains("import *"), "import statement should be suppressed in bundle");
}

#[test]
fn bundle_no_duplicate_modules() {
    let work = TempDir::new().expect("tempdir");
    let out_dir = work.path().join("dist");

    std::fs::write(work.path().join("util.art"), "func id(x) { return x; }").expect("write util");
    // Both main and lib import util — util should appear only once.
    std::fs::write(work.path().join("lib.art"), "import util;\nfunc wrap(x) { return id(x); }").expect("write lib");
    std::fs::write(work.path().join("main.art"), "import util;\nimport lib;\nprintln(wrap(\"hi\"));").expect("write main");

    let mut cmd = Command::cargo_bin("art").expect("binary");
    cmd.args(["build", work.path().join("main.art").to_str().unwrap(),
              "--target", "js", "--bundle", "--out", out_dir.to_str().unwrap()]);
    cmd.assert().success();

    let js = std::fs::read_to_string(out_dir.join("main.js")).expect("read output");
    // "function id" should appear exactly once
    let count = js.matches("function id(").count();
    assert_eq!(count, 1, "module util was inlined {count} times, expected 1");
}
