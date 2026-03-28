# Comparacao de Performance: Warmup vs PGO

Este fluxo executa a suite de benchmarks (`bench/cases/*.art`) em duas condicoes:

1. **Warmup baseline**: build release padrao.
2. **PGO**: build instrumentado + merge de perfil + build otimizado com `profile-use`.

Ao final, um relatorio Markdown e gerado em `artifacts/perf.md` com:

- tabela por benchmark
- totais agregados
- delta percentual entre warmup e PGO

## Comando

```bash
bash scripts/perf_compare.sh
```

Opcionalmente, voce pode escolher o arquivo de saida:

```bash
bash scripts/perf_compare.sh artifacts/perf.md
```

## Requisitos

- `python3`
- toolchain Rust com `llvm-profdata` disponivel
  - recomendado: `rustup component add llvm-tools-preview`

## Observacoes

- O script usa dados temporarios para PGO em `/tmp` e faz cleanup automatico.
- O binario baseline e salvo como `target/release/art-warmup`.
- O binario otimizado e salvo como `target/release/art-pgo`.
