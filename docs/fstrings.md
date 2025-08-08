# Interpolação de Strings (f-strings)

Artcode suporta f-strings: `f"texto {expressao} mais"`.

## Sintaxe
- Prefixo obrigatório `f` imediatamente antes de `"`.
- Dentro de `{ }` pode haver QUALQUER expressão válida da linguagem.
- Suporta chaves aninhadas e escapes.

## Escapes
| Sequência | Resultado |
|-----------|-----------|
| `{{` | `{` |
| `}}` | `}` |

## Exemplo
```
let a = 2
let b = 3
println(f"a={a} b={b} soma={a + b} quadrado={ (a + b) * (a + b) }")
```

## Implementação
1. Lexer emite `TokenType::InterpolatedString` contendo conteúdo bruto sem aspas.
2. Parser divide literal em partes (`InterpolatedPart::{Literal, Expr}`):
   - Scanner manual percorre chars, gerencia profundidade de `{`/`}`.
   - Para cada expressão, re-lexera e re-parseia trecho interno reutilizando o pipeline.
3. Interpreter concatena avaliando cada sub-expressão e chamando `to_string` nos valores.

## Limitações / TODO
- Erros usam `panic!` para casos de `{` não fechado ou `}` solto (melhorar para erro localizado).
- Não há formatação customizada (`{expr:format}`) ainda.
- Futuros conversores (ex: debug vs display) podem usar sintaxe `{:?}` estilo Rust.

## Testes Cobertos
- Expressões aritméticas
- Chaves aninhadas
- Escapes `{{` e `}}`

## Roadmap
| Item | Prioridade |
|------|------------|
| Diagnóstico com posição | Alta |
| Suporte a especificador de formato | Média |
| Reaproveitar tokens (evitar re-lex) | Baixa |
