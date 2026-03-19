# RFC 0002: Time-Travel Debugging (Event Sourcing)

## 1. Introdução

A versão `0.2.0` do Artcode planeja usar o runtime como *Artshell* e lançador de micro-processos (*Supervisor* do ArtOS). Nesses cenários, os scripts interagem pesadamente com sistema operativo, I/O e modelos de rede reativos. Isso traz grande complexidade ao debug.

O recurso **Time-Travel Debugging (TTD)** permite que um script no Artcode registre deterministicamente uma execução longa e instável em um arquivo (`.artlog` ou `.artd`) para posterior _replay_. No replay, o código fonte passeia pela execução real sem executar nenhum I/O pela segunda vez — permitindo inspecionar varíaveis e pular para pontos do passado ou futuro sem efeitos colaterais.

## 2. Arquitetura Proposta: Event Sourcing & Deltas

Tentar tirar um "snapshot global da memória" (como faz a VM Smalltalk ou emuladores clássicos de console) a cada instrução é proibitivo em performance. Em vez disso, a arquitetura do Artcode será baseada no paradigma de **Event Sourcing** gravando apenas fontes de Não-Determinismo nos delimitadores lógicos.

### 2.1. O que é gravado (Fontes Não-Determinísticas)
A máquina virtual Artcode é estritamente determinística, exceto pelas pontes FFI com o sistema host:
1. Valores vindos de `time_now()`.
2. Saídas e Seeds de algoritmos pseudo-aleatórios embutidos (ex: `rand_next()`).
3. Retornos de I/O de rede e arquivos (`http_get_text()`, leituras HTTP FFI).
4. Mailbox Messages de e para atores externos.
5. Operações multithread de `Atomic` (registradores concorrentes).

Todo evento desse tipo chamará um gancho `Tracer::record_event(tipo, payload binário)` quando rodar no modo `--record`, gerando entradas no log delta sequencial.

## 3. Formato do Arquivo de Traço (`.artlog`)

Aproveitando o buffer recém introduzido por nossa feature "Serialização Zero-copy (IPC)":

O arquivo de log será um fluxo contínuo de Buffers Serializados usando exatamente as regras e tags do serializador primitivo recém escrito no passo anterior do Artcode (`interpreter.rs` -> `encode_val`).

- `[Magic Header]` = `A R T L O G 0 1` -> 8 bytes
- `[Eventos Binários Consecutivos]` 

Onde cada evento é um Struct Tuple gravado através da API de buffer, contendo `(Tipo de Evento, Tick Lógico, Retorno ou Argumento Interceptado)`.

## 4. O Fluxo de Debug (Modo `--replay`)

No CLI:
`art debug --replay trace.artlog`

O runtime entra em modo `Replay`:
- Funções como `time_now()` e chamadas de request do mundo exterior perdem sua ligação com as dependências do Host OS. Elas devolvem exatamente o próximo evento binário equivalente extraído de `.artlog`, mantendo a linha do tempo do Artcode consistente sem alterar a performance base.

## 5. Próximos Passos
Esta é a **Fase 1**.
A Fase 1 envolve somente a criação desse mecanismo de Tracking e a flag `--record trace.artlog` emitindo arquivos funcionais, com `time_now` e `rand` determinísticos. Modos de replay de terminal ou Atores, e Keyframes snapshots de memória inteira não serão introduzidos neste primeiro momento.
