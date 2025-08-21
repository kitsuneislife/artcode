# Implementação: Finalizar Weak/Unowned (Contrato)

Objetivo
- Finalizar a implementação runtime de `Weak<T>` e `Unowned<T>` garantindo que:
  - Contadores strong/weak estejam consistentes.
  - Decrementos fortes ocorram em todos os caminhos (rebind, drop de env, substituição de campos mutáveis).
  - Weak/Unowned sejam invalidados de forma determinística quando o strong chegar a 0.

Contrato mínimo
- Inputs:
  - Código runtime atual (heap_objects, Environment, closures), testes novos.
- Outputs:
  - Implementação no runtime que passa testes unitários e de integração.
  - Exemplo `cli/examples/99_weak_unowned_demo.art` demonstrando comportamento.
  - Documentação em `docs/implementations/0001-finalize-weak-unowned.md`.

Casos de teste obrigatórios
1. Weak becomes None after last strong dropped.
2. Rebind doesn't double-decrement strong (rebind test).
3. Closure captured weak invalidated when target dropped.
4. Stress: many allocate/drop cycles mantêm contadores consistentes.

Plano de implementação (passos)
- Passo 1: Adicionar testes esqueléticos em `crates/interpreter/tests/weak_unowned.rs`.
- Passo 2: Inspecionar `heap_objects` e `Environment::define` e ajustar decrementos onde necessário.
- Passo 3: Implementar atomic ops na contagem se necessário (dependendo do modelo de threads).
- Passo 4: Adicionar runtime invalidation path para weak/unowned quando strong==0.
- Passo 5: Executar testes e corrigir regressões.

Critério de aceitação
- `cargo test -p interpreter` passa localmente.
- Exemplos `cli/examples/99_weak_unowned_demo.art` executam mostrando o comportamento esperado.

Riscos conhecidos
- Race conditions em multi-threaded envs (mitigar com atomics e testes).
- Falsos-positivos na análise estática que causam panics em tempo de debug.

Notas
- Este documento é um contrato de trabalho; durante a implementação, atualize-o com observações e decisões.
