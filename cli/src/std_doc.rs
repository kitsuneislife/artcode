pub fn print_std_docs() {
    println!("Artcode Standard Library Documentation\n====================================");
    println!("A StdLib básica do Artcode está embutida no engine global via Builtin Functions.\n");

    let docs = vec![
        ("println(value: Any)", "Imprime o valor convertido em string no output padrão."),
        ("len(collection: Array|Map|Set|String)", "Retorna o tamanho ou número de elementos do iterável."),
        ("type_of(value: Any)", "Retorna uma String com o nome representativo do tipo armazenado na variável."),
        ("", ""),
        ("--- GC & Memory Controls ---", ""),
        ("__weak(obj: HeapObject)", "Cria uma referência Weak não rasteável para coletor (Arena/ARC)."),
        ("__weak_get(weak: WeakRef)", "Tenta re-promover a referência Weak para Strong, se retornar Optional::none o objeto já foi descartado."),
        ("__unowned(obj: HeapObject)", "Cria um pointeiro não seguro (`unowned`) ignorado pelo GC. Fast-path manual."),
        ("__unowned_get(unowned: UnownedRef)", "Desreferencia perigosamente o unowned ref. Útil apenas durante escopos atômicos e hot-loops de Arenas."),
        ("__on_finalize(obj: HeapObject, callback: Fn)", "Anexa um destrutor ao objeto para rodar callbacks automáticos de limpeza na desassociação de memória."),
        ("", ""),
        ("--- Actor Runtime & Concurrency ---", ""),
        ("actor_send(actor: ActorRef, value: Any)", "Roteia a mensagem no Mailbox FIFO/Queue Lock-free do ator receptor."),
        ("actor_receive()", "Bloqueia (park) a corrotina e desempilha a próxima mensagem. Caso não haja mensagem retorna Option nulo."),
        ("actor_receive_envelope()", "Retorna um envelope iterável com os headers `sender` e a `payload` da mensagem desempilhada."),
        ("actor_yield()", "Cede o tempo de processamento agendado nativo imediatamente pra próxima thread do Event Loop."),
        ("actor_set_mailbox_limit(actor: ActorRef, limit: Int)", "Impõe Backpressure rejeitando mensagens sob pressão no Event Loop."),
        ("run_actors([max_steps: Int])", "Dispara e esvazia agressivamente todos os Agendadores/Atores num lock-step progressivo."),
        ("", ""),
        ("--- Concurrency Primitives ---", ""),
        ("atomic_new(initial: Int)", "Aloca primitivo de Atômico 64 para transições shared-memory-cross-actors."),
        ("atomic_load(atomic: AtomicRef)", "Leitura imutável SeqCst do primitivo."),
        ("atomic_store(atomic: AtomicRef, value: Int)", "Escrita concorrente do primitivo."),
        ("atomic_add(atomic: AtomicRef, delta: Int)", "Operação matemática atômica sem locks retornando o resultado mutado."),
        ("mutex_new(value: Any)", "Tranca exclusiva Rust level para exclusão mútua trans-atores de objetos profundos usando Spin/Wait logic."),
        ("mutex_lock(mutex: MutexRef)", "Obtém uma referência internal ou pára (Park) a thread atual no Event-Loop aguardando destrancamento."),
        ("mutex_unlock(mutex: MutexRef)", "Desbloqueia e agenda a próxima Thread esperando a exclusão mútua."),
    ];

    for (sig, desc) in docs {
        if sig.starts_with("---") {
            println!("\n{}\n", sig);
        } else if sig.is_empty() {
            continue;
        } else {
            println!("* `{}`\n  > {}", sig, desc);
        }
    }
}
