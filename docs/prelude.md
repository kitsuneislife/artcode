# Prelude e Tipos Built-in

O prelude inicializa o ambiente global com construções padrão necessárias para ergonomia da linguagem.

## Conteúdo Atual
- `Result { Ok(T), Err(E) }`
- Funções builtin: `println(value)`, `len(value)`, `type_of(value)`

## Builtins
Builtins são valores especiais representados internamente por `ArtValue::Builtin` e enumerados em `BuiltinFn`.

### `println`
Assinatura atual (provisória): `println(value: Any) -> None`

Características:
- Não variádica (apenas primeiro argumento é impresso; extras ignorados em versão futura ou gerarão diagnóstico quando suporte a aridade for estrito).
- Conversão via `Display` de cada tipo runtime.
- Retorna `None` (representado como `Optional(None)`).

Racional: manter custo de chamada mínimo sem criar `Function` sintética e sem checagens especiais no interpretador (despacho direto em `call_builtin`).

### `len`
Assinatura: `len(value: Array|String) -> Int`

Erros:
- Tipo não suportado gera diagnostic `len: unsupported type`.
- Falta de argumento gera diagnostic `len: missing argument`.

### `type_of`
Assinatura: `type_of(value: Any) -> String`

Retorna nome simples do tipo dinâmico. Falta de argumento gera diagnostic `type_of: missing argument`.

Próximos passos planejados para builtins:
| Item | Objetivo | Observação |
|------|----------|------------|
| Variádico controlado | `println(a, b, ...)` | Implementar coleta incremental evitando vetor intermediário grande |
| `len` | Tamanho de array/string | (Implementado) Diagnóstico para tipos não suportados |
| `type_of` | Inspeção de tipo | (Implementado) Suporte a debugging e REPL |
| Registry modular | Opt-in | Permitir runtime mínimo para scripts embed |

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
