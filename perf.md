# Performance Comparison (Warmup vs PGO)

Generated at: 2026-03-26T21:56:17.025049


| Benchmark | Warmup (ms) | PGO (ms) | Delta |
|---|---:|---:|---:|
| `arithmetic.art` | 34 | 26 | -23.53% (faster) |
| `fibonacci.art` | 33 | 27 | -18.18% (faster) |
| `match.art` | 23 | 27 | +17.39% (slower) |
| `method_dispatch.art` | 24 | 29 | +20.83% (slower) |
| `tree_alloc.art` | 31 | 22 | -29.03% (faster) |

## Totals

- Warmup total: **145 ms**
- PGO total: **131 ms**
- Delta: **-9.66%** (faster)

## Reproduce

```bash
bash scripts/perf_compare.sh
```
