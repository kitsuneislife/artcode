# Pipeline Lazy de Streams

Artcode suporta um slice de streams lazy para pipelines com `|>` sem gerar arrays intermediarios entre transformacoes.

## Forma suportada

```art
let out = [1, 2, 3, 4, 5] |> stream |> map(inc) |> filter(is_even) |> collect
let n = [1, 2, 3, 4, 5] |> stream |> map(inc) |> filter(is_even) |> count
```

Semantica atual:
- `stream(array)` cria um pipeline lazy sobre um array fonte.
- `map(stream, fn)` registra transformacao lazy.
- `filter(stream, pred)` registra filtro lazy.
- `collect(stream)` materializa resultado final em array.
- `count(stream)` conta elementos resultantes sem materializar array final.
- `for item in stream_pipeline { ... }` permite iteracao direta sobre o resultado lazy do pipeline.

Observacao:
- As etapas `map/filter` nao criam arrays intermediarios; a execucao ocorre em passe unico na etapa terminal (`collect`/`count`).

## Exemplo

Veja [examples/37_stream_pipeline.art](../examples/37_stream_pipeline.art).

## Validacao

A cobertura desta fase inclui:
- Runtime: composicao lazy de `stream/map/filter`.
- Runtime: terminais `collect` e `count`.
- Runtime: iteracao de `for` sobre stream pipeline.
- CLI: integracao de `art run` para pipeline lazy completo.
