# Capabilities (Move-Only)

O runtime do Artcode possui um baseline de capabilities para cenarios de IPC e autorizacao explicita.

## Builtins

- `capability_acquire(kind: String)`
  - Cria um token nao forjavel de capability no runtime.
- `capability_kind(capability: Capability)`
  - Retorna o kind declarativo da capability (ex.: `NetBind`).

## Semantica de move

Capabilities sao **move-only** no runtime:

- Ler uma variavel de capability consome o token.
- Reutilizar a mesma variavel apos o consumo gera diagnostico runtime.

Isso evita duplicacao acidental de handles de autorizacao no userspace.

## Exemplo

```art
let cap = capability_acquire("NetBind")
let kind = capability_kind(cap)
println(f"kind={kind}")
```

Veja tambem: `examples/42_capability_tokens.art`.
