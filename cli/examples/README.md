This folder contains runnable examples for the `art` CLI. Files are named with a two-digit prefix to
enforce ordering (00..99) and a descriptive suffix. Subdirectories (like `modules/`) may contain
package-style examples with their own `Art.toml`, `lib.art`, and `main.art`.

When adding examples, follow the existing pattern:
- `00_hello.art`
- `01_values_and_variables.art`
- `...`

Module/package examples live in `cli/examples/modules/<name>/` and should include `Art.toml`.
# Exemplos Artcode

Coleção progressiva de exemplos numerados demonstrando as capacidades atuais da linguagem.

Ordem sugerida:
00_hello.art – Hello World
01_values_and_variables.art – Tipos primitivos e variáveis
02_arrays_options.art – Arrays e option none (keyword: `none`)
03_control_flow.art – If, operadores lógicos, escopos
04_functions_closures.art – Funções e captura de ambiente
05_structs_enums_match.art – Struct, enum e match básico
06_enum_shorthand_inference.art – Inferência shorthand de enum
07_pattern_matching_guards.art – Pattern matching com guards
08_fstrings_format_specs.art – f-strings e format specs
09_methods_struct_enum.art – Métodos de struct e enum + introspecção
10_result_like_pattern_try.art – Propagação estilo try (?) em enums Result-like
11_arrays_builtin_methods.art – Métodos builtin de arrays
12_metrics_demo.art – Métricas de execução (stderr)

Para executar todos automaticamente veja script de teste em nível de workspace.
