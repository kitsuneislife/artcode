use std::path::PathBuf;
use std::process::Command;

/// Teste de integração que executa o binário `art` apenas para o exemplo 99.
/// Mantém paridade com `scripts/test_examples.sh` mas isolado para CI rápido.
#[test]
fn run_example_99() {
    // localizar workspace root via caminho relativo
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf();
    let bin = root.join("target/debug/art");
    // build se necessário
    if !bin.exists() {
        let st = Command::new("cargo")
            .arg("build")
            .status()
            .expect("cargo build falhou");
        assert!(st.success(), "cargo build falhou");
    }
    let example = root.join("cli/examples/99_weak_unowned_demo.art");
    let status = Command::new(bin)
        .arg("run")
        .arg(example)
        .status()
        .expect("falha ao executar o binário art");
    assert!(status.success(), "execução do exemplo 99 falhou");
}
