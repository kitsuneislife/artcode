# Interpreter e Execução

## Responsabilidades
- Executar statements sequencialmente
- Avaliar expressões e produzir `ArtValue`
- Gerenciar ambientes léxicos aninhados
- Manter registro de tipos (structs, enums)
- Resolver pattern matching e bindings

## Valores (`ArtValue`)
- Primitivos: Int, Float, Bool, String
- Optional (None / Some)
- Array
- StructInstance
- EnumInstance
- Function (closure)

## Chamadas
1. Avalia callee
2. Discrimina pelo tipo de valor:
   - Function -> prepara novo ambiente
   - EnumInstance sem valores -> trata como construtor parcial
   - FieldAccess fallback sem args -> retorna valor direto

## Pattern Matching
`pattern_matches` retorna vetor de bindings `(nome, valor)` quando o padrão casa.
Suporta:
- Literal
- Wildcard `_`
- Binding `let v` (internamente representado como `Binding(Token)`)
- EnumVariant com parâmetros recursivos

## Try / Result
`Expr::Try(inner)` extrai `Ok` ou retorna via `RuntimeError::Return` se `Err` (protótipo inicial). Futuro: mecanismo distinto de propagação.

## Erros de Execução
| Tipo | Condição |
|------|----------|
| UndefinedVariable | Nome não encontrado no chain de ambientes |
| TypeMismatch | Operação inválida para tipos |
| DivisionByZero | Divisão por zero |
| WrongNumberOfArguments | Aridade incorreta em chamada |
| InvalidEnumVariant | Variante não existe |
| MissingField | Campo ausente em init de struct |
| Other | Mensagens gerais |

## Otimizações Futuras
- Caching de lookup de variáveis
- Representação interna especializada para arrays numéricos
- JIT tiered (baseline + otimizado)

## Segurança / Determinismo
Sem side-effects escondidos além de `println`. Futuro: sandbox para FFI.
