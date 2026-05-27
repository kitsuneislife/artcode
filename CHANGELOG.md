# Changelog

Todas as mudancas relevantes deste projeto serao documentadas neste arquivo.
O formato segue [Keep a Changelog](https://keepachangelog.com) e SemVer.

## [Unreleased]

### Added
- **Bloco E — Runtime UI: scheduler assíncrono, lifecycle hooks, dom helpers:**
  - `JS_RUNTIME` em `cli/src/bundler.rs` expandido com: scheduler assíncrono (`__schedule`/`__flush` via `queueMicrotask`), `tick(fn)`, registry de lifecycle (`on_mount`, `on_destroy`, `on_update`), helpers DOM (`dom.create`, `dom.text`, `dom.append`, `dom.set_attr`, `dom.set_text`, `dom.remove`, `dom.on`, `dom.off`, `dom.query`)
  - Codegen: `set_X(v)` usa `__schedule` para batching assíncrono de updates DOM
  - Codegen: `Name_create` expõe `Name_component` e chama `tick(() => __run_mount(...))` após DOM inserido
  - 4 testes em `codegen_js/tests/scheduler_lifecycle.rs`

- **Bloco D — ReactivityPass e grafo de dependências (`crates/reactivity/`):**
  - Novo crate `reactivity`: `DepGraph` com nós `Source`/`Derived`/`Sink`, arestas tipadas
  - `ReactivityPass::analyse` visita `Stmt::ComponentBlock` e constrói DAG de dependências
  - Tarjan SCC para detecção de ciclos em memos — emite diagnóstico `reactive cycle detected`
  - Codegen JS atualizado: `set_X(v)` recomputa memos em ordem topológica e atualiza apenas os text nodes afetados (text nodes nomeados `__txt_X_N` para cada referência reativa no template)
  - 4 testes: DAG acíclico válido (×2), ciclo entre memos detectado (×2)

- **Bloco B — `component {}` e qualificadores de binding:**
  - Lexer: 6 novos tokens — `component`, `view`, `state`, `prop`, `memo`, `ref`
  - AST: `Stmt::ComponentBlock { name, bindings, view }`, `Stmt::QualifiedBinding { qualifier, name, type_ann, value }`, `enum BindingQualifier { State, Prop, Memo, Ref }`
  - Parser: `component Name { state/prop/memo/ref x: T = expr; view { <template> } }` → `Stmt::ComponentBlock`
  - Type checker: 4 regras de diagnóstico — prop imutável, memo possivelmente stale, state fora de escopo, ref em view
  - Codegen JS: `emit_component` gera `function Name_create(host)` com criação DOM inicial
  - Interpreter: `ComponentBlock`/`QualifiedBinding` são no-ops em runtime (compile-time only)
  - 8 testes novos: 4 parser + 4 typeck

## [0.4.0] - 2026-05-27

### Added
- **Parser ArtML (`crates/parser`):** suporte a templates declarativos embutidos na linguagem.
  `<tag>`, `<tag attr="val">`, `<tag attr={expr}>`, `<tag on:event={handler}>`, `<Tag />` (componente),
  `<if cond={expr}>...</if><else>...</else>`, `<for item in {items} key={expr}>...</for>`,
  `<slot name="x">...</slot>`. Nenhum novo token no lexer — `<` em posição prefix é
  inequivocamente início de template no Pratt parser. 12 testes de parser + 10 testes de codegen.
- **Codegen de templates (`crates/codegen_js`):** `Expr::Template` → IIFE com chamadas DOM
  (`document.createElement`, `setAttribute`, `addEventListener`, `createTextNode`,
  `createDocumentFragment`). Componentes PascalCase → `new Component({...})`.
- **AST (`crates/core`):** `enum TemplateNode` (`Element`, `Component`, `Text`, `Expr`, `If`,
  `For`, `Slot`), `struct TemplateAttr`, `enum TemplateAttrValue`, `Expr::Template`.
- **Diagnósticos de template em parse time:** tag sem fechamento → `error` com span exato;
  `<for>` sem atributo `key` → `warning`.

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
- **TTD Fase 2 — Debug Shell interativo:** `art debug --replay` agora expõe shell completo
  com `step`, `step-back`, `state` (variáveis com valores, filtrando builtins), `state-at <tick>`
  (salta para tick específico via fast-forward), `mailbox` (inspeciona mailboxes e status de
  atores), `breakpoint <line>` (pausa na linha), `continue` (executa até próximo breakpoint),
  `clear` (limpa breakpoints) e `quit`. Breakpoints são persistentes entre restarts do replay.
  6 testes de cobertura.
- **Type checker (`crates/typeck`):** novo crate de verificação estática em compile time.
  Inferência local (`let x = 42` → `x: Int`), verificação de anotações de funções
  (`func f(x: Int)` rejeita argumento `String` na chamada) e inferência paramétrica
  (`func identity<T>(x: T)` infere `T = Int` na chamada com inteiro). Integrado ao pipeline
  `art build --target js` como pré-passo antes do codegen; 12 testes de cobertura.
- **`art build --bundle`: bundler completo para JS de arquivo único.**
  `cli/src/bundler.rs` resolve imports recursivamente, compila cada módulo com
  `ModuleFormat::Bundle` (que suprime `import` stmts no codegen), deduplica módulos
  visitados e concatena: runtime preamble → módulos em ordem topológica → entry file.
  O runtime preamble (`JS_RUNTIME`) define os builtins Artcode em JS nativo
  (`println = console.log`, `str_*`, `len`, `none`, etc.), tornando o bundle
  autocontido para Node.js e browser. 3 testes de integração. Job `node-smoke-test`
  no CI verifica `art build --bundle examples/00_hello.art` com Node.js.
- **Diagnósticos de template ArtML:**
  - C.1: `on:click={expr}` onde `expr` tem tipo não-callable (Int, Float, String, Bool)
    → `lint error: event handler 'on:click' has type 'Int', which is not callable`.
    Verificado no type checker (`typeck`) ao visitar `TemplateAttrValue::EventHandler`.
    2 testes: handler não-callable emite aviso; handler function não emite.
  - C.2: Componente PascalCase usado em template sem import ou struct correspondente
    → `parse error: Component '<Counter>' is used but not imported or defined`.
    Verificado em post-parse pass no parser (`check_component_imports`).
    2 testes: componente sem definição emite erro; com struct definida não emite.
- **Robustez — fuzzing, property tests e regression detector:**
  - Novo fuzz target `interpreter_valid` — executa o interpreter apenas em programas sem erros de parse, complementando o `parser_loops` existente.
  - 9 property tests de round-trip `parse(format(src)) == parse(src)` em `cli/tests/formatter_roundtrip.rs`, incluindo idempotência do formatter.
  - 2 stress tests de arena: N atores com `performant {}` e 100 iterações de alloc/finalize (`cli/tests/actor_performant_stress.rs`).
  - `scripts/perf_regression.sh` mede `executed_statements/s` de fib(20) com baseline em `baseline/perf_fib20.json`; integrado no CI como job `perf-regression` — falha se a taxa cair abaixo de 50k stmt/s ou se a contagem de statements mudar.
  - `cli` expõe crate library (`lib.rs`) para testes de integração terem acesso direto ao `formatter`.
- **LSP — completion, goto-def, hover, rename, semantic tokens:** `art lsp` agora expõe
  `textDocument/completion` (builtins + variáveis + structs/enums com kind correto),
  `textDocument/definition` (goto-def para `let`, funções, structs, enums, imports),
  `textDocument/rename` (rename em todos os arquivos do workspace), hover com assinaturas
  de função reais e campos de struct/enum, `textDocument/semanticTokens/full` (highlight
  semântico). Arquitetura refatorada com `process_request` testável; 6 smoke tests de
  harness JSON. Guias de configuração adicionados em `docs/guides/lsp_vscode.md` e
  `docs/guides/lsp_neovim.md`.

### Deferred to v0.5
- Qualificadores `state`/`prop`/`memo`/`ref` no lexer e parser — depende de `component {}`.
- Regras de diagnóstico para `prop` mutada e `memo` com dependência ausente — depende de `component {}`.
- TTD delta snapshots e integração DAP mínima — fora de escopo v0.4.

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
