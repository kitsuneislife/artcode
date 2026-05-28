![Banner](/banner.png)
<p align="center">
	<img alt="CI" src="https://github.com/kitsuneislife/artcode/actions/workflows/ci.yml/badge.svg" />
	<a href="docs/internals/coverage.md"><img alt="Coverage" src="https://img.shields.io/badge/Coverage-docs-blue.svg" /></a>
	<img alt="License" src="https://img.shields.io/badge/license-MIT-blue.svg" />
	<a href="https://github.com/kitsuneislife/artcode/issues"><img alt="issues - artcode" src="https://img.shields.io/github/issues/kitsuneislife/artcode" /></a>
	<a href="https://github.com/kitsuneislife/artcode"><img alt="stars - artcode" src="https://img.shields.io/github/stars/kitsuneislife/artcode?style=social" /></a>
</p>

Implementação experimental de uma linguagem interpretada em Rust — **v0.4.0** (2026-05-27) · v0.5 em desenvolvimento.

# Complexidade Progressiva

Artcode é projetada com um princípio central: **comece simples, escale quando precisar**.
ARC implícito no nível padrão; weak/unowned e arenas explícitas quando você precisa de controle fino;
generics com constraints, actors, capabilities e componentes reativos para problemas avançados.

## Features da linguagem

- Structs, Enums (variantes com payload) e pattern matching com guards
- Loops nativos (`while`, `for`), tuplas e destructuring (`let (a, b) = value`)
- Funções, closures com captura léxica e métodos com auto-binding de `self`
- `impl Type { }` blocks — syntax para agrupar métodos por tipo
- Generics com constraints — `func foo<T: Numeric>(x: T)` valida tipos em runtime
- **Templates ArtML** — `<div class="x">`, `<input value={expr} />`, `<button on:click={fn}>`, `<if cond={x}>`, `<for item in {items} key={id}>`
- **Componentes reativos** (v0.5) — `component Name { state x; prop y; memo z; view { <template> } }`
- f-Strings com format specs (`upper`, `lower`, `trim`, `hex`, `padN`, `debug`)
- Error handling explícito: `try/catch` + operador `?` + enums `Result`/`Option`
- Modo `--pure` para execução sem I/O e sem não-determinismo
- Sintaxe shell: `$ comando args` e chamada estilo função (`echo("...")`)
- Operador `|>` para pipeline de expressões
- Streams lazy: `stream |> map |> filter |> collect` sem arrays intermediários

## Memória e runtime

- **Implicit Adaptive ARC** — escopo-automático com promoção para heap global
- `weak` / `unowned` explícitos com validação e detecção de ciclos
- `performant { }` — arenas temporárias para hot paths
- APIs de arena: `arena_new`, `arena_with`, `arena_release`
- Actors com mailbox, backpressure e agendamento cooperativo round-robin
- Capability tokens move-only (`capability_acquire`, `capability_kind`)
- Serialização binária IPC: `buffer_new`, `serialize`, `deserialize`
- `Deque<T>` (v0.5): `deque_new`, `deque_push_front`, `deque_push_back`, `deque_pop_front`, `deque_pop_back`, `deque_len`

## Tooling

- **`art build --target js`** — transpila para JavaScript ES2022 com source maps V3
- **`art build --bundle`** — bundle autocontido para Node.js e browser
- **ArtKit v0.1** (v0.5) — componentes reativos compilam para JS sem virtual DOM; scheduler assíncrono via `queueMicrotask`; lifecycle hooks `on_mount`, `on_destroy`, `on_update`
- **Type checker** — inferência local, verificação de anotações, inferência paramétrica
- **LSP completo** — completion, goto-def, hover, rename, semantic tokens (`art lsp`)
- **TTD shell** — `art debug --replay` com `step`, `breakpoint`, `state-at`; `--dap` para integração com editores DAP
- REPL limpo — exibe `=> valor` sem ruído de métricas
- Time-Travel Debugging: `--record` / `--replay` determinístico; delta snapshots
- Linter com detecção de hotspot de alocação (`art lint`)
- Formatter (`art format`), autodoc HTML (`art doc <arquivo>`)
- AOT experimental via C/LLVM (`art aot`)

---

## Instalação

### Forma rápida (Linux / macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/kitsuneislife/artcode/main/install.sh | bash
```

**Windows:** baixe o `.exe` na [página de releases](https://github.com/kitsuneislife/artcode/releases).

### Compilar do fonte

```bash
git clone https://github.com/kitsuneislife/artcode.git
cd artcode
cargo build -p cli --release
sudo cp target/release/art /usr/local/bin/
```

### Atualizar

```bash
art update --check   # verifica se há versão nova
art update --self    # autoatualização via script oficial
```

---

## Uso básico

```bash
# Executar script
art run examples/00_hello.art

# Modo puro (sem I/O)
art run --pure examples/27_pure_mode.art

# REPL interativo
art

# Lint
art lint meu_script.art

# Gerar HTML de documentação de um módulo
art doc meu_modulo.art

# Compilar para JavaScript
art build examples/00_hello.art --target js --out dist/
art build examples/00_hello.art --target js --bundle   # bundle autocontido

# Compilar componente ArtKit
art build examples/artkit/counter.art --target js --bundle --out dist/

# Time-travel: gravar e reproduzir
art run --record trace.artlog examples/44_ttd_keyframes.art
art debug --replay trace.artlog examples/44_ttd_keyframes.art

# Build e testes
cargo test --all
```

---

## Documentação

A pasta `docs/` está organizada em subpastas:

| Pasta | Conteúdo |
|---|---|
| [`docs/language/`](docs/language/) | Features da linguagem (enums, funções, generics, memória…) |
| [`docs/internals/`](docs/internals/) | Como o compilador funciona (interpreter, IR, parser…) |
| [`docs/guides/`](docs/guides/) | Guias práticos (instalação, migração, contribuição…) |
| [`docs/rfcs/`](docs/rfcs/) | RFCs de design (0001–0008) |

Links rápidos:

- [Visão geral](docs/overview.md)
- [Roadmap](docs/roadmap.md)
- [Notas e limitações conhecidas](docs/notes.md)
- [ArtKit — Quickstart](docs/guides/artkit_quickstart.md)
- [Migração v0.4 → v0.5](docs/guides/migration_v0.4_to_v0.5.md)
- [Memória (ARC / weak / arenas)](docs/language/memory.md)
- [Generics](docs/rfcs/0007-generics.md)
- [Concorrência e Actors](docs/language/concurrency.md)
- [Capabilities](docs/language/capabilities.md)
- [Time-Travel Debugging](docs/guides/debugging.md)
- [Contribuindo](docs/guides/contributing.md)
- [Changelog](CHANGELOG.md)

---

## Estrutura do projeto

```
crates/
  core/          AST, tokens, ambiente
  lexer/         Tokenizer
  parser/        Parser recursivo descendente (inclui ArtML + component blocks)
  interpreter/   Runtime (eval, exec, gc, actors, builtins…)
  diagnostics/   Erros com spans e sugestões
  typeck/        Type checker estático (inclui regras de componentes reativos)
  reactivity/    ReactivityPass, DepGraph, Tarjan SCC para detecção de ciclos
  codegen_js/    Codegen JavaScript ES2022 + source maps V3 + updaters cirúrgicos
  ir/            Representação intermediária
  jit/           JIT stub (LLVM, opcional)
cli/             Binário `art` com todos os subcomandos + bundler + JS_RUNTIME
examples/
  artkit/        Componentes ArtKit: counter.art, todo.art
```

---

## Contribuindo

- Leia `docs/guides/contributing.md` antes de mudanças maiores.
- Para mudanças de design, abra uma RFC em `docs/rfcs/` usando `0000-template.md`.
- Rode `cargo test --all` antes de submeter PR.

Licença MIT — veja `LICENSE`.
