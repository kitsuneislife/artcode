# Prelude e Tipos Built-in

O prelude inicializa o ambiente global com construções padrão necessárias para ergonomia da linguagem.

## Conteúdo Atual
- `Result { Ok(T), Err(E) }`
- Função builtin `println(value)`

## Enum Result
Registrado via `Interpreter::with_prelude()`.

Uso:
```
func dividir(a: Int, b: Int) -> Result<Int, ErroDiv> {
    if b == 0 { return .Err("zero") }
    return .Ok(a / b)
}

let r = dividir(10, 2)
match r {
    case .Ok(let v): println(f"valor={v}")
    case .Err(let e): println(f"erro={e}")
}
```

### Inferência Shorthand
`.Ok(123)` busca enum único contendo variante `Ok`.
- Sucesso se exatamente 1 enum registrado contém `Ok`.
- Erro se 0 (não encontrado) ou >1 (ambiguidade).

## Futuro Planejado
| Item | Status | Notas |
|------|--------|-------|
| Generics reais | Planejado | Substituir placeholders `T`/`E` por sistema de tipos formal |
| Prelude modular | Planejado | Opt-in para reduzir custo de inicialização |
| Outras coleções | Avaliação | Map, Set com semântica explícita |

## Boas Práticas
- Prefira pattern matching em vez de tentar “desembrulhar” diretamente.
- Evite variantes polimórficas excessivas; mantenha semântica clara.
