# Concorrência (Visão & MVP)

Este documento descreve o escopo do MVP de concorrência (Fase 9) baseado em atores.

Resumo rápido (MVP):
- `spawn actor { ... }` cria ator com mailbox FIFO.
- Builtins: `actor_send(actor, value)` e `actor_receive()`.
- Scheduler cooperativo single-threaded (round-robin) para evitar exigir Send/Sync em ArtValue.
- Backpressure: limite por mailbox com erro diagnóstico quando excedido.

Casos de teste desejados:
- Enfileiramento e ordem FIFO.
- Backpressure (erro quando mailbox cheia).
- Actor pode encerrar-se e liberar recursos.

Exemplos básicos

Spawn e enviar mensagens:

```art
let a = spawn actor {
	// esperar por mensagens
	let msg = actor_receive();
	println(msg);
}

actor_send(a, 1);
actor_send(a, 2);
```

Receber envelope completo (sender, payload, priority):

```art
let a = spawn actor { /* body that calls actor_receive_envelope() */ };
// dentro do actor, actor_receive_envelope() retorna um `Envelope` struct com campos `sender`, `payload`, `priority`
let env = actor_receive_envelope();
// acessar campos: env.sender, env.payload, env.priority
```

Backpressure:

```art
let a = spawn actor { /* ... */ };
actor_set_mailbox_limit(a, 1);
let ok = actor_send(a, 1); // true
let ok2 = actor_send(a, 2); // false (mailbox limit)
```

Construir um envelope manualmente

```art
// em tempos de construção dinâmica, é mais conveniente criar um Envelope via builtin
let e = envelope(None, 42, 5);
// e é um objeto heapificado com tipo `Envelope` e campos `sender`, `payload`, `priority`
```

Notas de design e trade-offs estão na RFC `docs/rfcs/0003-actors.md`.
