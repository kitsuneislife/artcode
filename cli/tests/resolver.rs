use assert_cmd::Command;

#[test]
fn resolver_handles_simple_import() {
    // create two temp files: b.art and a.art where a imports b
        // Create a temp directory and write b.art and a.art inside it so relative imports work.
        let dir = tempfile::tempdir().expect("tempdir");
        let bpath = dir.path().join("b.art");
        std::fs::write(&bpath, "let x = 42;").expect("write b");
        let apath = dir.path().join("a.art");
        // import by filename without extension; include trailing semicolon to satisfy parser
        std::fs::write(&apath, format!("import {};\nlet y = x;", "b")).expect("write a");

    let mut cmd = Command::cargo_bin("art").expect("binary present");
        cmd.arg("run").arg(apath.to_str().unwrap());
    let output = cmd.output().expect("run art");
    // We don't assert on stdout, just that resolution & run doesn't crash
    assert!(output.status.success(), "art exited non-zero for resolver test");
}
