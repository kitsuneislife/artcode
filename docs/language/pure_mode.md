# Modo Pure

O modo pure existe para cenários de configuração e validação determinística, como o
fluxo do Supervisor. Ele elimina efeitos colaterais e fontes de não-determinismo para
que a avaliação de um script seja reprodutível.

## Como usar

```bash
art run --pure arquivo.art
```

## Operações bloqueadas

Com `--pure` ativo, o runtime recusa a operação e emite erro de runtime:

| Operação | Motivo |
|---|---|
| `println` | efeito colateral observável |
| `io_read_text` | leitura de arquivo |
| `io_write_text` | escrita de arquivo |
| `http_get_text` | acesso à rede |
| `$ comando` (shell) | execução de processo |
| `time_now` | não-determinismo |
| `rand_seed` | não-determinismo |
| `rand_next` | não-determinismo |

```art
try {
    io_write_text("/tmp/out.txt", "x");
} catch err {
    // em --pure, err recebe a mensagem de bloqueio
}
```

## O que o modo pure não é

**Não é uma fronteira de segurança.** Ele foi desenhado para determinismo, e a
diferença importa:

- É binário. Ou tudo é permitido, ou nada — inclusive `println`, o que torna o modo
  inadequado para "executar código de terceiro com segurança".
- Não distingue origem. Um módulo importado roda com a mesma autoridade do programa
  principal, e o código de topo de um módulo executa no `import`.
- Fora de `--pure`, o padrão é acesso total: arquivos, rede e shell liberados.

Hoje, portanto, **Artcode não isola código de terceiro**. Um pacote instalado via
`art add` executa com toda a autoridade do processo. Trate dependências com o mesmo
cuidado que teria ao executar um script baixado.

A [RFC 0009](../rfcs/0009-capability-sandbox.md) propõe substituir esse modelo por
autoridade baseada em capabilities, em que `--pure` passa a ser o caso particular de
"nenhuma capability concedida".
