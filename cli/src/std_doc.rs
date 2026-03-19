use interpreter::interpreter::Interpreter;

struct StdDocMeta {
    category: &'static str,
    signature: &'static str,
    description: &'static str,
}

fn std_doc_meta(name: &str) -> Option<StdDocMeta> {
    match name {
        "println" => Some(StdDocMeta {
            category: "Core",
            signature: "println(value: Any)",
            description: "Imprime o valor convertido em string no output padrao.",
        }),
        "len" => Some(StdDocMeta {
            category: "Core",
            signature: "len(collection: Array|Map|Set|String)",
            description: "Retorna o tamanho ou numero de elementos do iteravel.",
        }),
        "type_of" => Some(StdDocMeta {
            category: "Core",
            signature: "type_of(value: Any)",
            description: "Retorna uma String com o nome representativo do tipo em runtime.",
        }),
        "weak" => Some(StdDocMeta {
            category: "Memory",
            signature: "weak(obj: HeapObject)",
            description: "Cria uma referencia fraca para objeto heap sem manter ownership forte.",
        }),
        "weak_get" => Some(StdDocMeta {
            category: "Memory",
            signature: "weak_get(ref: WeakRef)",
            description: "Tenta promover uma referencia fraca; retorna Optional::none se expirada.",
        }),
        "unowned" => Some(StdDocMeta {
            category: "Memory",
            signature: "unowned(obj: HeapObject)",
            description: "Cria referencia unowned para usos controlados de alta performance.",
        }),
        "unowned_get" => Some(StdDocMeta {
            category: "Memory",
            signature: "unowned_get(ref: UnownedRef)",
            description: "Acessa referencia unowned; pode falhar se objeto nao estiver mais vivo.",
        }),
        "on_finalize" => Some(StdDocMeta {
            category: "Memory",
            signature: "on_finalize(obj: HeapObject, callback: Fn)",
            description: "Registra callback para finalizacao do objeto.",
        }),
        "actor_send" => Some(StdDocMeta {
            category: "Actors",
            signature: "actor_send(actor: ActorRef, value: Any[, priority: Int])",
            description: "Envia mensagem para mailbox de um ator.",
        }),
        "actor_receive" => Some(StdDocMeta {
            category: "Actors",
            signature: "actor_receive()",
            description: "Consome proxima mensagem da mailbox do ator atual.",
        }),
        "actor_receive_envelope" => Some(StdDocMeta {
            category: "Actors",
            signature: "actor_receive_envelope()",
            description: "Consome e retorna envelope com sender, payload e prioridade.",
        }),
        "actor_yield" => Some(StdDocMeta {
            category: "Actors",
            signature: "actor_yield()",
            description: "Sinaliza cooperacao para o scheduler no modelo de atores.",
        }),
        "actor_set_mailbox_limit" => Some(StdDocMeta {
            category: "Actors",
            signature: "actor_set_mailbox_limit(actor: ActorRef, limit: Int)",
            description: "Define limite de mailbox para backpressure.",
        }),
        "envelope" => Some(StdDocMeta {
            category: "Actors",
            signature: "envelope(sender: Int, payload: Any, priority: Int)",
            description: "Cria envelope manual para mensagens de ator.",
        }),
        "make_envelope" => Some(StdDocMeta {
            category: "Actors",
            signature: "make_envelope(payload: Any[, priority: Int])",
            description: "Cria envelope preenchendo sender automaticamente no contexto atual.",
        }),
        "run_actors" => Some(StdDocMeta {
            category: "Actors",
            signature: "run_actors([max_steps: Int])",
            description: "Executa o scheduler de atores ate ociosidade ou limite de passos.",
        }),
        "atomic_new" => Some(StdDocMeta {
            category: "Concurrency",
            signature: "atomic_new(initial: Int)",
            description: "Cria valor atomico em heap.",
        }),
        "atomic_load" => Some(StdDocMeta {
            category: "Concurrency",
            signature: "atomic_load(atomic: AtomicRef)",
            description: "Le o valor atual do atomico.",
        }),
        "atomic_store" => Some(StdDocMeta {
            category: "Concurrency",
            signature: "atomic_store(atomic: AtomicRef, value: Int)",
            description: "Escreve novo valor em atomico.",
        }),
        "atomic_add" => Some(StdDocMeta {
            category: "Concurrency",
            signature: "atomic_add(atomic: AtomicRef, delta: Int)",
            description: "Soma delta no atomico e retorna novo valor.",
        }),
        "mutex_new" => Some(StdDocMeta {
            category: "Concurrency",
            signature: "mutex_new(value: Any)",
            description: "Cria mutex heap-backed para exclusao mutua.",
        }),
        "mutex_lock" => Some(StdDocMeta {
            category: "Concurrency",
            signature: "mutex_lock(mutex: MutexRef)",
            description: "Trava o mutex e retorna status de sucesso.",
        }),
        "mutex_unlock" => Some(StdDocMeta {
            category: "Concurrency",
            signature: "mutex_unlock(mutex: MutexRef)",
            description: "Destrava mutex e retorna status de sucesso.",
        }),
        "map_new" => Some(StdDocMeta {
            category: "Collections",
            signature: "map_new()",
            description: "Cria mapa vazio.",
        }),
        "map_set" => Some(StdDocMeta {
            category: "Collections",
            signature: "map_set(map: Map, key: String, value: Any)",
            description: "Define chave no mapa.",
        }),
        "map_get" => Some(StdDocMeta {
            category: "Collections",
            signature: "map_get(map: Map, key: String)",
            description: "Busca valor por chave e retorna Optional.",
        }),
        "map_has" => Some(StdDocMeta {
            category: "Collections",
            signature: "map_has(map: Map, key: String)",
            description: "Verifica existencia de chave no mapa.",
        }),
        "set_new" => Some(StdDocMeta {
            category: "Collections",
            signature: "set_new()",
            description: "Cria conjunto vazio.",
        }),
        "set_add" => Some(StdDocMeta {
            category: "Collections",
            signature: "set_add(set: Set, value: Any)",
            description: "Insere valor no conjunto.",
        }),
        "set_has" => Some(StdDocMeta {
            category: "Collections",
            signature: "set_has(set: Set, value: Any)",
            description: "Verifica existencia de valor no conjunto.",
        }),
        "math_abs" => Some(StdDocMeta {
            category: "Math",
            signature: "math_abs(value: Int|Float)",
            description: "Retorna valor absoluto.",
        }),
        "math_pow" => Some(StdDocMeta {
            category: "Math",
            signature: "math_pow(base: Number, exp: Number)",
            description: "Potenciacao numerica.",
        }),
        "math_clamp" => Some(StdDocMeta {
            category: "Math",
            signature: "math_clamp(value: Number, min: Number, max: Number)",
            description: "Limita valor no intervalo [min, max].",
        }),
        "dag_topo_sort" => Some(StdDocMeta {
            category: "Algorithms",
            signature: "dag_topo_sort(nodes: Array, deps: Array)",
            description:
                "Retorna ordenacao topologica; em ciclos retorna Optional::none e diagnostico.",
        }),
        "time_now" => Some(StdDocMeta {
            category: "IO & Time",
            signature: "time_now()",
            description: "Retorna timestamp atual do sistema.",
        }),
        "io_read_text" => Some(StdDocMeta {
            category: "IO & Time",
            signature: "io_read_text(path: String)",
            description: "Le arquivo texto e retorna conteudo.",
        }),
        "io_write_text" => Some(StdDocMeta {
            category: "IO & Time",
            signature: "io_write_text(path: String, content: String)",
            description: "Escreve conteudo texto em arquivo.",
        }),
        "http_get_text" => Some(StdDocMeta {
            category: "IO & Time",
            signature: "http_get_text(url: String)",
            description: "Faz GET HTTP basico e retorna corpo texto (suporta apenas http://).",
        }),
        "rand_seed" => Some(StdDocMeta {
            category: "Random",
            signature: "rand_seed(seed: Int)",
            description: "Inicializa gerador pseudoaleatorio deterministico.",
        }),
        "rand_next" => Some(StdDocMeta {
            category: "Random",
            signature: "rand_next()",
            description: "Retorna proximo valor pseudoaleatorio.",
        }),
        _ => None,
    }
}

fn fallback_doc(name: &str) -> StdDocMeta {
    StdDocMeta {
        category: "Misc",
        signature: Box::leak(format!("{}(...)", name).into_boxed_str()),
        description: "Sem metadata detalhada ainda. Builtin disponivel no prelude.",
    }
}

pub fn print_std_docs() {
    let builtins = Interpreter::prelude_builtin_bindings();

    println!("Artcode Standard Library Documentation");
    println!("====================================");
    println!(
        "Gerado automaticamente a partir do registro de builtins do prelude ({} itens).\n",
        builtins.len()
    );

    let mut current_category = "";
    for (name, _) in builtins {
        let meta = std_doc_meta(name).unwrap_or_else(|| fallback_doc(name));
        if current_category != meta.category {
            current_category = meta.category;
            println!("\n--- {} ---\n", current_category);
        }
        println!("* `{}`\n  > {}", meta.signature, meta.description);
    }
}
