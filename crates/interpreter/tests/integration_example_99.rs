mod common;

/// Teste de integração que executa o binário `art` apenas para o exemplo 99.
/// Mantém paridade com `scripts/test_examples.sh` mas isolado para CI rápido.
#[test]
fn run_example_99() {
    common::run_example("16_weak_unowned_demo.art");
}
