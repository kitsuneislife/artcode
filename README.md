![Banner](./github/banner.png)

<p align="center">
	<img alt="CI" src="https://github.com/kitsuneislife/artcode/actions/workflows/ci.yml/badge.svg" />
	<img alt="Coverage" src="https://img.shields.io/badge/Coverage-27BB3D.svg" />
	<img alt="License" src="https://img.shields.io/badge/license-MIT-blue.svg" />
</p>

Implementação experimental de uma linguagem interpretada em Rust com suporte a:

- Structs
- Enums (variantes com payload) + shorthand `.Variant` com detecção de ambiguidade
- Pattern matching com guards (`case .X(v) if v > 10:`)
- Funções e closures (captura léxica)
- Métodos em structs e enums com auto-binding de `self`
- Introspecção em métodos de enum (`variant`, `values`)
- f-Strings com format specs (`upper`, `lower`, `trim`, `hex`, `padN`, `debug` placeholder)
- Result-like enums e operador `?` (propagação inicial)
- Arrays com builtins (`sum`, `count`)
- Métricas de execução (handled_errors, executed_statements, crash_free%)

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
- Funções, closures e métodos com auto-binding de `self`
- f-Strings com format specs e re-lex/parsing das expressões internas
- Result-like enums e operador `?` para propagação de erros
- Blocos `performant {}` com arenas experimentais e análise conservadora de escape

Status do projeto
- Código modular em crates: `core`, `lexer`, `parser`, `interpreter`, `diagnostics`, `cli`.
- Testes: suíte unitária e de integração com exemplos em `cli/examples`.
- Ferramentas: `xtask` para cobertura e scripts para validar exemplos.

Rápido começo (Quickstart)

Prerequisitos: Rust stable (toolchain padrão).

Build e testes:
```bash
cargo test --all
```

Executar exemplos validados:
```bash
scripts/test_examples.sh
```

Executar o CLI (ex.: rodar um exemplo):
```bash
cargo run --bin art -- run cli/examples/00_hello.art
```

Design e diferenciais (curto)
- Complexidade Progressiva: níveis de abstração claros (ARC default → weak/unowned → arenas/performant).
- Diagnósticos de qualidade: `diagnostics` crate centraliza mensagens e spans para boa DX.
- Foco em interoperabilidade e PGO a médio prazo.

Contribuindo
- Leia `docs/` e as RFCs em `docs/rfcs/` antes de mudanças maiores.
- Use o checklist operacional em `/.kit/checklist.md` para priorizar trabalho.
- Para mudanças de design: abra uma RFC (`docs/rfcs/0000-template.md` quando existir) e link no PR.

Roadmap curto (prioridades atuais)
- Finalizar garantias de memória (weak/unowned e decrementos seguros).
- Harden dos blocos `performant` e testes de escape.
- Automatizar CI (já existe workflow mínimo) e adicionar fuzzing para parser/runtime.

Licença & contato
- Projeto com licença MIT (ver `LICENSE`).
- Para discussões de design: abra issues ou PRs no repositório.

---

Se quiser, eu posso também adicionar um badge de build/coverage visível no topo do README
ou abrir um PR com essas mudanças para revisão.

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
- [Enums & Pattern Matching](docs/enums.md)
- [Coverage & Métricas](docs/coverage.md)
- [Roadmap](docs/roadmap.md)
- [Sumário / Índice](docs/SUMMARY.md)

## Interoperabilidade / FFI (pasta `ffi/`)

A pasta `ffi/` contém esboços e diretrizes para integrar Artcode com C/Rust/WASM.
Resumo rápido:

- `ffi/README.md` — visão geral e recomendações de ownership ao passar strings e buffers.
- Convenções propostas:
	- Strings: `Arc<str>` ↔︎ `*const c_char` com funções helper de conversão.
	- Tipos primitivos: mapeamento direto (i64, f64, bool).
	- Ownership: documentar claramente quando a posse é transferida (caller/callee).
- PoC: exemplos simples devem viver em `ffi/examples/` (C wrapper e macro `art_extern!{}` no futuro).

Se você pretende usar Artcode em um projeto existente em Rust/C, veja `ffi/README.md`
para padrões recomendados e exemplos mínimos.

