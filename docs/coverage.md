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

## Fuzzing contínuo no CI

O projeto mantém um worker dedicado de fuzzing para parser/loops em:

- `.github/workflows/fuzz-ci.yml`
- `fuzzing/fuzz_targets/parser_loops.rs`

Execução local (janela curta):

```bash
bash scripts/run_fuzz_ci.sh 60
```

Objetivo: detectar regressões de robustez (panic/crash) em entradas adversariais antes de merge.
