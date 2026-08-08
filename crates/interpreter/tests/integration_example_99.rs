mod common;

/// Teste de integração que executa o binário `art` apenas para este exemplo.
/// A suíte completa roda no job `examples` do CI (`xtask run-examples`); este
/// teste mantém o caso de weak/unowned coberto por `cargo test`, sem depender
/// daquele job.
#[test]
fn run_example_99() {
    common::run_example("16_weak_unowned_demo.art");
}
