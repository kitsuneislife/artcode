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

## Métricas de ciclos no JSON

O comando de métricas agora exporta também sinais de GC/ciclos no JSON:

```bash
art metrics --json examples/00_hello.art
```

Campos adicionados para análise automática:

- `cycle_leaks_detected`
- `cycle_components_detected`
- `cycle_weak_dead_count`
- `cycle_unowned_dangling_count`
- `cycle_summary` com:
	- `weak_total`, `weak_alive`, `weak_dead`
	- `unowned_total`, `unowned_dangling`
	- `objects_finalized`, `heap_alive`
	- `avg_out_degree`, `avg_in_degree`
	- `candidate_owner_edges`
