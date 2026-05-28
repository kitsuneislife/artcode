# Roadmap

## Entregue em v0.4 (lançado 2026-05-27)

### Linguagem
- Structs, enums, match com guards
- F-strings com format specs (`upper`, `lower`, `trim`, `hex`, `padN`, `debug`)
- Métodos em structs/enums via `func Tipo.metodo(self) {}` e blocos `impl Type { }`
- Closures, tuplas, destructuring
- `while` / `for` nativos
- `try/catch` explícito no parser e interpreter
- Pipeline operator `|>` e stream pipeline
- Módulos básicos (resolução local e git)
- Modo puro (`--pure`)

### Runtime
- Actor model com mailbox e scheduler round-robin
- Arena allocation (`arena_new`, `arena_with`, `arena_release`)
- `performant` blocks com escape analysis
- Weak/unowned references e detecção de ciclos (Tarjan SCC)
- Adaptive ARC
- Capability tokens com move-semantics
- Zero-copy IPC / serialização binária (`Buffer`, `serialize`, `deserialize`)

### Tooling
- `art build --target js` — transpila para JavaScript ES2022 com source maps V3
- `art build --bundle` — bundle autocontido para Node.js e browser
- Type checker — inferência local, verificação de anotações
- LSP completo — completion, goto-def, hover, rename, semantic tokens (`art lsp`)
- TTD shell — `art debug --replay` com `step`, `breakpoint`, `state-at`
- Fuzz targets, property tests, CI regression detector

---

## Entregue em v0.5 (em desenvolvimento)

### Linguagem — Bloco B: `component {}` e qualificadores de binding
- Keywords `component`, `view`, `state`, `prop`, `memo`, `ref` no lexer
- `Stmt::ComponentBlock`, `Stmt::QualifiedBinding` no AST
- Parser: `component Name { state x: T = expr; view { <template> } }`
- Type checker: 4 regras — prop imutável, memo stale, state fora de escopo, ref em view

### Compilação — Bloco D: ReactivityPass e grafo de dependências
- Novo crate `reactivity`: `DepGraph` com nós `Source`/`Derived`/`Sink`
- `ReactivityPass` analisa `ComponentBlock` e constrói DAG de dependências
- Tarjan SCC para detecção de ciclos em memos — erro de compile time
- Codegen JS: `set_X(v)` recomputa memos em ordem topológica; text nodes nomeados

### Runtime UI — Bloco E: Scheduler, Lifecycle e DOM
- `JS_RUNTIME` com scheduler assíncrono (`__schedule`/`__flush` via `queueMicrotask`)
- Lifecycle hooks: `on_mount`, `on_destroy`, `on_update`, `tick`
- Helpers DOM: `dom.create`, `dom.text`, `dom.append`, `dom.set_attr`, `dom.set_text`, etc.

### ArtKit v0.1 — Bloco F
- `examples/artkit/counter.art` — componente Counter funcional
- `examples/artkit/todo.art` — TodoItem e TodoList com state
- `docs/guides/artkit_quickstart.md` — guia passo a passo
- CI smoke test: compila counter.art e verifica bundle com Node.js

### Stdlib e Tooling — Bloco G
- `Deque<T>` no prelude: 6 funções + `ArtValue::Deque`
- TTD delta snapshots: `Tracer::record_delta` armazena diff entre keyframes
- DAP mínimo: `art debug --dap` emite eventos `initialized`/`stopped`/`terminated`
- `art doc <path>` gera HTML em `docs/generated/<name>.html`
- `docs/guides/migration_v0.4_to_v0.5.md`

---

## v0.6 — Objetivos de médio prazo

- Generics no interpreter — monomorphização básica na chamada de função
- Diagnósticos com linha/coluna precisa — erros de parse mostram posição exata
- TTD Fase 2 — debug shell interativo com checkpoints navegáveis
- JIT compilação funcional (LLVM integrado sem flags extras)
- Sistema de módulos além do MVP (cache, resolução em rede)

---

## Horizonte (sem prazo definido)

- AOT compilation completa e portável
- WASM como target de compilação
- Generics com monomorfização completa e bounds checking
- Type checker gradual com inferência inter-procedural

---

## Métricas de Qualidade

| Métrica | Estado atual |
|---------|------|
| `cargo test --all` | Verde — 0 falhas |
| `cargo clippy -- -D warnings` | Limpo — 0 warnings |
| `cargo build --release` | Limpo |
| Exemplos | 57 exemplos funcionais |
| CI jobs | build-and-test, metrics, node-smoke, artkit-smoke, perf-regression, coverage |

---

## Contribuição

Abrir RFC para features que alterem sintaxe, semântica ou runtime. Pequenos ajustes (refactors internos, bugfixes) podem ir direto com testes.

Consulte `docs/versioning.md` para a política de compatibilidade e `docs/guides/contributing.md` para o fluxo RFC → ADR → implementação.
