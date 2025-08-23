## Modelo de Memória (Fase 8 – Draft Inicial)

Escada de abstração (alvo completo):
1. ARC simples (default)
2. weak/unowned explícitos para quebrar ciclos
3. Arenas em blocos `performant {}` com análise de lifetime

Progresso desta fase (parcial / atualizado):
* Closures agora armazenam `Weak<RefCell<Environment>>` evitando ciclos `Environment -> Function -> Environment`.
* Funções normais: `closure = Weak`; ambiente vive via cadeia de ambientes (stack léxico).
* Métodos "bound": adicionamos `retained_env` (strong) para manter o ambiente sintético vivo; `closure` continua Weak.
* Chamada com ambiente já coletado emite diagnóstico `Dangling closure environment` e executa em ambiente vazio (provisório para debug).
* Açúcar sintático: `weak expr`, `unowned expr`, postfix `?` (upgrade opcional de weak) e `!` (acesso unowned) mapeiam para builtins internos.
* Detector de ciclos protótipo: agora opera sobre ids reais de heap (`HeapComposite`) construindo grafo de objetos vivos; algoritmo Tarjan SCC. Classifica `isolated` (sem incoming externo), `reachable_from_root` e marca `leak_candidate = isolated && !reachable_from_root`.
* Sugestões: arestas internas iniciais + ranking (score = out_deg(from)+in_deg(to)) top 3.
* Métricas em runtime atuais: `weak_created`, `weak_upgrades`, `weak_dangling`, `unowned_created`, `unowned_dangling`, `cycle_reports_run`, `cycle_leaks_detected`, `strong_increments`, `strong_decrements`, `objects_finalized`, `heap_alive`, `avg_out_degree`, `avg_in_degree`.

### Métricas por arena

O runtime agora expõe métricas por arena quando blocos `performant {}` são usados. Elas aparecem na saída do CLI (`art metrics --json`) e no relatório compacto. Principais chaves:

- `arena_alloc_count: { arena_id: alloc_count }` — número de alocações registradas na arena identificada por `arena_id`.
- `objects_finalized_per_arena: { arena_id: finalized_count }` — quantos objetos daquela arena tiveram seus finalizers executados e foram contabilizados como finalizados.
- `finalizer_promotions_per_arena: { arena_id: promotions }` — quantos handles foram promovidos ao root porque um finalizer daquela arena criou referências sobreviventes.

Notas de interpretação e limites:

- `arena_id` é um identificador interno (u32) estável durante a execução do programa. Não há garantia sobre densidade ou ordenação entre ids (pode haver gaps).
- Valores são contadores não-negativos (usize) e podem ser zero quando a execução não usou arenas.
- As métricas por arena são úteis para localizar hotspots temporários e para validar que finalizers não estão promovendo referências inesperadas para o heap global.
## Modelo de Memória (Fase 8 — Documentação consolidada)

Esta página descreve o modelo de memória projetado e implementado na Fase 8. Ela resume a semântica de ownership, layout do heap, política de finalização, arenas (`performant {}`), métricas expostas pelo runtime e ferramentas de debug/test.

## Visão geral

- Modelo principal: ARC (contagem automática de referências) com contadores separados para strong e weak.
- Tipos de referência adicionais:
	- Weak: ponteiro fraco que pode ser atualizado (upgrade opcional para Option<T>).
	- Unowned: referência não-owning que assume validade enquanto o dono existir; em modo debug valida `alive` e registra diagnóstico se alvo morto.
- Finalizers: funções associadas a objetos heap que são executadas quando o objeto perde o último strong.
- Arenas: blocos lexicais `performant {}` que permitem alocações temporárias com contabilização e finalização por arena.

## Objetos e layout do heap

- Todo valor composto (arrays, structs, enums, closures, objetos wrapper) é heapificado e representado por um `ObjHandle` (internamente u64/u32).
- Estrutura mínima por objeto (`HeapObject`):
	- value: os dados/variant
	- strong: contador de referências fortes (usize)
	- weak: contador de referências weak (usize)
	- alive: booleano derivado (`strong > 0` durante execução)
	- arena_id: Option<u32> — se o objeto foi alocado/atribuído a uma arena

Observação: a implementação atual usa um único mapa `heap_objects` contendo todos os objetos; o `arena_id` é uma etiqueta lógica usada para finalização e métricas.

## Semântica de Weak e Unowned

- Weak:
	- `weak(x)` cria um wrapper fraco ao redor de `x` (ou reusa o id se `x` já for composto).
	- `upgrade(weak)` / postfix `?` tenta retornar `Some(handle)` se o alvo estiver `alive`, senão `None`.
	- Weak não afeta a vida do objeto (não incrementa `strong`).

- Unowned:
	- `unowned(x)` cria uma referência que pressupõe validade enquanto o dono existir.
	- Em modo debug, `unowned_get` valida `alive` e emite diagnóstico `dangling unowned reference` quando o alvo já foi finalizado.
	- Em produção, `unowned` é mais permissivo (sem checagens) para reduzir overhead, mas é categorizado como comportamento inseguro se usado incorretamente.

## Ciclos e detector de ciclos

- A runtime expõe um detector de ciclos orientado a testes que opera sobre o grafo de `heap_objects` (usa Tarjan SCC).
- Cada SCC é classificada com metadados: `isolated`, `reachable_from_root`, `leak_candidate`.
- O detector gera sugestões de arestas a enfraquecer (candidate_owner_edges) e um ranking simples para ajudar a triagem manual.

## Política de finalização (dois passos)

1) Execução de finalizers
	- Quando `strong` chega a 0 um objeto é marcado `alive=false` e qualquer finalizer associado é executado imediatamente.
	- Finalizers são executados num frame filho temporário. Isso permite que o finalizer crie handles locais fortes; o runtime irá identificar e promover (se necessário) handles que devam sobreviver após finalização.

2) Sweep / remoção
	- Após execução dos finalizers e decréscimos recursivos de strong nos objetos referenciados, o runtime executa uma passagem de limpeza que remove objetos com `alive == false && weak == 0`.
	- Objetos com `weak > 0` permanecem até que os weak sejam liberados.

Observações de estabilidade e robustez:
- A ordem de execução dos finalizers é determinística por id (ordenamento estável). Implementações críticas podem precisar de uma ordem baseada em grafo — isso é roadmap futuro.
- Pode ser feita uma multi-pass sweep para estabilizar efeitos de finalizers que desencadeiam novas finalizações.

## Arenas e blocos `performant {}`

- Objetivo: permitir alocações temporárias de baixa latência controlando os custos de contagem global.
- Semântica:
	- Alocações dentro de `performant {}` são marcadas com `arena_id` e contabilizadas em `arena_alloc_count`.
	- Ao sair do bloco, `finalize_arena(arena_id)` é invocado: decresce fortes, executa finalizers pertinentes e realiza sweep local/determinístico.
	- Finalizers de objetos de arena podem promover handles para o root; tais promoções são atribuídas à arena via `finalizer_promotions_per_arena`.

- Regras estáticas (checagem conservadora):
	- `return` dentro de `performant` é proibido (TypeInfer sinaliza).
	- Funções definidas dentro de `performant` são desaconselhadas/diagnosticadas (podem capturar arena values).
	- `let` com inicializador composto dentro de `performant` é sinalizado quando há risco de escape.

## Métricas expostas

O runtime expõe métricas voltadas para diagnóstico e telemetria. As chaves principais disponíveis via `art metrics --json`:

- Globais:
	- `handled_errors`: número de erros manejados durante execução
	- `executed_statements`: número de statements executados
	- `crash_free`: proporção em percentagem dos statements sem crash
	- `finalizer_promotions`: total de handles promovidos durante execução de finalizers
	- `weak_created`, `weak_upgrades`, `weak_dangling`
	- `unowned_created`, `unowned_dangling`
	- `cycle_reports_run` — contagem de detecções de ciclos em execução de debug

- Por-arena (quando arenas foram usadas):
	- `arena_alloc_count: { arena_id: alloc_count }`
	- `objects_finalized_per_arena: { arena_id: finalized_count }`
	- `finalizer_promotions_per_arena: { arena_id: promotions }`

Interpretação rápida:
- `arena_alloc_count` ajuda a localizar blocos com muitas alocações temporárias.
- `objects_finalized_per_arena` mostra quantos objetos de cada arena executaram finalizers (ajuda a diagnosticar trabalho pós-escopo).
- `finalizer_promotions_per_arena` identifica finalizers que promovem referências para o heap global (frequentemente um anti-pattern para temporários).

Boas práticas de telemetria:
- Em CI, alerte quando `finalizer_promotions_per_arena` exceder um limiar relativo (ex.: > 5% das alocações da arena).
- Correlacione `arena_alloc_count` com latência/throughput para detectar hotspots.

## Helpers de debug e API de testes

APIs destinadas a testes e diagnósticos (visíveis nos helpers do `Interpreter`):

- `debug_heap_register(val) -> id` — registra valor no heap e retorna id.
- `debug_heap_register_in_arena(val, arena_id) -> id` — registra valor com `arena_id`.
- `debug_define_global(name, val)` — define um global para fins de teste.
- `debug_heap_remove(id)` — simula remoção do último strong ref (invoca dec strong).
- `debug_heap_inc_weak(id)` / `debug_heap_dec_weak(id)` — manipulam contador weak para testes.
- `debug_run_finalizer(id)` — força execução do fluxo de finalização (dec_recursive + sweep).
- `debug_sweep_dead()` — varre e remove objetos mortos (`alive == false && weak == 0`).
- `debug_finalize_arena(arena_id)` — invoca explicitamente a finalização de uma arena.
- `debug_heap_contains(id)` — checa presença no heap.

Use estes helpers quando precisar tornar cenários determinísticos em testes de unidade/integrção.

### Centralização das mutações de contadores (implementação)

Nota de implementação: para tornar a adaptação do Arc interno consistente e evitar duplicação
de escrita nos campos `strong`/`weak`, as mutações diretas de `HeapObject` foram centralizadas
em `crates/interpreter/src/heap_utils.rs`.

- As funções exportadas em `heap_utils` aceitam apenas `&mut HeapObject` e realizam a mutação
	sobre o objeto: `inc_strong_obj`, `dec_strong_obj -> bool`, `inc_weak_obj`, `dec_weak_obj -> bool`,
	`force_strong_to_one_obj`.
- Contrato: os helpers NÃO atualizam métricas do `Interpreter` porque muitas chamadas ocorrem
	quando já há um borrow mutável ao mapa de heap; atualizar métricas dali causaria conflitos
	de borrowing. O `Interpreter` permanece responsável por incrementar `strong_increments`,
	`strong_decrements` e outras métricas quando apropriado, usando o valor retornado pelos
	helpers (`bool`) para decidir se um decrement foi efetivo.

Essa separação mantém os pontos de mutação auditáveis (um arquivo) e evita erros do
borrow-checker do compilador ao mesmo tempo que preserva a telemetria no nível do runtime.

## Exemplos práticos

Exemplo: finalizer que cria um handle promovido (pseudo-Artcode)

```art
let outside = null
{
	let target = [1,2,3]
	on_finalize(target, fn() { outside = copy(target) })
}
// após o bloco, target foi finalizado; `outside` foi promovido pelo runtime se o finalizer criou um handle forte
```

Exemplo: uso de arena e finalizer (pseudo-Artcode)

```art
performant {
	let a = [1,2,3] // alocado na arena 1
	on_finalize(a, fn() { promoted = wrap(a) })
}
// `a` finalizado; if `promoted` foi criado, será promovido ao root e atribuído à arena na métrica de promoção
```

## Recomendações e trade-offs

- Segurança vs. performance: `unowned` oferece performance com risco de dangling; recomenda-se uso com precaução e testes em modo debug.
- Finalizers poderosos: permitem cleanup, mas tráfego de promoção pode transformar temporários em permanentes; monitore `finalizer_promotions_per_arena`.
- Ordenação de finalizers: a estratégia atual é determinística por id; se houver requisitos semânticos mais fortes considere implementar ordenação por grafo ou topológica.

## Limitações e próximos passos

- Ordenação de finalizers baseada em dependências (em vez de id) — tópico para Fase seguinte.
- Otimizações de arenas: bump allocation para reduzir overhead, e integração com planos AOT/JIT para reduzir custos de wrapper.
- Validação mais forte em tempo de compilação para evitar padrões que escapem arenas.

---

Este documento será atualizado conforme a implementação evolui; abra RFCs para mudanças estruturais que alterem invariantes de runtime.
