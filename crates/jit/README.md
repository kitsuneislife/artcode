README for crates/jit

Habilitando e testando o JIT (resumo)

- O crate `crates/jit` é opcional e feature-gated. Para compilar com suporte a LLVM:
  - adicione `--features=jit` nos comandos `cargo build`/`cargo test`/`cargo run`.

- Smoke test:
  - Há um teste de smoke em `crates/jit/tests/jit_smoke.rs` que é compilado apenas quando a feature `jit` está ativa.
  - O teste está marcado `#[ignore]` por padrão; execute manualmente com:
    - `cargo test -p jit --features=jit -- --ignored`

- Executando em um ambiente com LLVM (Docker):
  - Um Dockerfile preparado está em `ci/docker/llvm/Dockerfile` e pode ser usado para produzir uma imagem com LLVM dev libs compatíveis.
  - Dentro da imagem, rode:
    - `cargo test -p jit --features=jit -- --ignored`

- Observações:
  - Mantemos o JIT por feature para não forçar dependências de LLVM em todos os contribuidores.
  - O código do builder LLVM está em `crates/jit/src/llvm_builder.rs` (protótipo). Se você for estender, siga o padrão de fallback para builds sem `jit`.

Referência CI (opt-in):

- Há um workflow opt-in `ci-jit-smoke.yml` que constrói uma imagem Docker com LLVM (`ci/docker/llvm/Dockerfile`) e executa o smoke test.
- No PR, adicione o label `jit-smoke` para disparar o job; também é possível acionar manualmente via "Run workflow".

Bench local:

- Script mínimo: `bench/run_jit_micro.sh` — prepara a build com `--features=jit` e tenta executar um micro exemplo.

Se precisar de ajuda para ajustar a imagem Docker (versões do LLVM / inkwell), abra uma issue com detalhes do host OS e a saída dos comandos `rustc --version` e `ldd $(which clang)`.
# JIT (scaffold)

Esta crate é um scaffold mínimo para a futura implementação do JIT (baseada em LLVM).
Ela está feature-gated: a feature `jit` ativa dependências e código que requerem LLVM
e `inkwell`.

Como usar localmente:

- Para compilar sem LLVM (comportamento padrão):

```text
cargo test -p jit
```

- Para compilar com a feature (requer LLVM dev libs instaladas):

```text
cargo test -p jit --features=jit
```

Planejamento futuro:

- Implementar lowering de IR -> LLVM IR usando `inkwell`.
- Integrar com ORC/MCJIT para compilação on-demand.
- Prover mecanismos seguros de fallback para o interpretador.

Por enquanto a crate expõe tipos de conveniência (`JitBuilder`) e stubs de API
que retornam erros informativos quando a feature não está habilitada.

# Crate `crates/jit` (scaffold)

Este crate contém o scaffold para um JIT experimental usando `inkwell` (bindings LLVM).

Propósito
- Fornecer um ponto de partida para implementar compile-on-demand via LLVM.
- Manter a crate compilável sem a feature `jit` para não forçar dependências pesadas aos contribuidores.

Como usar
- Por padrão a crate compila como stub. Para ativar o JIT é necessário habilitar a feature `jit` e ter LLVM/clang instalados no sistema.

Exemplo de build (com feature):

cargo build -p jit --features=jit

Notas
- A implementação completa do JIT (lowering -> LLVM Module -> ORC) ainda está pendente.
- Veja `docs/rfcs/0004-ir-architecture.md` e `docs/ir.md` para o design da IR e integração com runtime.
JIT scaffold crate
===================

This crate is a small scaffold for future JIT work. The real implementation should live
behind the `jit` feature and depend on `inkwell` (LLVM bindings). The scaffold ensures
the workspace builds for contributors that don't have LLVM installed.

Usage
-----

To enable the real JIT implementation (future):

1. Install LLVM and the required development headers on your system.
2. Add `inkwell` to the crate with the `jit` feature and implement `compile_function`.
3. Build with `cargo build -p jit --features jit`.
