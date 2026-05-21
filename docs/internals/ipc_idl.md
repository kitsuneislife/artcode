# IDL de IPC (MVP)

Este documento descreve o MVP de IDL para IPC no Artcode usando o proprio sistema de tipos de `struct`.

## Objetivo

Permitir que mensagens de IPC sejam declaradas por tipos de dominio (structs) e validadas em runtime de forma deterministica.

## Builtins

- `idl_schema(struct_name: String)`
  - Retorna um `Map` com `campo -> tipo` da struct registrada.
- `idl_validate(message: Any, struct_name: String)`
  - Valida se a mensagem segue o schema da struct e retorna `Bool`.
  - Em caso de mismatch, emite diagnostico runtime com o campo esperado/encontrado.

## Exemplo

```art
struct BootMsg {
    service: String,
    retries: Int
}

let msg = BootMsg { service: "nexus", retries: 3 }
let ok = idl_validate(msg, "BootMsg") // true

let bad = BootMsg { service: "nexus", retries: "oops" }
let nok = idl_validate(bad, "BootMsg") // false + diagnostico
```

## Escopo do MVP

- Fonte de schema: `struct` registrada no runtime.
- Tipos suportados na validacao: `Int`, `Float`, `Bool`, `String`, `Array`, `Tuple`, `Optional<T>`, `Array<T>`, nome de `Struct` e nome de `Enum`.
- Sem serializacao binaria neste slice (item separado da checklist).

## Roadmap relacionado

- Checklist v0.2, secao "Tipagem e IPC":
  - IDL: concluido neste MVP.
  - Capabilities: baseline concluido com token move-only (`capability_acquire`, `capability_kind`).
  - Serializacao zero-copy: baseline concluido com `Buffer`, `serialize` e `deserialize`.
