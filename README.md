# Art Language

![Coverage](./target/coverage_badge.svg)

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

## Prelude

`Interpreter::with_prelude()` registra automaticamente o enum `Result { Ok(T), Err(E) }` permitindo:

```
let v = .Ok(123);
let e = .Err("falhou");
```

Se múltiplos enums compartilharem a mesma variante, o uso abreviado gera erro de ambiguidade.

## f-Strings & Format Specs

Sintaxe: `f"texto {expressao} mais texto"`.

Regras:
- Expressões completas re-lexadas e re-parsadas.
- Chaves aninhadas suportadas; `{{` / `}}` para literais.
- Specs em `{expr:spec}` aplicadas sequencialmente: `upper`, `lower`, `trim`, `hex`, `pad10`, `debug`.
- Erros produzem diagnostics estruturados (sem panics) com spans.

Exemplo:
```
let n=255; let s="  AbC  "; println(f"hex={n:hex} {s:trim:upper} pad={s:pad10}");
```

## Exemplos e Testes

Exemplos numerados em `cli/examples` (00–12) cobrem a linguagem progressivamente. Rodar todos:
```
scripts/test_examples.sh
```
Eles também são validados em `cargo test` via teste integrado.

Suite de testes Rust (`cargo test`) cobre parsing, runtime (f-strings, métodos, pattern matching, enum arity, diagnostics). Para cobertura local:
```
cargo run -p xtask -- coverage --html
```

## Métricas de Qualidade
CLI imprime métricas de execução:
```
[metrics] handled_errors=3 executed_statements=120 crash_free=97.5%
```
Objetivo: crash_free >= 99%. Warnings/panics inesperados devem ser convertidos em diagnostics estruturados.

## Arquitetura & Próximos Passos
Ver `docs/` (overview, parser_lexer, interpreter). Próximos focos:
- Incrementar sistema de tipos (propagação mais rica, checagem de campos em parse)
- Blocos `impl` para agrupar métodos
- Inline caching / otimizações de dispatch
- Início do pipeline JIT/AOT + coleta de perfis
- Elevar threshold de cobertura gradualmente (>80%)

