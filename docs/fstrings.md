# Interpolação de Strings (f-strings)

Artcode suporta f-strings: `f"texto {expressao} mais"`.

## Sintaxe
- Prefixo obrigatório `f` imediatamente antes de `"`.
- Dentro de `{ }` pode haver QUALQUER expressão válida da linguagem.
- Suporta chaves aninhadas e escapes.
- Formatação opcional: `{expressao:spec}`.

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
2. Parser divide literal em partes (`InterpolatedPart::{Literal, Expr { expr, format }}`):
   - Scanner manual percorre chars, gerencia profundidade de `{`/`}`.
   - Se encontrar `:` na profundidade zero dentro das chaves, separa expressão de especificador.
   - Para cada expressão, re-lexera e re-parseia trecho interno reutilizando o pipeline.
3. Interpreter avalia cada sub-expressão, converte via `Display` e aplica spec se presente.

### Specs Suportadas (básicas)
| Spec | Efeito |
|------|--------|
| `upper` | Converte para maiúsculas |
| `lower` | Converte para minúsculas |
| `trim`  | Remove espaços em volta |
| `debug` | Usa representação `Debug` (provisória) |
| `hex`   | Inteiro em hexadecimal (ex: 255 -> `0xFF`) |
| `padN`  | Padding à direita até largura N (ex: `pad10`) |

Specs desconhecidas são ignoradas silenciosamente.

## Limitações / TODO
- Erros de sintaxe interna ainda não produzem spans precisos nas f-strings.
- Specs avançadas (alinhamento, preenchimento customizado, precisão numérica) não suportadas.
- `debug` usa fallback genérico; poderá divergir no futuro.

## Testes Cobertos
- Expressões aritméticas
- Chaves aninhadas
- Escapes `{{` e `}}`

## Roadmap
| Item | Prioridade |
|------|------------|
| Diagnóstico com posição | Alta |
| Especificadores adicionais (alinhamento, precisão) | Média |
| Reaproveitar tokens (evitar re-lex) | Baixa |
