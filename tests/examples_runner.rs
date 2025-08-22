#[test]
fn run_language_examples() {
    let status = match std::process::Command::new("bash").arg("scripts/test_examples.sh").status() {
        Ok(s) => s,
        Err(e) => panic!("failed to run examples script: {:?}", e),
    };
    assert!(status.success(), "examples script failed");
}
