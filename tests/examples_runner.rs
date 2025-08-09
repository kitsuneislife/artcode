#[test]
fn run_language_examples() {
    let status = std::process::Command::new("bash")
        .arg("scripts/test_examples.sh")
        .status()
        .expect("failed to run examples script");
    assert!(status.success(), "examples script failed");
}
