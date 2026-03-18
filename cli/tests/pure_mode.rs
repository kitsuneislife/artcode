use assert_cmd::Command;
use tempfile::TempDir;

#[test]
fn run_pure_blocks_io_write_text() {
    let work = TempDir::new().expect("workdir");
    let script = work.path().join("pure_io.art");
    std::fs::write(
        &script,
        r#"
            io_write_text("/tmp/artcode_pure_mode_should_not_write.txt", "x");
        "#,
    )
    .expect("write script");

    let mut cmd = Command::cargo_bin("art").expect("binary");
    cmd.arg("run").arg("--pure").arg(script.to_str().unwrap());
    let out = cmd.output().expect("run art");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not allowed in --pure mode"),
        "stderr should mention pure mode block. stderr={} stdout={}",
        stderr,
        String::from_utf8_lossy(&out.stdout)
    );
}
