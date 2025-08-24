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

Notas de design e trade-offs estão na RFC `docs/rfcs/0003-actors.md`.
