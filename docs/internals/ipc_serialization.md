# Serializacao IPC (Buffer / Zero-Copy Baseline)

Artcode fornece uma base de serializacao binaria para trafego de mensagens em IPC.

## Builtins

- `buffer_new(size: Int)`
  - Cria um buffer binario heap-backed.
- `serialize(value: Any)`
  - Codifica um valor suportado para `Buffer`.
- `deserialize(buffer: Buffer)`
  - Decodifica um buffer para valor Artcode.

## Tipos suportados (baseline)

- Primitivos: `Int`, `Float`, `Bool`, `String`
- Compostos: `Array`, `Tuple`, `Map`, `Set`, `Optional`, structs/enums serializaveis

## Restricoes

Tipos opacos/sensíveis nao sao serializados:

- `Capability`
- `Actor`
- `Function`
- Handles de runtime que nao possuem representacao deterministica de IPC

Quando ocorrer tentativa de serializar tipo proibido, o runtime emite diagnostico e retorna `None`.

## Exemplo

```art
let payload = map_new()
map_set(payload, "service", "nexus")
map_set(payload, "retries", 3)

let buf = serialize(payload)
let decoded = deserialize(buf)
println(decoded)
```

Veja tambem: `examples/43_ipc_serialization.art`.
