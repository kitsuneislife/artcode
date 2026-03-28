# Changelog

Todas as mudancas relevantes deste projeto serao documentadas neste arquivo.

O formato segue Keep a Changelog e SemVer adaptado para a trilha 0.2.x.

## [Unreleased]

### Added
- Politica publica de versionamento em docs/versioning.md.
- Exemplo 29 sobre metadados de release e compatibilidade (examples/29_versioning_policy.art).
- Comando de atualização no CLI: `art update --check` (consulta release mais recente com cache local) e `art update --self` (executa instalador oficial para autoatualização assistida).
- Aviso automático de nova release em terminal interativo (desativável com `ART_DISABLE_UPDATE_CHECK=1`).
- **Time-Travel Debugging (Fase 1: Tracer e Formato):** A infraestrutura base de trace determinístico por Event Sourcing foi estabelecida (RFC 0002). Adicionada flag no CLI `--record <arquivo>` permitindo salvar o log binário de fontes não-determinísticas nativas como `time_now` e `rand_next` para arquivo usando serialização zero-copy IPC. A API estendida para Time-Travel e log estruturado é o primeiro passo para a ferramenta de debug robusta que suporta IPC avançado. Detalhes documentados em `docs/debugging.md`.
- **Native Serialization (Zero-copy IPC)**:
  - Adicionado suporte a serialização binária recursiva focada em zero-copy no runtime e serializador binário DFS.
  - Adicionado o tipo de dado `Buffer` e builtins: `buffer_new`, `serialize`, `deserialize`.
  - Serialização rejeita e avisa em compilação handles em heap puramente opacos e vinculados à memória, como (Actors, Mutexes, Custom References e Capabilities).
- **Capability Tokens com Move-Semantics:** `ArtValue::Capability` e `ArtValue::MovedCapability` no AST; builtins `capability_acquire(kind)` e `capability_kind(cap)` na stdlib; enforcement de reuso (uma capability não pode ser usada após ser movida) tanto no type checker (TypeInfer) quanto no runtime (Environment::read_for_eval); suporte no parser para o tipo `Capability[Kind]`; docs no `art doc std`; exemplo `examples/42_capability_tokens.art` e suite de testes `tests/capability_tokens.rs`.

### Changed
- Adoção de actor runtime com mailbox e builtins: `actor_send`, `actor_receive`, `actor_receive_envelope`, `actor_set_mailbox_limit`, `actor_yield`, `envelope`, `make_envelope` e `run_actors`. Testes de actor HTTP e IPC (exs. `examples/33_actor_http_runtime.art`, `cli/tests/actor_http_runtime.rs`).
- Integração e hardening de builtin `http_get_text` com parsing de URL e corpo HTTP no runtime.
- Refinamentos de planejamento de chamadas FFI e C-ABI: `art_extern!`, `art_handle_*`, e `art_syscall_unsafe`.
- Implementado esquema de requisições de JIT/AOT com `irgen`, `jit`, e rotina `perf_compare` (scripts/perf_compare.sh, docs/perf_compare.md).
- Novas APIs de arena: `arena_new`, `arena_with`, `arena_release`, e métricas `arena_alloc_count`, `objects_finalized_per_arena`, `finalizer_promotions_per_arena`.
- Refatoração de CLI com `art lsp`, `art doc std`, `art format`, `art lint`, `art aot`, e `art run --pure`.
- Resolução de complexidade de parse e `lexer` com suporte completo a `while`, `for`, `yield`, `if let`, tuplas, destructuring, `|>` stream pipeline.

### Fixed
- Reforço no fluxo de actor scheduler para `parked` e `actor_receive` rerun sem dropping incorreto de variáveis.
- Correção de `performant` escape analysis (return/let arena object diagnostics) e `bind_value_to_pattern` para evitar leaks de arena.
- Correção de `arena_with` e `call_function` para não gerar arena implícita dupla no contexto arena_reuse.
- Ajustes no tracer/replayer de `time_now` e `rand_next` para replay determinístico em checkpoints.

### Docs
- Atualizada a página de changelog (`website/changelog.html`), navegação e exemplos 44/45/46.
- `docs/memory.md`, `docs/debugging.md`, `docs/contributing.md`, `docs/ffi.md`, `docs/enums.md` e docs do website sincronizados.
- `README.md` e `docs/overview.md` explicam a política de versionamento e upgrade (`art upgrade`).


### Changed
- Documentacao de contribuicao e roadmap atualizada para referenciar versao e compatibilidade da serie 0.2.x.

### Docs
- Estrutura de governanca, RFC/ADR e versionamento sincronizada entre docs, README e website.

## [0.2.0] - 2026-03-18

### Added
- Loop statements nativos (while/for), tuplas e destructuring.
- Blocos explicitos de try/catch no parser e interpretador.
- Modo de execucao puro via run flag --pure.
- Builtin dag_topo_sort para ordenacao topologica de dependencias.
- Workflow de triagem automatica de issues com labels lang-design, runtime e tooling.
- Autodoc de stdlib via comando art doc std.
- Politica de versionamento publico com promessas de compatibilidade para 0.2.x.

### Changed
- GOVERNANCE.md formalizado com fluxo RFC -> ADR -> implementacao.
- CONTRIBUTING.md atualizado com processo RFC, ADR e triagem automatica.
- docs/decisions ganhou template ADR canonicamente referenciado.

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

## Geracao Semiautomatica

Use scripts/changelog_from_git.sh para obter um rascunho por categorias semanticas a partir do git log.
Revise manualmente antes de publicar release.
