![Banner](/banner.png)
<p align="center">
	<img alt="CI" src="https://github.com/kitsuneislife/artcode/actions/workflows/ci.yml/badge.svg" />
	<a href="docs/coverage.md"><img alt="Coverage" src="https://img.shields.io/badge/Coverage-docs-blue.svg" /></a>
	<img alt="License" src="https://img.shields.io/badge/license-MIT-blue.svg" />
	<a href="https://github.com/kitsuneislife/artcode/issues"><img alt="issues - artcode" src="https://img.shields.io/github/issues/kitsuneislife/artcode" /></a>
	<a href="https://github.com/kitsuneislife/artcode"><img alt="stars - artcode" src="https://img.shields.io/github/stars/kitsuneislife/artcode?style=social" /></a>
</p>

Implementação experimental de uma linguagem interpretada em Rust com suporte a:

- Structs
- Enums (variantes com payload) + shorthand `.Variant` com detecção de ambiguidade
- Pattern matching com guards (`case .X(v) if v > 10:`)
- Loops nativos (`while`, `for`) e tuplas com destructuring (`let (a, b) = value`)
- Tratamento explicito de erro com `try/catch` (alem do operador `?`)
- Modo de execução `--pure` para bloquear operações de I/O e não-determinismo em configurações seguras
- Ferramenta de DAG para resolver ordem topológica de dependências (`dag_topo_sort`)
- Funções e closures (captura léxica)
- Métodos em structs e enums com auto-binding de `self`
- Introspecção em métodos de enum (`variant`, `values`)
- f-Strings com format specs (`upper`, `lower`, `trim`, `hex`, `padN`, `debug` placeholder)
- Result-like enums e operador `?` (propagação inicial)
- Arrays com builtins (`sum`, `count`)
- Standard Library embutida: Collections (Map, Set), Math (abs, pow, clamp), Time & Rand, File IO (sandboxed).
- Time-Travel Debugging com trace determinístico e keyframes/checkpoints em arquivo `.artlog` (`--record` / `--replay` no modo debug)
- Sintaxe shell com statement `$ comando args...`, pipeline `|>`, retorno tipado em `shell_result` e chamada estilo função (`echo("...")`)
- Operador de pipeline para expressoes (`valor |> fn(...)`) com encadeamento funcional
- Pipeline lazy de streams com `stream |> map |> filter |> collect/count` sem arrays intermediarios entre etapas
- Métricas de execução (handled_errors, executed_statements, crash_free%)
- Language Server Protocol (LSP) com diagnósticos, autocomplete, goto-definition, rename e semantic tokens na IDE (`art lsp`)

# Complexidade Progressiva

Artcode é uma linguagem experimental implementada em Rust que foca em ser
fácil para iniciantes e potente para especialistas — "Complexidade Progressiva":
comece com ARC simples e suba a escada para arenas, weak/unowned e blocos
performant quando precisar de controle de memória e máxima performance.

Por que Artcode é relevante
- Projeto modular, com lexer, parser, interpreter e runtime separados.
- Diagnósticos estruturados: erros com spans e sugestões, sem panics em parsing.
- Modelo de memória pragmático: ARC por padrão + weak/unowned explícitos;
	ferramenta de detecção de ciclos para testes.
- Plano de JIT/AOT com PGO: permite otimizações guiadas por perfil quando
	chegar a etapa de compilação AOT.

Principais recursos
- Structs e Enums (variants com payloads e shorthand `.Variant` com checagem de ambiguidade)
- Pattern matching com guards
- Loops `while` e `for` com execução em runtime e inferência de tipos conservadora
- Tuplas literais e destructuring por pattern (`let (a, b) = expr`)
- Error handling explicito por statements `try/catch`
- Modo `run --pure` para execução sem operações impuras (`println`, `io_*`, `time_now`, `rand_*`)
- Ordenação topológica de dependências para cenários de boot/configuração
- Funções, closures e métodos com auto-binding de `self`
- f-Strings com format specs e re-lex/parsing das expressões internas
- Standard Library Expansiva (Coleções Padrão de Map/Set, Manipulação de Matemática e IO Simples)
- Result-like enums e operador `?` para propagação de erros
- Blocos `performant {}` com arenas experimentais e análise conservadora de escape
- APIs de arena reutilizavel no stdlib (`arena_new`, `arena_with`, `arena_release`) para workloads de baixo nivel
- IDL de IPC via structs com introspecao/validacao runtime (`idl_schema`, `idl_validate`)
- Capabilities move-only para IPC/autorizacao (`capability_acquire`, `capability_kind`)
- Serializacao binaria de IPC (`buffer_new`, `serialize`, `deserialize`) com restricoes para tipos opacos
- Sintaxe shell via statement `$` e chamada estilo função para executáveis no PATH, com retorno `Result` em `shell_result` e bloqueio automático em `--pure`
- Operador `|>` para pipeline de expressoes (transformado para chamada com insercao do argumento a esquerda)
- Streams lazy para pipelines de dados (`stream/map/filter/collect/count`) em passe unico na etapa terminal
- FFI baseline para C-ABI com call-gate seguro por handles opacos (`art_handle_*`) e gateway `unsafe` de syscall por registradores (`art_syscall_unsafe`)

Status do projeto
- Código modular em crates: `core`, `lexer`, `parser`, `interpreter`, `diagnostics`, `cli`.
- Testes: suíte unitária e de integração com exemplos em `examples`.
- Ferramentas: `xtask` para cobertura e scripts para validar exemplos.

## Instalação

### Forma rápida (Linux / macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/kitsuneislife/artcode/main/install.sh | bash
```

Isso baixa o binário da [última release](https://github.com/kitsuneislife/artcode/releases) e instala em `/usr/local/bin/art`.

**Windows:** baixe o `.exe` direto na [página de releases](https://github.com/kitsuneislife/artcode/releases).

### Compilar a partir do fonte

Prerequisitos: Rust stable toolchain (`curl https://sh.rustup.rs -sSf | sh`).

```bash
git clone https://github.com/kitsuneislife/artcode.git
cd artcode
cargo build -p cli --release
sudo cp target/release/art /usr/local/bin/
```

### Uso básico

```bash
# Executar um script
art run examples/00_hello.art

# Executar em modo puro (sem I/O e sem fontes de não-determinismo)
art run --pure examples/27_pure_mode.art

# Métricas de execução
art metrics --json meu_script.art
# (inclui sumário de GC/ciclos em cycle_summary e cycle_leaks_detected)

# Documentação da stdlib (autogerada a partir do prelude)
art doc std

# Checagem de migração entre versões
art upgrade --check examples/31_upgrade_migration.art

# Lint com heurística de hotspot de alocação em loops
art lint examples/23_linter_tests.art
# (inclui validação semântica de weak/unowned e uso de postfix `?`/`!`)

# Fuzzing contínuo (parser/loops)
bash scripts/run_fuzz_ci.sh 60

# Fluxo actor request/response com HTTP básico
art run examples/33_actor_http_runtime.art

# Closures retornadas e callbacks com captura de ambiente (ARC)
art run examples/34_closure_callbacks_arc.art

# Sintaxe shell com execução de processo externo
art run examples/35_shell_syntax.art

# Shell via chamada de função (mapeamento PATH)
art run examples/39_shell_function_call.art

# Arenas reutilizáveis via stdlib
art run examples/40_reusable_arena.art

# IDL de IPC via structs
art run examples/41_idl_ipc.art

# Capabilities move-only
art run examples/42_capability_tokens.art

# Serializacao binaria para IPC
art run examples/43_ipc_serialization.art

# Time-travel com keyframes/checkpoints
art run --record examples/44_ttd_keyframes.artlog examples/44_ttd_keyframes.art

# Exemplo de highlights de release/changelog
art run examples/45_release_changelog.art

# Pipeline de expressoes
art run examples/36_pipeline_operator.art

# Pipeline lazy de streams
art run examples/37_stream_pipeline.art

# Language Server (LSP) para editores
art lsp
# (suporta diagnósticos, autocomplete, goto-definition/rename multi-arquivo com indexação recursiva de imports e semantic tokens)

# Build e testes (desenvolvimento)
cargo test --all
```


Design e diferenciais (curto)
- Complexidade Progressiva: níveis de abstração claros (ARC default → weak/unowned → arenas/performant).
- Diagnósticos de qualidade: `diagnostics` crate centraliza mensagens e spans para boa DX.
- Parser/runtime com string interning (`intern` + `intern_arc`) para reduzir alocações repetidas em símbolos e literais.
- Resolver de módulos com coleta paralela de dependências e emissão determinística do programa final.
- Foco em interoperabilidade e PGO a médio prazo.

Contribuindo
- Leia `docs/` e as RFCs em `docs/rfcs/` antes de mudanças maiores.
- Use o checklist operacional em `/.kit/checklist-v0.2.0.md` para priorizar trabalho.
- Para mudanças de design: abra uma RFC (`docs/rfcs/0000-template.md` quando existir) e link no PR.

Licença & contato
- Projeto com licença MIT (ver `LICENSE`).
- Para discussões de design: abra issues ou PRs no repositório.

---

## Documentação (pasta `docs/`)

A pasta `docs/` contém material técnico e de design — roteiros que explicam decisões
arquiteturais e guias de contribuição:

- `overview.md` — visão geral da linguagem e arquitetura dos crates.
- `parser_lexer.md` — como o lexer e o parser foram projetados, spans e diagnostics.
- `interpreter.md` — runtime model, representações de valores e execução.
- `memory.md` — especificação do modelo de memória (ARC, weak/unowned, arenas).
- `fstrings.md`, `functions.md`, `enums.md` — guias de recursos e exemplos.

Leia `docs/SUMMARY.md` para um índice rápido. Se você for contribuir com mudanças de linguagem,
crie uma RFC em `docs/rfcs/` e referencie-a nas PRs.

Links rápidos para os principais documentos:

- [Visão geral](docs/overview.md)
- [Lexer & Parser](docs/parser_lexer.md)
- [Interpreter (runtime)](docs/interpreter.md)
- [Memória (ARC / weak / arenas)](docs/memory.md)
- [f-Strings (format specs)](docs/fstrings.md)
- [Funções & Closures](docs/functions.md)
- [Loops & Tuplas](docs/loops_tuples.md)
- [Error Handling](docs/error_handling.md)
- [Modo Pure](docs/pure_mode.md)
- [DAG de Dependências](docs/dependency_dag.md)
- [IDL de IPC](docs/ipc_idl.md)
- [Capabilities](docs/capabilities.md)
- [Serializacao IPC](docs/ipc_serialization.md)
- [Sintaxe Shell](docs/shell_syntax.md)
- [Operador Pipeline](docs/pipeline_operator.md)
- [Pipeline Lazy de Streams](docs/stream_pipeline.md)
- [Enums & Pattern Matching](docs/enums.md)
- [Coverage & Métricas](docs/coverage.md)
- [Roadmap](docs/roadmap.md)
- [Versionamento Público](docs/versioning.md)
- [Guia de Migração](docs/migration.md)
- [Changelog](CHANGELOG.md)
- [Website Changelog](website/changelog.html)
- [Concorrência (Atores)](docs/concurrency.md)
- [Sumário / Índice](docs/SUMMARY.md)

## Interoperabilidade / FFI (pasta `docs/`)

A pasta `docs/` contém esboços e diretrizes para integrar Artcode com C/Rust/WASM.
Resumo rápido:

- `docs/ffi.md` — visão geral e recomendações de ownership ao passar strings e buffers.
- Convenções propostas:
	- Strings: `Arc<str>` ↔︎ `*const c_char` com funções helper de conversão.
	- Tipos primitivos: mapeamento direto (i64, f64, bool).
	- Ownership: documentar claramente quando a posse é transferida (caller/callee).
- PoC: exemplos simples devem viver em `examples/docs/` (C wrapper e macro `art_extern!{}` no futuro).

Se você pretende usar Artcode em um projeto existente em Rust/C, veja `docs/ffi.md`
para padrões recomendados e exemplos mínimos.

