<!-- RFC: Runtime de Atores (MVP) -->
# RFC 0003 - Runtime de Atores (MVP)

Status: Accepted (MVP implemented)

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

1. Implementação MVP concluída: builtins e scheduler cooperativo implementados no `Interpreter`.
2. Tests: `crates/interpreter/tests/actors_mvp.rs` e `crates/interpreter/tests/actors_stress.rs` adicionados e passando localmente.
3. Próximos: avaliar heurística Send-safe e projetar primitivas compartilhadas para blocos `performant`.

## Riscos

- Sem threads, atores podem monopolizar CPU se não cooperarem. Mitigação: limitar fatia por ator e exigir `yield` ou steps automáticos.
- Heurística de Send-safe pode ser conservadora, reduzindo expressividade; aceitável no MVP.

## Checklist de aceitação (status)

- [x] Parser: reconhecer `spawn actor { ... }`.
- [x] Interpreter: criar actor handle e registrar programa.
- [x] Builtins: `actor_send`, `actor_receive` funcionando no modo cooperativo.
- [x] Scheduler: round-robin simples que executa atores até esgotarem (ou até limite de passos).
- [x] Tests: send/receive, backpressure, mailbox FIFO (tests added).
- [ ] Docs e exemplo em `cli/examples/concurrency/` (docs present but examples directory pending).

Observação: a opção de multi-threading (swapping para threads OS) foi deliberadamente adiada até que tenhamos uma análise robusta de Send-safe e primitivas sincronizadas para blocos `performant`.
