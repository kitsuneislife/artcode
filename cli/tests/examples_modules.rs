use assert_cmd::Command;
use tempfile::TempDir;

#[test]
fn run_modules_example() {
    // Create a temp working dir and copy the example there
    let work = TempDir::new().expect("workdir");
    // Use CARGO_MANIFEST_DIR so test finds the example independent of cwd when running tests
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let example_dir = manifest_dir.join("examples/modules/demo");
    let dst = work.path().join("demo");
    std::fs::create_dir_all(&dst).expect("create dst");
    // copy files
    for entry in std::fs::read_dir(example_dir).expect("read example") {
        let entry = entry.expect("entry");
        let file_name = entry.file_name();
        let src = entry.path();
        let dest = dst.join(file_name);
        if src.is_file() {
            std::fs::copy(&src, &dest).expect("copy file");
        }
    }

    let main = dst.join("main.art");

    let mut cmd = Command::cargo_bin("art").expect("binary");
    cmd.arg("run").arg(main.to_str().unwrap());
    let out = cmd.output().expect("run art");
    assert!(
        out.status.success(),
        "art failed: stdout={} stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
