This folder contains runnable examples for the `art` CLI. Files are named with a two-digit prefix to
enforce ordering (00..99) and a descriptive suffix. Subdirectories (like `modules/`) may contain
package-style examples with their own `Art.toml`, `lib.art`, and `main.art`.
This folder contains runnable examples for the `art` CLI. Files are named with a two-digit prefix to
enforce ordering (00..99) and a descriptive suffix. Package-style examples live under `modules/`.

Current examples (ordered):

- `00_hello.art` — Hello World
- `01_values_and_variables.art` — Tipos primitivos e variáveis
- `02_arrays_options.art` — Arrays e option none (keyword: `none`)
- `03_control_flow.art` — If, operadores lógicos, escopos
- `04_functions_closures.art` — Funções e captura de ambiente
- `05_structs_enums_match.art` — Struct, enum e match básico
- `06_enum_shorthand_inference.art` — Inferência shorthand de enum
- `07_pattern_matching_guards.art` — Pattern matching com guards
- `08_fstrings_format_specs.art` — f-strings e format specs
- `09_methods_struct_enum.art` — Métodos de struct e enum + introspecção
- `10_result_like_pattern_try.art` — Propagação estilo try (?) em enums Result-like
- `11_arrays_builtin_methods.art` — Métodos builtin de arrays
- `12_metrics_demo.art` — Métricas de execução (stderr)
- `13_weak_cycle_demo.art` — Demonstração de ciclos fracos
- `14_cycle_stub.art` — Stub de ciclo
- `15_finalizer_examples.art` — Exemplos de finalizers
- `16_weak_unowned_demo.art` — Weak / Unowned demos
- `17_metrics_arena_demo.art` — Arena/metrics demo
- `18_performant_bad.art` — Exemplos que violam restrições `performant`

Module/package examples live in `cli/examples/modules/<name>/` and should include `Art.toml` and a `main.art` entrypoint.

To run an example:

```sh
art run cli/examples/00_hello.art
```

To run a package example:

```sh
cd cli/examples/modules/demo && art run main.art
```

When adding or renaming examples, follow the two-digit prefix pattern so listings stay ordered.
