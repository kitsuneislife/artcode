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
