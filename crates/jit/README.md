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

This crate is a small scaffold for future JIT work. The real implementation lives
behind the `jit` feature and should depend on `inkwell` (LLVM bindings). The scaffold
keeps the workspace buildable for contributors without LLVM.

Quick notes
-----------

- To build and run tests without LLVM (default):

	cargo test -p jit

- To build with the JIT feature (requires LLVM dev libs):

	cargo test -p jit --features=jit

- Current status: the `jit` feature path includes a placeholder `compile_function` that
	returns `JitError::Other("NotImplemented")`. This is intentional to allow development
	without a full LLVM toolchain.

Contributing
------------

If you implement the real JIT, keep the default stub behavior behind `not(feature = "jit")`
so contributors without LLVM are not blocked.
