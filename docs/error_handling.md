# Error Handling

Artcode combina dois mecanismos complementares de tratamento de erros:

- Propagacao de erros com operador `?` em expressoes (`Expr::Try`).
- Tratamento explicito com blocos `try/catch` em statements.

## Try/Catch

Sintaxe basica:

```art
try {
    let (a, b) = 10;
} catch err {
    println(err);
}
```

Comportamento atual:

- O bloco `catch` captura erros de runtime do tipo `TypeError`.
- O nome apos `catch` cria um binding local com a mensagem do erro (`String`).
- Se o bloco `try` nao falhar, o `catch` e ignorado.
- Retornos de funcao (`return`) nao sao interceptados por `catch`.

## Operador `?`

O operador `?` segue suportado para propagacao de erro em expressoes que usam enums estilo `Result`.

```art
func pipeline(x) {
    let ok = validate(x)?;
    return ok;
}
```

## Observacoes

- Esta implementacao cobre o fluxo essencial de tratamento explicito no runtime.
- Extensoes futuras podem incluir tipos de erro mais ricos e pattern matching direto em `catch`.
