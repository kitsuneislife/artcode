![Banner](/banner.png)
<p align="center">
	<img alt="CI" src="https://github.com/kitsuneislife/artcode/actions/workflows/ci.yml/badge.svg" />
	<a href="docs/internals/coverage.md"><img alt="Coverage" src="https://img.shields.io/badge/Coverage-docs-blue.svg" /></a>
	<img alt="License" src="https://img.shields.io/badge/license-MIT-blue.svg" />
	<a href="https://github.com/kitsuneislife/artcode/issues"><img alt="issues - artcode" src="https://img.shields.io/github/issues/kitsuneislife/artcode" /></a>
	<a href="https://github.com/kitsuneislife/artcode"><img alt="stars - artcode" src="https://img.shields.io/github/stars/kitsuneislife/artcode?style=social" /></a>
</p>

Implementação experimental de uma linguagem interpretada em Rust — **v0.3.0** (2026-05-21).

# Complexidade Progressiva

Artcode é projetada com um princípio central: **comece simples, escale quando precisar**.
ARC implícito no nível padrão; weak/unowned e arenas explícitas quando você precisa de controle fino;
generics com constraints, actors e capabilities para problemas avançados.

## Features da linguagem

- Structs, Enums (variantes com payload) e pattern matching com guards
- Loops nativos (`while`, `for`), tuplas e destructuring (`let (a, b) = value`)
- Funções, closures com captura léxica e métodos com auto-binding de `self`
- **`impl Type { }` blocks** — syntax para agrupar métodos por tipo (novo em v0.3)
- **Generics com constraints** — `func foo<T: Numeric>(x: T)` valida tipos em runtime (novo em v0.3)
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

## Tooling

- LSP com diagnósticos, autocomplete e goto-definition (`art lsp`)
- **Diagnósticos com linha:coluna** em erros de runtime (novo em v0.3)
- **REPL limpo** — exibe `=> valor` sem ruído de métricas (novo em v0.3)
- Time-Travel Debugging: `--record` / `--replay` determinístico
- Linter com detecção de hotspot de alocação (`art lint`)
- Formatter (`art format`), autodoc de stdlib (`art doc std`)
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

# Autodoc da stdlib
art doc std

# Time-travel: gravar e reproduzir
art run --record trace.artlog examples/44_ttd_keyframes.art
art run --replay trace.artlog examples/44_ttd_keyframes.art

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
  parser/        Parser recursivo descendente
  interpreter/   Runtime (eval, exec, gc, actors, builtins…)
  diagnostics/   Erros com spans e sugestões
  ir/            Representação intermediária
  jit/           JIT stub (LLVM, opcional)
cli/             Binário `art` com todos os subcomandos
```

---

## Contribuindo

- Leia `docs/guides/contributing.md` antes de mudanças maiores.
- Para mudanças de design, abra uma RFC em `docs/rfcs/` usando `0000-template.md`.
- Rode `cargo test --all` antes de submeter PR.

Licença MIT — veja `LICENSE`.
