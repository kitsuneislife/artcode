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

## Protótipo: `Atomic` e `Mutex` (heap-backed)

Status: protótipo implementado no runtime (single-threaded) e exposto via builtins.

Resumo rápido:
- As primitivas compartilhadas são representadas por handles heap-backed (`ArtValue::Atomic` e `ArtValue::Mutex`).
- Implementação atual é single-threaded; operações são _semânticas_ (não mapeiam diretamente para primitivos nativos de CPU).
- Builtins disponíveis (protótipo):
  - `atomic_new(value)` -> cria um `Atomic` com valor inicial.
  - `atomic_load(a)` -> retorna o valor atual.
  - `atomic_store(a, v)` -> substitui o valor, retorna `true` em sucesso.
  - `atomic_add(a, delta)` -> soma um inteiro e retorna o novo/antigo (convenção do runtime).
  - `mutex_new(value)` -> cria um `Mutex` contendo um valor inicial.
  - `mutex_lock(m)` / `mutex_unlock(m)` -> operações de bloqueio (protótipo cooperativo/no-op em single-threaded).

Exemplo de uso (Art):

```art
let a = atomic_new(0);
let old = atomic_add(a, 1);
let cur = atomic_load(a);

let m = mutex_new([1, 2, 3]);
mutex_lock(m);
// mutações seguras enquanto "possuímos" o lock no protótipo
mutex_unlock(m);
```

Observações e limitações:
- Atualmente as primitivas são um protótipo runtime: a semântica multithreaded formal (fences, ordering, atomicity real) ainda precisa ser definida.
- Os objetos são heapificados e referenciados por handles; finalizadores/arenas e o coletor atual tratam desses handles como objetos normais.

## Análise conservadora de Send-safety

O compilador/analizador tem uma verificação conservadora para evitar capturas/envios de valores potencialmente não-send-safe entre atores/threads.

Regras aplicadas (heurística conservadora):
- Em chamadas para `actor_send(...)`, `make_envelope(...)` e em `spawn actor { ... }` analisamos o payload/capturas; expressões simples (números, strings, nomes de variáveis locais primitivas, handles explícitos como `Atomic`/`Mutex`) são consideradas seguras.
- Expressões compostas, arrays/structs que contêm handles de objetos heap ou closures são consideradas não-send-safe e geram diagnóstico durante a fase de inference.

Comportamento do diagnóstico:
- O analisador emite diagnósticos informativos e impede (em fase de diagnóstico) padrões óbvios de envio de valores não-send-safe. A regra é propositalmente conservadora para evitar erros de tempo-de-execução.

Limitações e próximos passos (prioritizados):
1. Formalizar tipos Send/Sync no sistema de tipos e propagar propriedades pelos composites (arrays/structs) para reduzir falsos-positivos.
2. Definir semântica multithreaded das primitivas `Atomic`/`Mutex` (ordenações, fences, atomicidade) e, quando apropriado, mapear para primitivas nativas ou bibliotecas.
3. Representar `Atomic`/`Mutex` como kind explícito no `HeapObject` (em vez de snapshot de `StructInstance`) para reduzir complexidade e facilitar finalização/promotion.
4. Adicionar mais testes: misuse (double-unlock), tipos inválidos para `atomic_add`, interações com arenas e finalizadores, e testes de regressão de send-safety (spawn/make_envelope em vários cenários).

## Estado atual e como contribuir

- Protótipo implementado no crate `interpreter` com testes unitários em `crates/interpreter/tests/atomic_mutex.rs` e `atomic_mutex_send_safety.rs`.
- Checklist atualizado (`.kit/checklist.md`) marcando o protótipo; a análise rica de Send-safety ainda está pendente.
- Para contribuir: implementar formalização de tipos Send/Sync, mapear operações atômicas para primitivas reais e adicionar testes de integração multithreaded.

---
