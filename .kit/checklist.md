# Artcode Roadmap Operacional (Pós Fase 7)

Guia incremental para evoluir do protótipo atual até os pilares do manifesto v2. Mantém filosofia de Complexidade Progressiva: cada fase entrega valor isolado e prepara a seguinte. Marque [x] quando concluído; adicione links para PRs e RFCs ao lado. Evite adicionar comentários nos códigos.
Não precisa comittar esse arquivo.

## Convenção de Marcação
- [ ] Item planejado | [~] Em progresso | [x] Concluído | [!] Risco/Bloqueio
- Itens com (RFC) exigem proposta formal antes de implementação ampla.

## Fase 8 – Modelo de Memória Avançado
 - [x] RFC: Design de referências weak/unowned (semântica, queda para none, proibições) (docs/rfcs/0001-weak-unowned.md)
 - [x] Implementar tipos `Weak<T>` e `Unowned<T>` no runtime (protótipo completo: builtins, açúcar sintático, validação alive; finalização invalida wrappers) (see commit: 76705f2)
 - [x] Adaptar `Arc` interno para contadores separados (strong/weak) (espelho `heap_objects` criado; comportamento parcial: decrements/finalizers instrumentados, resta trabalho em alguns caminhos de saída)
	 - 2025-08-23: implementação consistente: centralizei mutações de strong/weak em helpers e movi as mutações diretas para `crates/interpreter/src/heap_utils.rs` (`inc_strong_obj`, `dec_strong_obj`, `inc_weak_obj`, `dec_weak_obj`, `force_strong_to_one_obj`). Atualizei chamadores para usar os helpers, mantive a atualização de métricas no `Interpreter` e documentei o contrato dos helpers em `docs/memory.md`. Suite do `crates/interpreter` e `cargo test --all` rodaram verdes após as mudanças.
 - [x] Ferramenta de detecção de ciclos (agora baseada em heap ids, Tarjan SCC, reachability, ranking de sugestões)
 - [x] Relatório de ciclos: sugestões iniciais + reachability (leak_candidate) + ranking simples
 - [x] Arena API (escopo lexical) protótipo em blocos `performant` (AST + runtime support implemented; see interpreter tests)
 - [x] Priority 2: arena finalization hardening and per-arena metrics implemented (commit refs: d5ae06e, 190fd67, 0ae8e1b)
 - [x] Análise estática mínima: impedir escape de referências de arena (checagem conservadora implementada — `return` e capturas/lets compostos sinalizados; see `crates/interpreter/src/type_infer.rs`)
 - [x] Métricas: `cycle_leaks_detected` present; strong_increments/decrements partially instrumented; weak/unowned counters present. Implemented & exported: `arena_alloc_count`, `finalizer_promotions_per_arena`, `objects_finalized_per_arena` (see commits d5ae06e, 0ae8e1b, be27791)
 - [~] Docs: `docs/memory.md` (seção inicial adicionada; precisa refinamento e exemplos de métricas/trade-offs)

- [x] Docs: `docs/memory.md` (seção inicial adicionada e refinada — agora documenta centralização de mutações e o contrato dos helpers; exemplos/trade-offs podem ser expandidos futuramente)
 - [x] Infra interna: closures usam Weak + retained_env para evitar ciclos
 - [x] Finalizar caminho de decremento forte automático (escopos, rebind, coleta recursiva) e invalidar weak/unowned (NOVA) — cobertura de testes adicionada e runtime ajustado; testes do crate `interpreter` verdes localmente (2025-08-21)
 - [x] (2025-08-22) Marca: invalidação de weak/unowned em finalização implementada; testes adicionados e commit criado (main: 76705f2)

**Registro rápido (commits relevantes):**

- d5ae06e — interpreter: track objects_finalized_per_arena; cli: expose objects_finalized_per_arena in metrics; add test
- 190fd67 — interpreter: harden finalize_arena (deterministic order, multi-pass sweep)
- 0ae8e1b — cli: export arena_alloc_count and per-arena promotions in metrics JSON and compact output
- 76705f2 — feat(interpreter): mark weak/unowned wrappers as dangling on finalization; add tests
- be27791 — cli: add integration test for metrics JSON; add dev-deps; CI workflow for metrics validation

**Registro de commits / progresso rápido (2025-08-21):**

- 2025-08-21: Priority 1 (finalizar caminho de decremento forte automático) implementado e testes do crate `interpreter` rodando verdes; commit será criado com essas mudanças.
	- commit: 9d6c271
	- note: mudanças aplicadas localmente, pronta revisão/PR.

**Notas recentes (2025-08-21):**
- Runtime: finalizers, arena prototype and two-phase finalization implemented (objects can be registered in arena, `finalize_arena` performs decrements, sweep and invariant hardening).
- Métricas & tooling: added `finalizer_promotions` metric, CLI `art metrics --json`, `scripts/run_metrics.sh` and a CI job that uploads `artifacts/metrics.json` as an artifact.
- Testes: exposed deterministic debug helpers and updated interpreter tests to enable invariant checks; crate tests run green locally.
- Outstanding: complete automatic strong-decrement coverage across all scope-exit paths, finalize arena/collection edge-cases, and expand metrics for arena allocations.

**Progresso (2025-08-16):** Implementado comportamento adicional e protótipos relevantes para Fase 8:
- Parser: reconhece `performant { ... }` e produz `Stmt::Performant`.
- AST/Interpreter: `Stmt::Performant` tratado em runtime; arena_id atribuído a alocações dentro do bloco; `finalize_arena` libera objetos da arena ao sair do bloco.
- Environment / Heap: `Environment::define` evita double-decrement ao rebind; `heap_objects` ganhou `arena_id` e `objects_finalized` é contabilizado.
- Segurança: análise estática conservadora (TypeInfer) sinaliza `return` dentro de `performant`, funções definidas no bloco e `let` com inicializadores compostos; runtime checa e (em debug) panica ao detectar escape de objetos de arena.
- Testes: novos testes cobrindo rebind/finalizer e comportamento básico de arena; suíte do crate `interpreter` e `parser` estão verdes.

## Fase 9 – Concorrência Híbrida
- [ ] RFC: Runtime de Atores (mailbox FIFO, isolamento por mensagem)
- [ ] Implementar `spawn actor { ... }` sintaxe (ou função builtin temporária)
- [ ] Tipo de mensagem polimórfico (enum ValueEnvelope)
- [ ] Scheduler cooperativo inicial (round-robin / prioridade simples)
- [ ] Backpressure: limite configurável de mailbox + diagnóstico
- [ ] Blocos `performant {}` parse + verificação de restrições
- [ ] Primitivas compartilhadas autorizadas em performant: Mutex, AtomicInt
- [ ] Análise básica: proibir captura de valor não Send-safe (placeholder rule)
- [ ] Docs: `docs/concurrency.md` com exemplos comparando estilos

## Fase 10 – Pipeline JIT/AOT + PGO
- [ ] RFC: Arquitetura IR interna (lowering AST -> IR -> LLVM)
- [ ] Gerar IR textual para inspeção (`--emit-ir`)
- [ ] JIT inicial (LLVM ORC / alternativa simples) executando funções isoladas
- [ ] Instrumentação de perfil: contadores de borda e frequência de chamadas
- [ ] `art run --gen-profile profile.dat`
- [ ] AOT compiler: `art build --release` lendo `profile.dat`
- [ ] Aplicar heurísticas: inline hot, desinline cold, reorder de blocos
- [ ] Métrica: tempo de warmup vs. binário PGO (registrar em `.kit/perf.md`)

## Fase 11 – Interoperabilidade (FFI)
- [ ] RFC: Convenções ABI (naming, alinhamento, ownership crossing)
- [ ] Camada C: export/import de funções simples (string, i64, f64)
- [ ] Zero-cost Rust: macro `art_extern!{}` gerando ponte segura
- [ ] Conversão automática Arc<str> <-> *const c_char (com cache)
- [ ] WASM PoC: compilar função Artcode para WASM (funções puras numéricas)
- [ ] Exemplos: `examples/ffi/` demonstrando C e Rust
- [ ] Docs: `docs/ffi.md` expandido (tabelas de mapeamento de tipos)

## Fase 12 – Time-Travel Debugging
- [ ] RFC: Formato de traço (event log compactado)
- [ ] Modo record: `art run --record trace.artlog`
- [ ] Modo replay determinístico: `art debug --replay trace.artlog`
- [ ] Comandos interativos: step-back, state-at <tick>, inspect mailbox
- [ ] Captura de seeds de RNG e relógio lógico
- [ ] Compactação incremental de estados grandes (delta snapshots)
- [ ] Docs: `docs/debugging.md`

## Fase 13 – Sistema de Módulos & Pacotes
- [ ] RFC: Sintaxe `import foo.bar` e resolução
- [ ] Manifesto de pacote `Art.toml` (nome, versão, deps, profile build)
- [ ] Resolver com cache local (`~/.artcode/cache`)
- [ ] Namespaces: separar prelude mínimo vs. imports explícitos
- [ ] CLI: `art add <pkg>` baixa e fixa versão (sem publicar ainda)
- [ ] Docs: `docs/modules.md`

## Fase 14 – Governança & RFC Processual
- [ ] Criar `docs/rfcs/0000-template.md`
- [ ] Documento `GOVERNANCE.md` (papéis, fluxo de decisão)
- [ ] Atualizar `contributing.md` ligando para processo RFC
- [ ] Registro de decisões: `docs/decisions/` com changelog de design
- [ ] Issue labeler (futuro CI) para categorias: lang-design / runtime / tooling

## Fase 15 – Expansão da Stdlib
- [ ] Coleções: Map (hash), Set, Deque
- [ ] Math util (abs, pow, clamp)
- [ ] Tempo (instante monotônico) – só para debug determinístico
- [ ] IO básico abstraído (file, read_text, write_text) sandboxed
- [ ] Random (seed configurável)
- [ ] Docs geradas automaticamente de builtins (`art doc std`)

## Fase 16 – Tooling de Produtividade
- [ ] Formatter idempotente (`art fmt`) – regras mínimas
- [ ] Linter inicial (shadowing suspeito, unused variable, dead match arm)
- [ ] Categorias de diagnósticos (lex/parse/type/runtime/concurrency/memory)
- [ ] Geração de docs HTML: `art doc <path>`
- [ ] Skeleton Language Server (hover + go-to-definition protótipo)

## Fase 17 – Performance & Benchmarks
- [ ] Suites micro (arith, match, method dispatch)
- [ ] Suites macro (interpretação exemplos grandes)
- [ ] Script PGO automatizado (gera perfil + build + report)
- [ ] Capture baseline contínuo em `.kit/perf_history.csv`
- [ ] Regressão detector: threshold % falha CI (futuro)

## Fase 18 – Evolução do Sistema de Tipos
- [ ] RFC: Generics em funções (T param) com monomorfização
- [ ] Constraints simples (ex: T: Numeric)
- [ ] Açúcar para Result/Option (`if let`, `unwrap_or`, pipeline?)
- [ ] Erros de tipo com sugestões (did-you-mean)
- [ ] Preparar terreno para traits/interfaces (adiar implementação completa)

## Fase 19 – Segurança & Robustez
- [ ] Fuzzing (cargo-fuzz harness para parser & evaluator)
- [ ] Property tests (ex: parse->print->parse igual)
- [ ] Auditoria de panics internos (zerar novos panics não test)
- [ ] Stress test de memória (ciclos massivos + arenas)
- [ ] Teste determinístico de concorrência (sequência de entregas gerada)

## Fase 20 – Preparação de Release Público (0.1.0)
- [ ] Definir política de versionamento (semver adaptado)
- [ ] CHANGELOG.md inicial
- [ ] Licença revisada + cabeçalhos padrão nos arquivos
- [ ] Site simples / landing README reforçado (features > getting started)
- [ ] Guia de migração (pré 0.1 -> 0.1 se houver breaking)

## Backlog Cross-Cutting (Triagem Contínua)
- [ ] Melhorar mensagens de erro de pattern matching (highlight do subpattern)
- [ ] Otimização de alocação de strings (interning ampliado)
- [ ] Parallel compilation (dependências de módulos, futuro)
- [ ] Cache incremental para IR/JIT
- [ ] Métricas de GC de ciclos (ferramenta) integradas no relatório geral

## Riscos & Mitigações
- Weak/unowned sem análise -> vazios/segfault: mitigar com runtime checks opcionais em debug.
- Scheduler de atores injusto em cargas pesadas: adotar filas MPSC + work stealing futura.
- PGO coleta perfis irreais: documentar cenários representativos + script de validação.
- Explosão de monomorfização com generics: política inicial de limites + LRU de instâncias.
- Time-travel trace grande: compressão + sampling configurável.

## Métricas a Monitorizar
- crash_free_ratio
- handled_errors_total
- alloc_count / arena_alloc_count
- actor_mailbox_avg / max
- jit_hot_functions_inlined
- pgo_profile_coverage%
- cycle_leaks_detected (modo teste)

---
Atualize continuamente; abra RFC antes de mudanças estruturais profundas.
