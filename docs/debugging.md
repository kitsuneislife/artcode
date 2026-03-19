# Artcode Time-Travel Debugging (TTD)

O Artcode possui suporte nativo à gravação do histórico de execução através de um modelo eficiente de **Event Sourcing**, em oposição aos custosos snapshots totais da memória comummente encontrados em debuggers retrôs (Fase 1).

## Gravando um Traço (.artlog)

Scripts muitas vezes lidam com eventos de rede complexos e aleatoriedade. Para gravar o fluxo exato de execução e poder debugar falhas que "só acontecem uma vez na vida", execute o arquivo base com a flag de monitoramento `--record`.

```bash
art run --record my_bug_trace.artlog src/main.art
```

Sempre que a execução passar por fontes explícitas de não-determinismo globais que afetam fluxos puros lógicos no run-loop:
- `time_now()` (Sistema de Data)
- `rand_next()` (Geração Aleatória)

*(Outras primitivas como I/O de rede e mensagens vindas do Nexus/Actor Supervisor serão agregadas sequencialmente na Fase 2)*

O interpretador usará o pipeline de **Serialização IPC (Zero-copy)** para injetar deltas no arquivo `my_bug_trace.artlog` acompanhado do instante lógico (clock) e payload.

---
## Formato Interno: `.artlog`

Se você precisar varrer, ler, ou criar um programa que interprete as falhas do script manualmente, o arquivo final é puramente binário usando a macro nativa de Arrays do Artcode:

1. **Header (8 Bytes):** `ARTLOG01` (ASCII).
2. Sequência de serialização pura de instâncias de `Map` descrevendo:
  - `"type"`: Tag do nome de onde a interceptação ocorreu (e.g., `time_now`).
  - `"tick"`: Statements decorridos no código-fonte até este exato momento.
  - `"payload"`: O valor *retornado* do Evento.
