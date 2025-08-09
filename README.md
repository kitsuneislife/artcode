# Art Language

![Coverage](./target/coverage_badge.svg)

Implementação experimental de uma linguagem interpretada em Rust com suporte a:

- Estruturas (struct)
- Enums com variantes e inferência de tipo em uso abreviado (.Ok / .Err)
- Pattern matching simples
- Funções e escopos léxicos (closures via ambiente capturado)
- Resultado (enum Result) pré-registrado via prelude
- Interpolação de strings estilo f-string com expressões arbitrárias (`f"valor={a + b}"`), incluindo aninhamento de chaves e escapes `{{` e `}}`

## Prelude

`Interpreter::with_prelude()` registra automaticamente o enum `Result { Ok(T), Err(E) }` permitindo:

```
let v = .Ok(123);
let e = .Err("falhou");
```

Se múltiplos enums compartilharem a mesma variante, o uso abreviado gera erro de ambiguidade.

## Interpolação de Strings

Sintaxe: `f"texto {expressao} mais texto"`.

Regras:
- Expressões completas são lexers/parsers recursivamente.
- Suporte a chaves aninhadas `{ {a + {b}} }`.
- Escapes: `{{` produz `{`, `}}` produz `}`.
- Erros agora geram diagnostics estruturados (sem panics) com spans e mensagem.

## Testes

Cobrem:
- Expressões em f-strings
- Escapes e chaves aninhadas
- Inferência abreviada de enum
- Escopo preservado em chamadas de função
- Acesso de campo/pseudo-métodos em arrays (sum, count)

Execute:
```
cargo test
```

Para cobertura local:
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
Ver diretório `docs/` para detalhes (overview, parser_lexer, interpreter). Focos imediatos:
- Sistema de tipos incremental (inferência básica, enum/struct metadata)
- Performance fundacional (intern pool, redução de clones)
- Builtins estruturados (remoção de caso especial de println)
- Cobertura & tooling (script unificado, cobertura mínima)

