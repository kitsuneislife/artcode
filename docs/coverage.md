## Cobertura de Código

Uso rápido com cargo-llvm-cov (requer LLVM):

Instalar:
```
cargo install cargo-llvm-cov
```

Relatório HTML:
```
cargo llvm-cov --workspace --html
```

Resumo terminal:
```
cargo llvm-cov --workspace
```

Limpar dados:
```
cargo llvm-cov clean
```

Próximos passos:
- Integrar em script/xtask.
- Threshold mínimo em CI.
