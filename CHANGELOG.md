# Changelog

Todas as mudancas relevantes deste projeto serao documentadas neste arquivo.
O formato segue [Keep a Changelog](https://keepachangelog.com) e SemVer.

## [Unreleased]

### Added
- **Codegen JavaScript (`crates/codegen_js`):** novo crate que transpila AST Artcode para
  JavaScript ES2022. Suporta `let`/`var`, funções, structs (→ `class`), enums (→ tagged union),
  `match` (→ if-else com bindings), `for`/`while`, closures, f-strings (→ template literals),
  `performant {}` (→ IIFE), `spawn actor {}` (→ Web Worker), `import` (→ ES modules).
- **`art build --target js`:** comando CLI para compilar `.art` para JavaScript.
  Flags: `--out <dir>` (padrão `dist/`), `--sourcemap` (source map V3 com VLQ encoding),
  `--bundle`. Flag `--target wasm` scaffolded para v0.5.
- **Source maps V3:** encoding VLQ completo; erros em runtime apontam para o `.art` original
  no DevTools do browser.
- **Stdlib de strings:** 8 novos builtins no prelude — `str_split`, `str_join`, `str_contains`,
  `str_starts_with`, `str_replace`, `str_slice`, `str_to_int`, `str_to_float`. Todos com
  diagnósticos de tipo corretos e 25 testes de cobertura.

## [0.3.0] - 2026-05-21

### Added
- **`impl Type { }` syntax:** blocos `impl` como açúcar para registrar métodos em tipos,
  equivalente ao `func Type.method(self)` existente. Retrocompatível.
- **Generics no runtime:** validação de constraints de tipo (`Numeric`, `Eq`, `Hash`,
  `Comparable`) na chamada de funções genéricas; inferência do tipo concreto pelos
  argumentos quando `type_args` não são explícitos.
- **`ArtValue::type_name()`:** reflexão de tipo em runtime para todos os variants de valor.
- Capability tokens com move-semantics: `ArtValue::Capability` / `ArtValue::MovedCapability`;
  builtins `capability_acquire`, `capability_kind`; enforcement no type checker e runtime.
- Time-Travel Debugging fase 1: tracer determinístico, flag `--record <arquivo>` no CLI,
  serialização zero-copy de fontes não-determinísticas (`time_now`, `rand_next`).
- Serialização nativa zero-copy: tipo `Buffer`, builtins `buffer_new`, `serialize`,
  `deserialize`. Serialização rejeita handles opacos (actors, mutexes, capabilities).
- APIs de arena: `arena_new`, `arena_with`, `arena_release`; métricas
  `arena_alloc_count`, `objects_finalized_per_arena`, `finalizer_promotions_per_arena`.
- Política pública de versionamento (`docs/guides/versioning.md`) e comando
  `art update --check` / `art update --self`.

### Changed
- **`interpreter.rs` reduzido de 6.734 → 1.715 linhas** por extração de sete módulos:
  `builtins`, `actors`, `cycle_detection`, `gc`, `exec`, `eval`.
- Diagnósticos de builtins agora reportam linha/coluna corretos: `call_span` propagado
  do field-access call site para todos os 81 handlers de builtin (antes sempre `0:0`).
- REPL suprime saída `[metrics]` e `[mem]` por linha; exibe `=> valor` sem ruído.
- Actor runtime com mailbox e builtins: `actor_send`, `actor_receive`,
  `actor_receive_envelope`, `actor_set_mailbox_limit`, `actor_yield`,
  `envelope`, `make_envelope`, `run_actors`.
- CLI reorganizado: `art lsp`, `art doc std`, `art format`, `art lint`, `art aot`,
  `art run --pure`.

### Fixed
- 6 `unwrap()` encadeados no actor scheduler substituídos por `expect()` descritivo.
- `aot.rs`: `.to_str().unwrap()` em path substituído por passagem direta de `OsStr`.
- `lsp.rs`: `serde_json::to_string().unwrap()` substituído por early-return seguro.
- `main.rs`: três pontos de `unwrap()` críticos corrigidos (debug repl, upgrade, copy).
- Seções duplicadas `### Changed` e `### Docs` no [Unreleased] consolidadas.
- Referência morta a `.kit/checklist.md` em `docs/overview.md` corrigida.
- Reforço no fluxo de actor scheduler para `parked` / `actor_receive` rerun.
- Escape analysis de `performant` e `bind_value_to_pattern` para evitar leaks de arena.
- Replay determinístico de `time_now` e `rand_next` no tracer/replayer.

### Docs
- `docs/` reorganizada em subpastas: `language/`, `internals/`, `guides/`, `rfcs/`.
- Números de RFC duplicados corrigidos (0002×2, 0003×2, 0005×2 → 0006, 0007, 0008).
- `docs/roadmap.md` reescrito refletindo o estado real de v0.2 e metas de v0.3/v0.4.
- `docs/notes.md` expandido com limitações conhecidas (JIT, LSP, generics, diagnostics).

## [0.2.0] - 2026-03-18

### Added
- Loop statements nativos (while/for), tuplas e destructuring.
- Blocos explicitos de try/catch no parser e interpretador.
- Modo de execucao puro via flag `--pure`.
- Builtin `dag_topo_sort` para ordenacao topologica de dependencias.
- Workflow de triagem automatica de issues com labels lang-design, runtime e tooling.
- Autodoc de stdlib via comando `art doc std`.
- Politica de versionamento publico com promessas de compatibilidade para 0.2.x.

### Changed
- GOVERNANCE.md formalizado com fluxo RFC -> ADR -> implementacao.
- CONTRIBUTING.md atualizado com processo RFC, ADR e triagem automatica.
- `docs/decisions` ganhou template ADR canonicamente referenciado.

### Fixed
- Ajustes de parser/lexer/runtime para loops, tuples e semantica de try/catch.
- Correcoes de compatibilidade do linter para mudancas recentes de AST.

### Docs
- Novos guias: loops_tuples, error_handling, pure_mode, dependency_dag e versioning.
- README, docs e website sincronizados com recursos entregues da trilha 0.2.

## Convencao de Atualizacao

- Atualizar [Unreleased] a cada PR mergeado.
- Em release, mover [Unreleased] para uma secao versionada datada.
- Classificar entradas em Added, Changed, Deprecated, Removed, Fixed e Docs.
