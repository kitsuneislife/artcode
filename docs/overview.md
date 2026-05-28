# Visão Geral do Projeto Artcode

Artcode é uma linguagem de programação experimental construída em Rust com foco em **Complexidade Progressiva**: iniciantes têm uma sintaxe simples; usuários avançados ganham mecanismos explícitos (pattern matching, f-strings, generics, actors, componentes reativos).

## Objetivos

- Construir uma base clara e modular (lexer, parser, core AST, interpreter, codegen, CLI) em direção a compilação JIT/AOT futura.
- Fornecer `Result`/`Option` e enums para modelagem de erros sem exceções implícitas.
- Suportar desenvolvimento de UIs reativas via `component {}` blocks com ReactivityPass e geração de JS cirúrgico.
- Manter execução determinística e transparente com Time-Travel Debugging.

## Arquitetura em Camadas

```
cli  ──► parser ──────► core (AST + tokens + env)
  \         ^                 ^
   \        |                 |
    \──► lexer ───────────────/
          |
          ├──► interpreter  (eval, exec, actors, builtins, GC, arena)
          ├──► typeck        (inferência local, diagnósticos de componentes)
          ├──► reactivity    (ReactivityPass, DepGraph, Tarjan SCC)
          ├──► codegen_js    (transpiler ES2022, source maps, bundler)
          └──► diagnostics   (erros com spans e sugestões)
```

### Crates

| Crate | Responsabilidade |
|---|---|
| `core` | AST, valores, tokens, ambiente léxico, padrões de match |
| `lexer` | Tokenizador com suporte a f-strings, keywords v0.5 (`component`, `view`, `state`, `prop`, `memo`, `ref`) |
| `parser` | Parser recursivo descendente; ArtML templates; `component {}` blocks |
| `interpreter` | Runtime: eval, exec, GC, actors, builtins, arena, IPC |
| `diagnostics` | Erros acumulativos com spans e sugestões |
| `typeck` | Type checker estático; 4 regras de diagnóstico para componentes |
| `reactivity` | ReactivityPass: analisa `ComponentBlock`, constrói DAG, detecta ciclos (Tarjan SCC) |
| `codegen_js` | Transpila AST para JavaScript ES2022; source maps V3; updaters cirúrgicos para componentes reativos |
| `ir` | Representação intermediária (scaffolded) |
| `jit` | JIT stub via LLVM (requer `--features=jit`) |
| `cli` | Binário `art` com todos os subcomandos + bundler + JS_RUNTIME |

## Fluxo de Execução

**Modo interpretador (`art run`):**
1. Ler fonte → Lexer → tokens
2. Parser → AST (`Program = Vec<Stmt>`)
3. Typeck → diagnósticos estáticos
4. Interpreter → executa statements, gerencia escopos, actors

**Modo compilação (`art build --target js`):**
1. Ler fonte → Lexer → Parser → AST
2. Typeck → valida tipos e componentes
3. `ReactivityPass` analisa `ComponentBlock` → `DepGraph`
4. Codegen JS emite JavaScript ES2022 com updaters cirúrgicos
5. Bundler injeta `JS_RUNTIME` (scheduler, lifecycle, DOM helpers)

## Decisões de Design

| Tema | Decisão | Racional |
|------|---------|----------|
| Memória | Adaptive ARC + arenas | Simplicidade com controle quando necessário |
| Erros | Diagnostics acumulativos + `Result` | Erros reportados sem panics, favorecendo IDE tooling |
| Reatividade | ReactivityPass + DAG | Ciclos em compile time; updates cirúrgicos sem virtual DOM |
| Componentes | `component {}` blocks → JS | Modelo declarativo compilado, não interpretado |
| Generics | Suporte no parser/AST | Monomorphização de runtime planejada para v0.6 |

## Estado Atual (v0.4 / v0.5-unreleased)

- `cargo test --all` verde; `cargo clippy -- -D warnings` limpo.
- Todos os 57 exemplos em `examples/` executam sem regressão.
- `component {}` blocks, qualificadores de binding e ReactivityPass entregues em v0.5.
- ArtKit v0.1: `counter.art` e `todo.art` funcionais; CI smoke test passando.
- JIT/AOT scaffolded mas sem compilação nativa funcional.
- Generics suportados no parser; monomorphização no interpreter não implementada.

## Contribuindo

1. Abrir issue ou RFC para mudanças estruturais.
2. Manter separação de responsabilidades entre crates.
3. Adicionar testes de integração em `crates/interpreter/tests/` para comportamento de linguagem.
4. `cargo xtask ci` roda pipeline local (fmt, clippy, testes).

---

Próximo: veja `docs/roadmap.md` para o estado atual e planos futuros.
