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
 - [x] Docs: `docs/memory.md` (seção inicial adicionada e refinada — agora documenta centralização de mutações e o contrato dos helpers; exemplos/trade-offs podem ser expandidos futuramente)

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
	- [x] RFC: Runtime de Atores (mailbox FIFO, isolamento por mensagem) (see `docs/rfcs/0003-actors.md`)
	- [x] Implementar `spawn actor { ... }` sintaxe (ou função builtin temporária)
	- [x] Tipo de mensagem polimórfico (enum ValueEnvelope)
	- [x] Scheduler cooperativo inicial (round-robin / prioridade simples)
	- [x] Backpressure: limite configurável de mailbox + diagnóstico (MVP implemented: mailbox_limit + return bool; docs pending)
	- [x] Blocos `performant {}` parse + verificação de restrições (parser + runtime prototype present)
	- [x] Primitivas compartilhadas autorizadas em performant: Mutex, AtomicInt (prototype runtime implemented)
	- [x] Análise básica: proibir captura de valor não Send-safe (placeholder rule) (conservative checks in place; richer analysis pending) (see commit: de6033f)
	- [x] Docs: `docs/concurrency.md` com exemplos comparando estilos

Notas rápidas:
- Implementação: builtins `actor_send`, `actor_receive`, `actor_receive_envelope`, `actor_set_mailbox_limit`, `make_envelope` e scheduler round-robin implementados em `crates/interpreter/src/interpreter.rs`.
- Testes: `crates/interpreter/tests/actors_mvp.rs` e `crates/interpreter/tests/actors_stress.rs` exercitam FIFO, prioridade e backpressure (passando localmente).
- Pendências principais: primitives compartilhadas (`Mutex`, `AtomicInt`) e análise Send-safe para captures entre atores.
 - Pendências principais: análise Send-safe enriquecida (proibir envio/compartilhamento de valores não-Send-safe entre atores/threads); formalizar semântica multithreaded das primitivas (atualmente single-threaded prototype)

## Fase 10 – Pipeline JIT/AOT + PGO
- [x] RFC: Arquitetura IR interna (docs/rfcs/0004-ir-architecture.md) — finalizar e aprovar
	- owner: eng-compiler / eng-runtime
	- aceitação: RFC revisada com plano de rollout, exemplos IR e formato de profile; sinal verde em revisão técnica

- [x] Crates & API (infra mínima)
	- [x] `crates/ir`: tipos (Module/Function/Instr/Type) + emitter/parser textual
	- aceitação: `cargo test -p ir` verde; `Function::emit_text` produz formato documentado
	- [x] `crates/ir::lowering` public hook que recebe AST -> retorna `ir::Function`
	- aceitação: lowering das funções de exemplo em `cli/examples/` produz golden files estáveis
	- [x] `crates/jit` (scaffold, optional, feature = "jit")
		- aceitação: crate presente, compilável sem a feature `jit`; instruções para habilitar na doc
 - [x] `crates/ir`: tipos (Module/Function/Instr/Type) + emitter/parser textual
 - aceitação: `cargo test -p ir` verde; `Function::emit_text` produz formato documentado
 - [x] `crates/ir::lowering` public hook que recebe AST -> retorna `ir::Function`
 - aceitação: lowering das funções de exemplo em `cli/examples/` produz golden files estáveis
 - [x] `crates/jit` (scaffold, optional, feature = "jit")
	- aceitação: crate presente, compilável sem a feature `jit`; instruções para habilitar na doc

 - [x] Tooling: xtask / irgen
	- [x] `xtask gen-golden --update` / `--check`
		- aceitação: `xtask -- gen-golden --check` retorna não-zero quando há diffs; CI usa `--check`
	- [x] `xtask emit-ir <example>` que roda lowering e escreve IR em stdout/file

- [ ] CLI integration
	- [x] `art run --emit-ir` (ou `--emit-ir=out.ir`) — imprime IR textual por função
		- aceitação: comando executa em modo interpretado e grava IR para cada função sem alterar semântica

- [ ] Runtime instrumentation & profile format
	- [~] call counters por função (hotness) e borda (opcional)
	- [~] especificação simples de `profile.dat` (JSON v1: { functions: { name: count }, edges: [...] })
	- [x] `art run --gen-profile profile.dat` -> produz `profile.dat`
		- aceitação: arquivo gerado e pode ser lido pela fase AOT

- [ ] JIT prototype (feature `jit`, opt-in)
	- [~] lowering -> LLVM module builder (inkwell behind feature) (prototype in `crates/jit::llvm_builder`)
	- [~] compile-on-demand via ORC (ou MCJIT) e API `compile_function(fn) -> pointer` (scaffold present)
	- [ ] fallback stubs: código nativo valida com checks e cai para interpretador em caso de mismatch
	- aceitação: com LLVM instalado e `--features jit` é possível compilar+executar um microkernel (ex: sum i64 array)

- [ ] AOT pipeline (iterativo)
	- [~] `art build --with-profile profile.dat` que consome profile e aplica heurísticas (plan generation + artifact impl present)
	- heurísticas iniciais: inline hot functions, split cold code, reorder basic blocks (TODO)
	- aceitação: gera um artefato (JSON artifact present). Emissão de bitcode/binary é objetivo futuro.

- [ ] Tests, benchmarks & golden files
	- [~] golden tests para cada regra de lowering em `crates/ir/tests/` (in progress; added phi tests)
	- [ ] microbench harness: `bench/` com scripts para medir warmup e steady-state
	- [ ] comparação automática que gera `.kit/perf.md` com warmup vs PGO binary

- [ ] CI / dev ergonomics
	- [x] `xtask gen-golden --check` job (always run)
	- [ ] optional `jit-smoke` job: runs on LLVM-enabled runner or via docker image (opt-in matrix)
	- aceitação: default CI still green for contributors without LLVM; optional job validates JIT changes

- [ ] Docs & developer experience
	- [x] `docs/ir.md` with textual IR spec, examples, and `--emit-ir` semantics
	- [ ] `docs/rfcs/0004-ir-architecture.md` updated with decisions and owner links
	- [x] `docs/dev/llvm-docker.md` showing how to reproduce LLVM dev image

Note: the project contains `.github/workflows/xtask-irgen-check.yml` (runs on push/PR) and an optional `ci-jit-smoke.yml` workflow that can be triggered via PR label `jit-smoke` or adapted to use the Docker image documented in `docs/dev/llvm-docker.md`.

- [ ] Risk register & mitigations (short)
	- LLVM heavy: feature-gate `jit`; provide docker image and CI opt-in
	- correctness: interpreter remains canonical and golden tests gate JIT usage
	- contributor friction: keep default workflow interpreter-first; only advanced runners need LLVM

- Aceitação geral da Fase 10 (definition of done)
	- lowering pipeline passa todos os golden tests
	- `xtask gen-golden --check` integrado ao CI
	- runtime produz `profile.dat` via `art run --gen-profile`
	- com LLVM e `--features jit` é possível compilar e executar pelo menos 1 microbenchmark mais rápido que o interpretador (MVP)
	- documentação mínima presente (`docs/ir.md`, `docs/rfcs/0004-ir-architecture.md`, `docs/dev/llvm-docker.md`)

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
 - [~] PRIORIDADE: Fase 13 — Sistema de Módulos & Pacotes (focar implementação MVP)
 	 - [x] RFC: Sintaxe `import foo.bar` e resolução (docs/rfcs/0002-modules.md)
 	 - [x] Parser: reconhecer `import` e gerar AST
 	 - [x] Resolver local: mapear `import` para arquivo no workspace
 	 - [x] Manifesto de pacote `Art.toml` (nome, versão, deps, profile build) (parsing TOML MVP aplicado)
 	 - [x] Cache local: `~/.artcode/cache` e regra de resolução (lookup por nome/main)
 	 - [x] CLI: `art add <path-or-git>` instalar no cache + atualizar `.art-lock` (suporte file:// e git/local)
 	 - [x] Docs: `docs/modules.md` + exemplos em `cli/examples/modules/` (docs e RFC atualizados)

## Fase 14 – Governança & RFC Processual
- [~] Criar `docs/rfcs/0000-template.md`
- [~] Documento `GOVERNANCE.md` (papéis, fluxo de decisão)
- [~] Atualizar `contributing.md` ligando para processo RFC
- [~] Registro de decisões: `docs/decisions/` com changelog de design
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
