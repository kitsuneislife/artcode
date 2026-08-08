# Roadmap v0.5 — Artcode → ArtKit v0.1

Objetivo central: fechar 100% dos pré-requisitos do documento `artcode_prereqs_artkit.pdf` —
concluir os blocos B (qualificadores de binding), D (ReactivityPass) e E (runtime UI) —,
garantir integridade de tudo entregue até v0.4, e entregar o primeiro componente ArtKit
funcionando no browser sem virtual DOM.

Estado: v0.5.1 — todos os pré-requisitos ArtKit 100% cumpridos.

---

## Progresso geral  [██████████]  55 / 55  ✓ COMPLETO

---

## Q — QA Sweep: Integridade de v0.3 e v0.4  [██████████]  10 / 10  ✓ COMPLETO

Auditoria completa antes de qualquer nova feature. Nenhuma regressão tolerada.

- [x] `cargo test --all` verde, sem skips, em modo debug e release
- [x] `cargo clippy -- -D warnings` zero warnings em todos os crates do workspace
- [x] `cargo build --release` limpo sem warnings
- [x] Todos os exemplos em `examples/` compilando e executando sem regressão (`art run`)
- [x] Bundle smoke test: `art build --target js --bundle examples/00_hello.art` + `node dist/00_hello.js` retorna "Hello, Artcode!"
- [x] 6 smoke tests LSP passando (harness JSON)
- [x] 9 property tests de round-trip formatter passando
- [x] Fuzz targets `parser_loops` e `interpreter_valid` rodando sem crashes
- [x] `scripts/perf_regression.sh` passando com baseline atual (≥ 50k stmt/s, stmts = 76621 — resultado: ~1.16M stmt/s)
- [x] Consistência documental: versões em todos os Cargo.toml, CHANGELOG, README e website alinhadas

---

## B — component {} + Qualificadores de Binding  [██████████]  14 / 14  ✓ COMPLETO

Fecha o Bloco B do `artcode_prereqs_artkit.pdf` (CRÍTICO). Desbloqueador de D e F.

**Lexer — novos tokens**
- [x] Keywords `component` e `view` adicionadas ao lexer (`crates/lexer/src/lexer.rs`)
- [x] Keywords `state`, `prop`, `memo`, `ref` adicionadas ao lexer como qualificadores de binding

**AST — novos nós em `crates/core/src/ast.rs`**
- [x] `Stmt::ComponentBlock { name: String, bindings: Vec<Stmt>, view: Vec<TemplateNode> }`
- [x] `enum BindingQualifier { State, Prop, Memo, Ref }` e `Stmt::QualifiedBinding { qualifier, name, type_ann, value }`

**Parser — novas regras**
- [x] `component Name { ... }` → `Stmt::ComponentBlock` (`crates/parser/src/parser.rs`)
- [x] `state x: Type = expr` / `prop x: Type` / `memo x: Type = expr` / `ref x = expr` dentro do bloco
- [x] `view { <template> }` — sub-bloco ArtML dentro de component, popula `ComponentBlock::view`

**Type checker — 4 regras de diagnóstico (`crates/typeck/src/lib.rs`)**
- [x] `prop` mutada dentro do componente → `error: prop 'x' is immutable in this scope`
- [x] `memo` referenciando state/prop ausente nas deps → `warning: memo 'y' may be stale — 'count' not in dep list`
- [x] `state` lido fora de bloco `component {}` → `error: 'state' only valid inside component scope`
- [x] `ref` usada como valor reativo em `view {}` → `warning: ref values are not reactive; use state instead`

**Type checker — B.4 — type annotation enforcement**
- [x] `state x: Int = "hello"` → `type error: 'x' declared as Int, initializer has type String`
- [x] `memo m: Int = "text"` → `type error` (mesma regra para todos os qualificadores)

**Testes**
- [x] 8 testes em `crates/typeck/` e `crates/parser/`: violação + caso válido por cada uma das 4 regras
- [x] 4 testes B.4: mismatch em state, mismatch em memo, match válido, prop sem initializer

---

## D — ReactivityPass + Grafo de Dependências  [██████████]  12 / 12  ✓ COMPLETO

Fecha o Bloco D do `artcode_prereqs_artkit.pdf` (ALTO). Depende de B.

**Crate `crates/reactivity/`**
- [x] Estrutura: `ReactivityPass` visitor sobre `Stmt::ComponentBlock`, retorna `DepGraph`
- [x] `DepGraph`: nós tipados `Source` (state, prop), `Derived` (memo), `Sink` (view TemplateNode)

**Construção do DAG**
- [x] Aresta `Source → Derived`: memo que lê state ou prop
- [x] Aresta `Source → Sink`: nó de template que referencia state ou prop diretamente
- [x] Aresta `Derived → Sink`: nó de template que lê memo
- [x] Aresta `Derived → Derived`: memo que depende de outro memo

**Detecção de ciclos**
- [x] Tarjan SCC sobre o `DepGraph` — ciclo em dependências de memo = erro de compilação
- [x] Diagnóstico claro: `error: reactive cycle detected — 'A' depends on 'B' which depends on 'A'`

**Codegen — updaters cirúrgicos (`crates/codegen_js/`)**
- [x] `Stmt::ComponentBlock` → `function Name_create(host)` com criação DOM inicial
- [x] Para cada `state X`: gerar `set_X(v)` que recomputa memos afetados em ordem topológica e atualiza só os sinks do DAG (sem re-render do componente inteiro)
- [x] `Name_create` retorna `{ set_X, ... }` — todos os setters de state expostos para composição pai/filho

**Testes**
- [x] 4 testes: ciclo detectado em memo circular (×2) + DAG acíclico compila corretamente (×2)
- [x] 2 testes D.3: return com setter único + return com múltiplos setters

---

## E — Runtime UI: Scheduler, Lifecycle e DOM  [██████████]  8 / 8  ✓ COMPLETO

Fecha o Bloco E restante do `artcode_prereqs_artkit.pdf` (MÉDIO). Strings já entregues em v0.4.

**Scheduler assíncrono no `JS_RUNTIME` (`cli/src/bundler.rs`)**
- [x] `const __pending = new Set(); let __scheduled = false;` + `function __schedule(updater)` + `function __flush()` via `queueMicrotask` no preamble
- [x] Codegen: `set_X(v)` chama `__schedule(updater_X)` — DOM nunca é atualizado síncrono

**Lifecycle hooks no `JS_RUNTIME`**
- [x] `on_mount(component, fn)` — executa após inserção no DOM; injetado pelo codegen em `Name_create`
- [x] `on_destroy(component, fn)` — executa antes de remoção; deregistra event listeners
- [x] `on_update(component, fn, deps)` — executa quando qualquer dep muda
- [x] `tick(fn)` — nextTick via `queueMicrotask(fn)`

**Módulo `dom` no codegen**
- [x] Bindings `dom.create`, `dom.text`, `dom.append`, `dom.set_attr`, `dom.set_text`, `dom.remove`, `dom.on`, `dom.off`, `dom.query` mapeados para chamadas DOM diretas no codegen JS

**Testes**
- [x] 4 testes em `codegen_js/tests/scheduler_lifecycle.rs`: `set_X` usa `__schedule`, memo recomputado dentro do closure, `tick`/`__run_mount` injetados, `__run_update` chamado

---

## F — ArtKit v0.1: Primeiro Componente Real  [██████████]  6 / 6  ✓ COMPLETO

Critério de saída de v0.5. Depende de B + D + E.

- [x] `examples/artkit/counter.art` — `component Counter` com `state count: Int = 0`, `view { <div><p>{count}</p><button on:click={increment}>+</button></div> }`
- [x] `art build --bundle examples/artkit/counter.art` gera bundle autocontido sem erros, sem panics
- [x] Bundle executado com Node.js sem erros de runtime; DOM atualiza cirurgicamente via `set_count` + `__schedule`
- [x] `examples/artkit/todo.art` — `TodoItem` com `prop label: String` + `TodoList` com `state items`
- [x] `docs/guides/artkit_quickstart.md` — hello world ArtKit passo a passo: instalar, criar componente, compilar, rodar no browser
- [x] Job CI `artkit-smoke` em `.github/workflows/ci.yml`: compila counter.art, verifica bundle com Node.js, confirma ausência de erros de runtime

---

## G — Pendências e Completude  [██████████]  6 / 6  ✓ COMPLETO

Itens abertos do checklist operacional independentes dos blocos acima.

**TTD**
- [x] Delta snapshots: armazenar diff entre keyframes — reduz tamanho do trace ~10×
- [x] Integração DAP mínima: `art debug` reporta posição atual ao editor via protocolo DAP

**Stdlib**
- [x] `Deque<T>` no prelude: `deque_new`, `deque_push_front`, `deque_push_back`, `deque_pop_front`, `deque_pop_back`, `deque_len` com 6 testes

**Tooling**
- [x] `art doc <path>` gerando HTML a partir de docstrings no source — output em `docs/generated/`

**Release**
- [x] `docs/guides/migration_v0.4_to_v0.5.md` — breaking changes, novos tokens de keyword, guia de migração
- [x] Pacote binário no GitHub Releases (Linux x86_64, macOS arm64) + `install.sh` testado e documentado

---

## Ordem de implementação

```
Q (QA Sweep) ──► B (component + quals) ──► D (ReactivityPass) ──► E (Runtime UI) ──► F (ArtKit v0.1)
                         │
                         └──► G (paralelo — independente)
```

**Status:**
```
Q (QA Sweep)          — 10/10  ✓ COMPLETO
B (component + quals) — 12/12  ✓ COMPLETO
D (ReactivityPass)    — 11/11  ✓ COMPLETO
E (Runtime UI)        —  8/8   ✓ COMPLETO
F (ArtKit v0.1)       —  6/6   ✓ COMPLETO
G (Pendências)        —  6/6   ✓ COMPLETO
```

---

## Cobertura dos prereqs após v0.5

```
Bloco A — Compilador JS / Bundler     ✓  (v0.4)
Bloco B — Tipos + qualificadores      ✓  (v0.5: B)
Bloco C — Parser ArtML                ✓  (v0.4)
Bloco D — ReactivityPass              ✓  (v0.5: D)
Bloco E — Runtime UI                  ✓  (v0.5: E — strings em v0.4)
```

---

## Critérios de saída para v0.5

- [x] `cargo test --all` verde; `cargo clippy -- -D warnings` limpo; `cargo build --release` limpo
- [x] Qualificadores `state` / `prop` / `memo` / `ref` e bloco `component {}` reconhecidos, verificados com diagnósticos corretos
- [x] ReactivityPass: ciclos em deps de memo detectados em compile time com mensagem clara
- [x] Codegen: `set_X(v)` atualiza só os sinks afetados (updaters cirúrgicos, sem re-render global)
- [x] Scheduler `__schedule` / `__flush` via `queueMicrotask` no JS_RUNTIME; lifecycle hooks `on_mount` / `on_destroy` funcionando
- [x] `art build --bundle examples/artkit/counter.art` roda no browser com contador interativo
- [x] Todos os exemplos de v0.3 e v0.4 funcionando sem regressão
- [x] `artcode_prereqs_artkit.pdf` 100% coberto — blocos A ✓ B ✓ C ✓ D ✓ E ✓
