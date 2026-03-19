use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

#[test]
fn resolver_parallel_keeps_import_order_deterministic() {
    let proj = TempDir::new().expect("proj");
    let main = proj.path().join("main.art");
    let a = proj.path().join("a.art");
    let b = proj.path().join("b.art");
    let c = proj.path().join("c.art");

    fs::write(&a, "let v = 10;").expect("write a.art");
    fs::write(&b, "import a; let marker = 2;").expect("write b.art");
    fs::write(&c, "import a; let marker = 3;").expect("write c.art");
    fs::write(
        &main,
        "import b;\nimport c;\nprintln(marker);\nprintln(v);\n",
    )
    .expect("write main.art");

    let mut cmd = Command::cargo_bin("art").expect("binary");
    cmd.arg("run").arg(main.to_str().expect("utf8 path"));
    let out = cmd.output().expect("run art");

    assert!(
        out.status.success(),
        "art failed: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8(out.stdout).expect("utf8 stdout");
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(
        lines.len() >= 2,
        "expected at least 2 lines, got: {}",
        stdout
    );

    assert_eq!(
        lines[0], "3",
        "import order should keep last override from c"
    );
    assert_eq!(
        lines[1], "10",
        "shared dependency should load correctly once"
    );
}
