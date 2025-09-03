use std::path::PathBuf;
use std::process::Command;

/// Teste de integração que executa o binário `art` apenas para o exemplo 99.
/// Mantém paridade com `scripts/test_examples.sh` mas isolado para CI rápido.
#[test]
fn run_example_99() {
    // localizar workspace root via caminho relativo
    let root = match PathBuf::from(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2) {
        Some(p) => p.to_path_buf(),
        None => panic!("failed to run integration_example_99 setup"),
    };
    let bin = root.join("target/debug/art");
    // build se necessário
    if !bin.exists() {
        let st = match Command::new("cargo").arg("build").status() {
            Ok(s) => s,
            Err(e) => panic!("cargo build falhou: {:?}", e),
        };
        assert!(st.success(), "cargo build falhou");
    }
    let example = root.join("cli/examples/99_weak_unowned_demo.art");
    let status = match Command::new(bin).arg("run").arg(example).status() {
        Ok(s) => s,
        Err(e) => panic!("falha ao executar o binario art: {:?}", e),
    };
    assert!(status.success(), "execução do exemplo 99 falhou");
}
