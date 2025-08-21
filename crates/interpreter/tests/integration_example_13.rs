use std::path::PathBuf;
use std::process::Command;

/// Teste de integração que executa o binário `art` apenas para o exemplo 13.
#[test]
fn run_example_13() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf();
    let bin = root.join("target/debug/art");
    if !bin.exists() {
        let st = Command::new("cargo")
            .arg("build")
            .status()
            .expect("cargo build falhou");
        assert!(st.success(), "cargo build falhou");
    }
    let example = root.join("cli/examples/13_weak_cycle_demo.art");
    let status = Command::new(bin)
        .arg("run")
        .arg(example)
        .status()
        .expect("falha ao executar o binário art");
    assert!(status.success(), "execução do exemplo 13 falhou");
}
