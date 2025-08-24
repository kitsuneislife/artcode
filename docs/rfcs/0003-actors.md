<!-- RFC: Runtime de Atores (MVP) -->
# RFC 0003 - Runtime de Atores (MVP)

Status: Draft

Authors: equipe Artcode

Date: 2025-08-24

## Sumário

Propor um runtime mínimo de atores que permita modelar concorrência segura por mensagem. O MVP foca em:

- Sintaxe: `spawn actor { ... }` para criar um ator com corpo léxico.
- Mailbox por ator (fila FIFO), API mínima para `actor_send(actor, value)` e `actor_receive()`.
- Scheduler cooperativo (modo inicial): execução de atores controlada pelo runtime do processo (não POSIX threads) para evitar problemas de Send/Sync com tipos não thread-safe.
- Backpressure configurável (limite por mailbox) e diagnosticável.

## Motivação

Concorrência baseada em atores simplifica isolamento de estado e evita data races. Em Artcode, o objetivo é permitir padrões pedagógicos e protótipos sem introduzir complexidade de memória compartilhada ou exigir que valores sejam Send/Sync.

## Proposta

1. Sintaxe

   - `spawn actor { ... }` — cria um ator com o corpo fornecido; retorna um handle (actor id) para o reator.

2. Runtime (MVP)

   - Implementar um scheduler cooperativo no processo principal que mantém atores como máquinas de estado (program counter + ambiente lexical) e executa fatias de trabalho em round-robin.
   - Mailbox: VecDeque(ArtValue) protegido internamente pelo scheduler (sem Mutex no MVP, execução single-threaded).
   - Builtins: `actor_send(actor_id, value)` enfileira mensagem; `actor_receive()` devolve a próxima mensagem (blocking sem busy-wait, mas cooperativo).
   - Backpressure: caixa configurável; quando cheia, `actor_send` retorna erro diagnóstico.

3. Tipos de Mensagem

   - MVP: mensagens transportam `ArtValue` (por cópia/clone dentro do runtime); não exigimos Send/Sync porque não haverá múltiplas threads.

4. Análise estática

   - Proibir captura de valores não Send-safe em contextos que permitiriam que eles sejam enviados entre atores (heurística conservadora para MVP).

## Alternativas

- Implementação baseada em threads OS (requere tipos Send/Sync) — rejeitada no MVP devido ao uso intensivo de `Rc` e outros tipos não-Send no runtime atual.
- Integração com async/await runtime — overkill para MVP.

## Impacto

- Runtime: novos módulos `crates/interpreter/src/actors.rs` e extensão de `Interpreter`.
- Tooling: testes novos em `crates/interpreter/tests/actors_*.rs`.
- Docs: `docs/concurrency.md` e exemplos em `cli/examples/concurrency/`.

## Plano incremental

1. Documentação + RFC (este documento).
2. Implementar builtins e scheduler cooperativo que executa atores via `Interpreter` per-actor state, sem threads.
3. Tests: happy path (send/receive), backpressure, mailbox ordering, graceful shutdown.
4. Iterar: permitir trabalho em background (threads) quando ArtValue estiver seguro para Send.

## Riscos

- Sem threads, atores podem monopolizar CPU se não cooperarem. Mitigação: limitar fatia por ator e exigir `yield` ou steps automáticos.
- Heurística de Send-safe pode ser conservadora, reduzindo expressividade; aceitável no MVP.

## Checklist de aceitação

- [ ] Parser: reconhecer `spawn actor { ... }`.
- [ ] Interpreter: criar actor handle e registrar programa.
- [ ] Builtins: `actor_send`, `actor_receive` funcionando no modo cooperativo.
- [ ] Scheduler: round-robin simples que executa atores até esgotarem (ou até limite de passos).
- [ ] Tests: send/receive, backpressure, mailbox FIFO.
- [ ] Docs e exemplo em `cli/examples/concurrency/`.
