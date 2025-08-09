# Interpreter e Execução

## Responsabilidades
- Executar statements sequencialmente
- Avaliar expressões e produzir `ArtValue`
- Gerenciar ambientes léxicos aninhados
- Manter registro de tipos (structs, enums)
- Resolver pattern matching (com guards) e bindings
- Avaliar f-strings com especificadores de formatação
- Registrar e despachar métodos de structs e enums
- Coletar métricas de execução (erros tratados, statements executados)

## Valores (`ArtValue`)
- Primitivos: Int, Float, Bool, String
- Optional (None / Some)
- Array
- StructInstance
- EnumInstance
- Function (closure)
- BoundFunction (função já ligada a um `self` interno para métodos)

## Chamadas & Métodos
Fluxo:
1. Avalia callee
2. Despacho:
   - Function -> cria ambiente filho, injeta parâmetros
   - BoundFunction -> usa ambiente baseado no `self` pré-capturado
   - Método: `inst.metodo()` é resolvido via lookup no `TypeRegistry` produzindo BoundFunction (auto-binding de `self` quando primeiro parâmetro nomeado exatamente `self`)
   - EnumInstance sem payload e chamado como construtor parcial: (futuro refinement)
   - FieldAccess sem args sobre valor não chamável -> retorna o valor (fallback legado para arrays builtin)

Introspecção em enums dentro de métodos: nomes especiais `variant` e `values` são injetados ao entrar no corpo do método de enum (nome da variante e array dos valores/payloads).

## Pattern Matching
`pattern_matches` retorna vetor de bindings `(nome, valor)` quando o padrão casa.
Suporta:
- Literal
- Wildcard `_`
- Binding `let v` (internamente representado como `Binding(Token)`)
- EnumVariant com parâmetros recursivos
- Guards: `case P if expr =>` só casa se `expr` for truthy após casar `P`.

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

Todos os erros convertem-se em `Diagnostic` agregado; o interpretador não panica em entradas malformadas de usuário (meta: crash-free). Panics restantes são tratados como bugs e rastreados pelo script de scan.

## f-Strings & Format Specs
Pipeline:
1. Lexer produz token `InterpolatedString` bruto.
2. Parser divide em partes (texto literal, expressões, expr+spec).
3. Durante execução cada expressão é avaliada e specs aplicadas em ordem:
   - `upper` / `lower` / `trim`
   - `hex` (para inteiros -> hex minúsculo)
   - `padN` (N decimal, padding à direita com espaços)
   - `debug` (renderização estruturada futura; placeholder atual usa Display)
Erros como chaves desbalanceadas ou `pad` inválido viram diagnostics lex/parse dedicados.

## Métricas
Contadores internos:
- `handled_errors`: incrementado ao agregar diagnostic de runtime
- `executed_statements`: incrementado por statement avaliado
- `crash_free%`: derivado para relatórios (futuro endpoint / comando)

Esses dados suportam monitorar estabilidade e orientar otimizações antes do JIT.

## Otimizações Futuras
- Caching de lookup de variáveis
- Representação interna especializada para arrays numéricos
- JIT tiered (baseline + otimizado)
- Inline caching para chamadas de método frequentes
- Especialização de BoundFunction (monomorfização leve)

## Segurança / Determinismo
Sem side-effects escondidos além de `println`. Futuro: sandbox para FFI, mais validações de ownership nas fronteiras.
