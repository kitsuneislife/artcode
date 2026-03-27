This folder contains runnable examples for the `art` CLI. Files are named with a two-digit prefix to
enforce ordering (00..99) and a descriptive suffix. Subdirectories (like `modules/`) may contain
package-style examples with their own `Art.toml`, `lib.art`, and `main.art`.
This folder contains runnable examples for the `art` CLI. Files are named with a two-digit prefix to
enforce ordering (00..99) and a descriptive suffix. Package-style examples live under `modules/`.

Current examples (ordered):

- `00_hello.art` — Hello World
- `01_values_and_variables.art` — Tipos primitivos e variáveis
- `02_arrays_options.art` — Arrays e option none
- `03_control_flow.art` — If, operadores lógicos, escopos
- `04_functions_closures.art` — Funções e captura de ambiente
- `05_structs_enums_match.art` — Struct, enum e match básico
- `06_enum_shorthand_inference.art` — Inferência shorthand de enum
- `07_pattern_matching_guards.art` — Pattern matching com guards
- `08_fstrings_format_specs.art` — f-strings e format specs
- `09_methods_struct_enum.art` — Métodos de struct e enum + introspecção
- `10_result_like_pattern_try.art` — Propagação estilo try (?)
- `11_arrays_builtin_methods.art` — Métodos builtin de arrays
- `12_metrics_demo.art` — Métricas de execução (stderr)
- `13_weak_cycle_demo.art` — Demonstração de ciclos fracos
- `14_cycle_stub.art` — Stub de ciclo (usado no linter de dependências)
- `15_finalizer_examples.art` — Finalizers (métodos especiais executados no GC)
- `16_weak_unowned_demo.art` — Weak/Unowned com açúcar sintático e validação semântica no lint
- `17_metrics_arena_demo.art` — Profiling com a Arena (métricas de alocamento de vida curta)
- `18_stdlib_features.art` — Standard Library embutida: I/O files, Time, Map e Set nativos
- `19_performant_arena.art` — Blocos experimentais "performant" de GC Bypass
- `20_actors_simple.art` — Atores em concorrência: spawn, envelopes e messaging assíncrono
- `21_microkernel.art` — Estrutura de microkernel baseada em Message Passing (Atores)
- `22_fmt_test.art` — Teste de parsing e auto-formatação (`art fmt`)
- `23_linter_tests.art` — Teste de shadowing, arm morto e hint de hotspot de alocação (`art lint`)
- `24_result_option_sugar.art` — Uso avançado das Mônadas Nativas (Result/Option)
- `25_loops_tuples_destructuring.art` — Loops `while`/`for` com tuplas e destructuring
- `26_try_catch.art` — Tratamento explícito de erro com `try/catch`
- `27_pure_mode.art` — Execução com `--pure` bloqueando operações impuras
- `28_dependency_dag.art` — Ordenação topológica de dependências com `dag_topo_sort`
- `29_versioning_policy.art` — Metadados de release e promessas de compatibilidade pública
- `30_changelog_workflow.art` — Estrutura semântica de changelog para releases
- `31_upgrade_migration.art` — Guia rápido para checagem de migração com `art upgrade`
- `32_fuzz_ci.art` — Worker de fuzzing contínuo para parser/loops no CI
- `33_actor_http_runtime.art` — Request/response HTTP básico via atores
- `34_closure_callbacks_arc.art` — Closures retornadas e callbacks com captura de ambiente
- `35_shell_syntax.art` — Sintaxe shell com statement `$`, pipeline `|>` e retorno tipado em `shell_result`
- `36_pipeline_operator.art` — Operador de pipeline de expressões (`valor |> fn(...)`)
- `37_stream_pipeline.art` — Pipeline lazy com `stream/map/filter/collect/count` sem arrays intermediários entre etapas
- `38_custom_iterators.art` — Iteradores customizados com protocolo `next()` e açúcar `yield`
- `39_shell_function_call.art` — Execução shell em chamada de função (`echo("...")`) com retorno `Result`
- `40_reusable_arena.art` — APIs de arena reutilizável (`arena_new`, `arena_with`, `arena_release`)
- `41_idl_ipc.art` — IDL de IPC via `struct` com `idl_schema` e `idl_validate`
- `42_capability_tokens.art` — Capabilities move-only (`capability_acquire`, `capability_kind`) com consumo em runtime
- `43_ipc_serialization.art` — Serialização binária (`serialize`, `deserialize`, `buffer_new`) e restrições para tipos opacos
- `44_ttd_keyframes.art` — Time-travel com keyframes/checkpoints (`--record`/`--replay`)
- `45_release_changelog.art` — Highlights de release e estrutura semântica de changelog
- `46_perf_compare_workflow.art` — Rotina de comparação warmup vs PGO com geração de `perf.md`

Module/package examples live in `examples/modules/<name>/` and should include `Art.toml` and a `main.art` entrypoint.

To run an example:

```sh
art run examples/00_hello.art
```

To run a package example:

```sh
cd examples/modules/demo && art run main.art
```

When adding or renaming examples, follow the two-digit prefix pattern so listings stay ordered.
