# Enums e Pattern Matching

Enums modelam alternativas nomeadas e opcionais com parâmetros.

```
enum Resultado {
    Sucesso(Int)
    Falha(String)
}

let r = Resultado.Sucesso(10)
```

## Variantes Sem / Com Parâmetros
- `Estado.Ativo` (sem dados)
- `Resultado.Sucesso(42)` (dados posicionais)

## Shorthand `.Variante`
Uso abreviado sem nome do enum:
```
let ok = .Sucesso(1)
```
Regras:
- Aceito apenas se a variante existir em exatamente um enum carregado.
- Senão, erro de ambiguidade ou não encontrado.

## Pattern Matching
```
match r {
    case .Sucesso(let v): println(f"ok={v}")
    case .Falha(let e): println(f"erro={e}")
}
```
Bindings (ex: `let v`) capturam o valor interno quando a variante contém exatamente um parâmetro; caso múltiplos, o padrão deve detalhar cada posição.

## Implementação Interna
Representado como `ArtValue::EnumInstance { enum_name, variant, values }`.

### Verificação de Padrões
- Nome da variante precisa coincidir.
- Número de sub-padrões deve corresponder ao número de valores.
- Bindings retornam mapeamento `(nome, valor)` para injetar em novo escopo durante o `case`.

## Erros
| Cenário | Descrição |
|---------|-----------|
| Variante inválida | Variante não existe no enum registrado |
| Número de argumentos | Chamando variante com aridade incorreta |
| Ambiguidade shorthand | Mais de um enum contém a variante |

## Roadmap
- Suporte a variantes nomeadas com campos (`Resultado.Sucesso{ valor: 10 }`)
- Guard clauses em padrões (`case .Sucesso(let v) if v > 0:`)
- Derivações automáticas (Display, Debug) por macro futura
