# Art Language

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
- Erros: chaves não balanceadas disparam panic (futuro: melhorar para erro recuperável).

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

## Próximos Passos
- Melhorar sistema de erros para interpolação (diagnósticos com posição)
- Introduzir generics reais para enums/structs em vez de placeholders de string
- Suporte a métodos definidos pelo usuário (sintaxe de impl)
- Otimizações: reduzir clonagens de tokens/valores
