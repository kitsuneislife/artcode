# Roadmap v0.4 — Artcode → ArtKit

Objetivo central: transformar o Artcode de interpretador em plataforma capaz de compilar
para o browser, habilitando o desenvolvimento do ArtKit (framework de UI declarativo nativo).

Estado: v0.3.0 lançado. Desenvolvimento em curso.

---

## Progresso geral  [██████████]  71 / 72

---

## A — Codegen JavaScript  [██████████]  13 / 13  ✓

Crate `crates/codegen_js/` implementado. `art build --target js` funcional.

- [x] Estrutura do crate: visitor sobre a AST emitindo JS ES2022
- [x] `let`/`var` → `const`/`let`
- [x] `func f(a, b) {}` → `function f(a, b) {}`; closures capturadas naturalmente
- [x] `struct Foo { x, y }` → `class Foo { constructor(x,y){} }` com prototype methods
- [x] `enum Status { Ok(v), Err(e) }` → tagged union `{ tag, payload }`
- [x] `match` → if-else encadeado com bindings; guards suportados
- [x] `spawn actor {}` → `new Worker()` com blob URL
- [x] `performant {}` → IIFE (semântica de arena aproximada)
- [x] Comando `art build <file.art> --target js --out dist/`
- [x] Source maps V3: VLQ encoding completo, span `.art` → posição no `.js`
- [x] ES Modules: `import foo.bar` → `import * as foo from "./foo/bar.js"`
- [x] Flag `--bundle`: runtime JS preamble + inlining recursivo de imports + deduplicação; `ModuleFormat::Bundle` no codegen suprime `import` stmts; 3 testes
- [x] Smoke test executado com Node.js no CI — job `node-smoke-test` em `ci.yml`

---

## B — Sistema de Tipos Estático  [███████░░░]  7 / 10

Crate `crates/typeck/` — verificação em compile time para state/prop/memo e generics.

- [x] `type_params` no AST + generics com constraints em runtime (`Numeric`, `Eq`, `Hash`, `Comparable`) — v0.3
- [x] `ArtValue::type_name()` — reflexão de tipo para todos os valores — v0.3
- [x] Crate `crates/typeck/` com visitor de tipo sobre a AST — integrado ao pipeline `art build --target js`
- [x] Inferência local: `let x = 42` → `x: Int` rastreado no escopo sem anotação
- [x] Verificação de funções anotadas: `func f(x: Int) -> String` rejeitado se tipos não batem em compile time
- [ ] Qualificadores `state`, `prop`, `memo`, `ref` no lexer e parser (novos tokens) — depende de Bloco C
- [ ] Regra: `prop` mutada dentro de componente → `error: prop 'x' is immutable in this scope` — depende de Bloco C
- [ ] Regra: `memo` com dependência ausente → `warning: memo 'y' may be stale — 'count' not in dep list` — depende de Bloco C
- [ ] Regra: `state` usado fora de bloco `component {}` → `error: 'state' only valid inside component scope` — depende de Bloco C
- [x] Inferência de tipo paramétrico: `func render<T>(items: Array<T>)` chamado com `[1,2,3]` infere `T = Int`

---

## C — Parser ArtML  [██████████]  17 / 17  ✓

Extensão do parser existente. Sem novos tokens no lexer — `<` em posição prefix é
inequivocamente início de template (Pratt parser). `If`/`For`/`Else` reusados como tags.

**Lexer — novos tokens:** (todos resolvidos via disambiguation no parser, sem tokens adicionais)
- [x] `TagOpen`/`TagClose`/`SelfClose`: `<` (`Less`) em prefix position + `/` + `>`
- [x] `AttrName`: `Identifier` token
- [x] `AttrValue`: `String` literal ou `{expr}` com `LeftBrace`
- [x] `ControlTag`: `<if>`, `<for>`, `<else>` via tokens `If`/`For`/`Else` existentes

**AST — novo nó:**
- [x] `enum TemplateNode` em `crates/core/src/ast.rs`: `Element`, `Component`, `Text`, `Expr`, `If`, `For`, `Slot`
- [x] `Expr::Template(Vec<TemplateNode>)` na AST

**Parser — novas regras:**
- [x] Elemento simples: `<div class="c"><h1>{title}</h1></div>`
- [x] Atributo dinâmico: `<input value={count} disabled={count == 0} />`
- [x] Event binding: `<button on:click={handler}>texto</button>`
- [x] Componente filho (PascalCase): `<Counter label="Total" initial={0} />`
- [x] Condicional: `<if cond={logged_in}>...</if><else>...</else>`
- [x] Lista: `<for item in {items} key={item.id}><li>{item.name}</li></for>`
- [x] Slot: `<slot name="header"><h2>Default</h2></slot>`

**Diagnósticos em parse time:**
- [x] Tag sem fechamento → `error` com span exato
- [x] `on:click` com expressão não-callable → `warning` via type checker (`lint error: not callable`)
- [x] Componente usado sem importar → `error` com post-parse pass no parser
- [x] `<for>` sem `key` → `warning: lista sem key pode causar re-renders incorretos`

**Codegen:**
- [x] `TemplateNode` → JavaScript DOM calls via `codegen_js` — IIFE retornando DOM node/fragment

---

## D — Stdlib: Operações de String  [██████████]  8 / 8  ✓

Todas implementadas e testadas (25 testes passando).

- [x] `str_split(s: String, sep: String) -> Array<String>`
- [x] `str_join(arr: Array<String>, sep: String) -> String`
- [x] `str_contains(s: String, sub: String) -> Bool`
- [x] `str_starts_with(s: String, prefix: String) -> Bool`
- [x] `str_replace(s: String, from: String, to: String) -> String`
- [x] `str_slice(s: String, start: Int, end: Int) -> String` (suporta índices negativos)
- [x] `str_to_int(s: String) -> Result<Int, String>`
- [x] `str_to_float(s: String) -> Result<Float, String>`

---

## E — TTD Fase 2: Debug Shell  [████████░░]  8 / 10

- [x] Record/replay determinístico com event sourcing (`--record`, `--replay`) — v0.3
- [x] Keyframes/checkpoints no trace — v0.3
- [x] `art debug --replay trace.artlog` abre shell interativo REPL
- [x] Comando `step` / `step-back` — avança/recua um evento
- [x] Comando `state` — imprime variáveis do escopo atual no tick (filtrando builtins)
- [x] Comando `state-at <tick>` — salta diretamente para tick específico
- [x] Comando `mailbox` — inspeciona estado do mailbox e status dos atores
- [x] Comando `breakpoint <line>` — pausa no evento gerado por aquela linha
- [ ] Delta snapshots: armazenar diff entre keyframes (reduz tamanho do trace ~10×)
- [ ] Integração DAP mínima: `art debug` reporta posição atual ao editor via protocolo DAP

---

## F — LSP: Completion e Goto-Def  [██████████]  8 / 8  ✓

- [x] Diagnósticos (lex/parse/type) publicados via LSP — v0.2
- [x] Hover básico — v0.2
- [x] `textDocument/completion`: variáveis em escopo, métodos de struct/enum, builtins
- [x] `textDocument/definition`: goto-def para funções, structs, enums e campos
- [x] `textDocument/rename`: renomear símbolo em todos os arquivos do workspace
- [x] `textDocument/semanticTokens`: highlight semântico (tipos, funções, variáveis)
- [x] Smoke test via harness JSON (request/response LSP sem IDE) — 6 testes cobrindo initialize, completion, hover, definition, rename, shutdown
- [x] Configs de exemplo para VS Code e Neovim em `docs/guides/`

---

## G — Robustez  [██████████]  6 / 6  ✓

- [x] cargo-fuzz harness para parser (`fuzz/cargo_fuzz/fuzz_targets/parser_loops.rs`) — v0.3
- [x] CI de fuzzing 60s por push — v0.3
- [x] Harness de fuzzing para o interpreter (`interpreter_valid.rs`) — skip parse-error programs, only runs interpreter on parseable input
- [x] Property test: `parse(format(parse(src))) == parse(src)` para o formatter — 9 testes em `cli/tests/formatter_roundtrip.rs` incluindo idempotência
- [x] Stress test: N atores em `performant {}` + 100 iterações de arena alloc/finalize — `cli/tests/actor_performant_stress.rs`
- [x] Detector de regressão no CI: `scripts/perf_regression.sh` mede `executed_statements/s` de fib(20), falha se abaixo de 50k stmt/s ou se contagem mudar; integrado em `.github/workflows/ci.yml`

---

## Fora de escopo em v0.4

- Bloco D (ReactivityPass / grafo de dependências) → v0.5
- Bloco E (runtime DOM, scheduler, lifecycle hooks) → v0.5
- ArtKit v0.1 (primeiro componente real no browser) → v0.5
- `art build --target wasm` (scaffolding em v0.4, implementação real v0.5)
- Registry público de pacotes → v0.5+

---

## Ordem de implementação

```
A (Codegen JS) ✓──► C (ArtML Parser) ✓──► D+E no v0.5
        │
        └──► B (Types) ──► C (valida atributos com tipos) ✓
                   │
                   └──► F (LSP usa tipos para completion)

A (Codegen JS)     ✓ — 13/13 concluído
B (Tipos estáticos)  — 7/10 (3 items bloqueados por component{} → v0.5)
C (Parser ArtML)   ✓ — 17/17 concluído
D (Stdlib strings) ✓ — 8/8 concluído
E (TTD Fase 2)       — 8/10 (2 items opcionais: delta snapshots, DAP)
F (LSP)            ✓ — 8/8 concluído
G (Robustez)       ✓ — 6/6 concluído
```

**Status:** v0.4 está em 71/72. O único item restante (B: state/prop/memo/ref qualifiers) está bloqueado no concept de `component {}` que é escopo de v0.5. v0.4 pode ser considerado feature-complete.

---

## Critérios de saída para v0.4

- [x] `art build --target js examples/00_hello.art` gera JS executável
- [ ] Diagnóstico de `prop` mutada e `memo` com dep ausente funcionando — BLOQUEADO: depende de `component {}` → v0.5
- [x] `<div>{x}</div>` no parser gera `TemplateNode::Element` sem panics
- [x] Todos os 8 `str_*` builtins no prelude com testes (25 testes)
- [x] `art debug --replay` com `step`, `step-back`, `state`, `state-at`, `mailbox`, `breakpoint` funcionando
- [x] LSP com completion e goto-def passando smoke tests
- [x] Property tests de round-trip parse→format→parse no CI
- [x] `cargo test --all` verde; zero novos panics em `clippy -- -D warnings`
- [x] Todos os exemplos em `examples/` compilando sem regressão
