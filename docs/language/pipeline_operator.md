# Operador Pipeline de Expressoes

Artcode suporta pipeline de expressoes com `|>` para encadear transformacoes sem nesting manual de chamadas.

## Forma suportada

```art
let x = 10 |> inc
let y = 10 |> inc |> mul(3)
let z = 5 |> add(7)
```

Semantica atual:
- `left |> fn` vira `fn(left)`.
- `left |> fn(a, b)` vira `fn(left, a, b)`.
- Encadeamento e aplicado da esquerda para a direita.
- O slice atual e funcional para funcoes/callables, sem otimizacao especial de alocacao de streams.

## Exemplo

Veja [examples/36_pipeline_operator.art](../examples/36_pipeline_operator.art).

## Relacao com pipeline shell

- Pipeline shell (`$ ... |> ...`) continua disponivel para processos externos.
- Pipeline de expressoes (`a |> f`) opera no nivel de AST/calls da linguagem.

## Validacao

A cobertura desta fase inclui:
- Parser: transformacao de `|>` em chamadas (`fn(left, ...)`).
- Runtime: execucao de chamadas encadeadas via pipeline.
- CLI: teste de integracao com `art run`.
