use std::path::PathBuf;
use std::process::Command;

/// Teste de integração que executa o binário `art` apenas para o exemplo 13.
#[test]
fn run_example_13() {
    let root = match PathBuf::from(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2) {
        Some(p) => p.to_path_buf(),
        None => panic!("integration_example_13 setup failed"),
    };
    let bin = root.join("target/debug/art");
    if !bin.exists() {
        let st = match Command::new("cargo").arg("build").status() {
            Ok(s) => s,
            Err(e) => panic!("cargo build falhou: {:?}", e),
        };
        assert!(st.success(), "cargo build falhou");
    }
    let example = root.join("cli/examples/13_weak_cycle_demo.art");
    let status = match Command::new(bin).arg("run").arg(example).status() {
        Ok(s) => s,
        Err(e) => panic!("falha ao executar o binario art: {:?}", e),
    };
    assert!(status.success(), "execução do exemplo 13 falhou");
}
