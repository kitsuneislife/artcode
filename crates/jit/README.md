# `crates/jit`

Este crate contém o scaffold experimental de compilação JIT/AOT para Artcode, suportado por bindings LLVM (`inkwell`).

## Propósito
- Fornecer um ponto de partida para implementar compile-on-demand via LLVM.
- Manter a crate compilável sem a feature `jit` (como stub) para não forçar dependências pesadas de C++ (LLVM) a todos os contribuidores.

## Como Usar
Por padrão, o workspace artcode ignora a compilação real do JIT atuando como um fallback interpretado. Para ativar:

1. Instale o LLVM (16+) e o `clang` no seu ambiente.
2. Construa ou teste a crate ativando a flag:
```sh
cargo build -p jit --features=jit
cargo test -p jit --features=jit
```

## Arquitetura e CI
- A implementação realiza o lowering da Intermediate Representation (IR) do Artcode (`crates/ir`) para LLVM IR.
- No Github Actions, o job `ci-jit-smoke.yml` garante que as construções com a feature habilitada funcionem via contêineres Docker dedicados.
