use assert_cmd::Command;
use std::io::Write;

#[test]
fn resolver_handles_simple_import() {
    // create two temp files: b.art and a.art where a imports b
    let mut b = tempfile::NamedTempFile::new().expect("create b");
    write!(b, "let x = 42;").expect("write b");
    let bpath = b.path().to_str().unwrap().to_string();
    // a imports b via path without extension
    let mut a = tempfile::NamedTempFile::new().expect("create a");
    // construct relative import using filename only
    let bname = std::path::Path::new(&bpath).file_name().unwrap().to_str().unwrap().to_string();
    write!(a, "import {}\nlet y = x;", bname).expect("write a");
    let apath = a.path().to_str().unwrap();

    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("run").arg(apath);
    let output = cmd.output().expect("run art");
    // We don't assert on stdout, just that resolution & run doesn't crash
    assert!(output.status.success(), "art exited non-zero for resolver test");
}
