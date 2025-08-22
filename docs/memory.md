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
* Heap agora unificado: apenas `heap_objects` armazena `HeapObject { value, strong, weak, alive }`. Arrays / Structs / Enums são sempre heapificados e representados no código por `ArtValue::HeapComposite(ObjHandle)`; resolução transparente em operações de pattern matching, type_of e field access.
* Introduzido `ObjHandle(u64)` e variantes `ArtValue::WeakRef/UnownedRef` usam esse handle. Builtins `weak(x)` e `unowned(x)` reutilizam o id se `x` já for composto heapificado (não duplicam objeto), ou criam wrapper heap para escalares (permitindo weak de escalares em debug).
* Decremento automático de strong em fim de escopo: cada `let` de valor composto registra handle no Environment; ao sair do bloco chamamos `drop_scope_heap_objects` que faz `dec_object_strong_recursive`, liberando em cascata. Rebind de variável executa decremento do valor anterior composto.
* Finalização: quando `strong` chega a 0 marcamos `alive=false`, incrementamos `objects_finalized`, executamos `on_finalize(obj, fn)` se registrado (função rodada em frame filho do global e promovendo variáveis criadas) e recursivamente decrementamos filhos (modelo determinístico simples sem GC segundo plano).
* `alive` reflete `strong > 0`; upgrade de weak consulta flag; unowned_get em modo debug emite diagnóstico quando alvo morto.

Política de finalização em duas fases (atual):

- Fase 1 — marcação e execução de finalizers: quando um objeto tem `strong` reduzido a 0, `alive` é marcado `false` e quaisquer finalizers associados são executados. A execução dos finalizers pode criar novos handles fortes; implementamos um frame filho temporário para executar finalizers e, em seguida, promovemos quaisquer handles fortes relevantes ao ambiente raiz.
- Fase 2 — limpeza/remoção: após a execução de finalizers e a propagação recursiva de decrementos, fazemos uma passagem de limpeza que remove do heap os objetos que estão `alive == false` e `weak == 0`. Objetos com `weak > 0` permanecem no heap até que os weak sejam removidos.

Notas sobre `weak` e `unowned`:

- `weak` produz um `WeakRef` que pode ser atualizado (upgrade) para um `Option<T>` via `weak_get`/`?`; upgrades falham (retornam `None`) se o alvo não estiver `alive`.
- `unowned` produz um `UnownedRef` que assume validade enquanto o dono existir; em modo debug `unowned_get` verifica `alive` e registra um diagnóstico `dangling unowned reference` se o alvo estiver morto, retornando `None`.
- Testes e helpers de depuração: para simular cenários de retenção e limpeza, a API de testes expõe helpers como `debug_heap_register`, `debug_heap_remove`, `debug_heap_inc_weak`, `debug_heap_dec_weak`, `debug_sweep_dead`, `debug_finalize_arena`, e `debug_heap_contains`.

Relatório de ciclos estendido:
* Campos agregados gerais: `heap_alive`, médias de graus de saída/entrada (`avg_out_degree`, `avg_in_degree`).
* Heurística ownership inicial: coleta `candidate_owner_edges` para arestas cujo nome de campo contém `parent` ou `owner` (destinadas a serem candidatas a edges fracas para quebra de ciclo ou confirmação de dominância). Estas arestas aparecem em `summary.candidate_owner_edges` e podem ser cruzadas com SCCs para priorização.

Próximos passos planejados (atualizados):
1. Refinar heurística ownership: incorporar frequência de uso e grau relativo (ex: se campo `parent` tem alto in-degree no alvo, confirmar padrão back-pointer).
2. Extensão de relatório por ciclo: distribuição de graus interna, classificação de cada aresta (provável-dominante, provável-back-pointer) e simulação de impacto de enfraquecimento.
3. Arenas em blocos `performant {}`: alocação bump + verificação de escape; integrar com handles (tag de arena nos bits altos do id) mantendo compatibilidade com weak/unowned.
4. Métricas futuras: `arena_alloc_count`, latência de detecção de ciclos, contagem de finalizers executados por tipo.
5. Time-travel snapshot antes de finalizer para debugging determinístico.

Decisões preliminares / atualizadas:
* `Weak` leitura retorna Option.
* `Unowned` assume validade; em debug checa e falha se inválido.
* Igualdade de Weak/Unowned: compara identidade do alvo vivo; alvo morto nunca iguala.
* Postfix `?` sempre produz `Optional<T>`; posterior inferência poderá propagar tipo interno.
* `ObjHandle` é opaco na linguagem; usuários não o manipulam diretamente; fornece ponto único para futuras estratégias (arenas / segregação / compressão de ponteiros).

Documento será expandido conforme itens forem marcados na checklist.

## Exemplos rápidos e helpers de teste

Exemplo: finalizer criando handles (pseudo-Artcode):

```artcode
// define finalizer que cria um handle 'saved' para o objeto owner
fn fin() {
	let saved = owner
}

on_finalize(target, fin)
// quando `target` cair para strong == 0, fin() é executado; `saved` é promovido ao root
```

Exemplo: arena (bloco performant) e finalizer promovendo referência externa:

```artcode
performant {
	let a = [1,2,3] // alocado na arena
	on_finalize(a, fn() { promoted = outside })
}
// ao sair do bloco a é finalizado; finalizer pode promover handles externos como `outside`
```

Regras práticas para `performant` (resumo)

- Objetivo: permitir alocações rápidas e temporárias sem overhead de contagem global a cada campo.
- Regras estáticas (checagem conservadora via `TypeInfer`):
	- `return` é proibido dentro de `performant` (sinaliza diagnóstico de tipo).
	- Declaração de `func` dentro de `performant` emite diagnóstico (fechamentos podem capturar valores de arena).
	- `let x = <inicializador composto>` dentro de `performant` emite diagnóstico conservador (pode escapar).

- Comportamento em tempo de execução (runtime checks):
	- Em builds de debug, retornos/lets que provoquem escape de objeto de arena podem causar `debug_assert`/panic para facilitar o diagnóstico.
	- Em builds normais, o runtime registra um `DiagnosticKind::Runtime` e continua (para não quebrar programas em produção durante o protótipo).

- Exemplo: padrão proibido (sintaxe Artcode):

```art
performant {
	let tmp = [1,2,3]
	return tmp // -> TypeInfer emite erro estático; runtime também checa
}
```

- Exemplo: função dentro de `performant` (proibido estaticamente):

```art
performant {
	func make() { return [1] } // função local: TypeInfer emite diagnóstico
}
```

- Padrão seguro / recomendado:

```art
performant {
	let local = [1,2,3]
	on_finalize(local, fn() { /* usar valor para atualizar métricas internas ou registrar evento */ })
	// não retornar nem atribuir `local` a variáveis fora do bloco
}
```

Promoção via finalizer (comportamento resumido)

- Quando um objeto de arena atinge `strong == 0`, o runtime marca `alive = false` e executa finalizers registrados via `on_finalize`.
- Finalizers são executados em um frame filho do root; quaisquer handles fortes criados nesse frame são promovidos ao root (para evitar que finalizer crie references que morram imediatamente).
- Métrica `finalizer_promotions` contabiliza quantos handles foram promovidos durante finalizers, e `finalizer_promotions_per_arena` quebra por arena.

Exemplo ilustrativo (pseudo-Artcode):

```art
performant {
	let a = [1,2]
	on_finalize(a, fn() {
		// cria um handle que deve sobreviver após finalização
		promoted = some_wrapper(a)
	})
}
// após saída do bloco, `a` foi finalizado, mas `promoted` foi promovido ao root pelo runtime
```

Helpers de teste disponíveis no `Interpreter` (usados nos testes do repositório):

- `debug_heap_register(val) -> id` : registra valor no heap e retorna id.
- `debug_heap_register_in_arena(val, arena_id) -> id` : registra valor em arena.
- `debug_define_global(name, val)` : define global para facilitar cenários de teste.
- `debug_heap_remove(id)` : simula remoção do último strong ref (dec strong).
- `debug_heap_inc_weak(id)` / `debug_heap_dec_weak(id)` : manipula contador weak para testes.
- `debug_run_finalizer(id)` : força execução do fluxo de finalização (dec_recursive + sweep).
- `debug_sweep_dead()` : varre e remove objetos mort os (`alive == false` && `weak == 0`).
- `debug_finalize_arena(arena_id)` : finaliza explicitamente uma arena (invoca `finalize_arena`).
- `debug_heap_contains(id)` : checa presença no heap.

Use esses helpers em testes para tornar o comportamento determinístico e evitar flakes dependentes de timing.
