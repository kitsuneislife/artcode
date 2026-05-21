# Modo Pure

O modo pure foi criado para cenarios de configuracao e validacao deterministica, como o fluxo do Supervisor.

## Como usar

```bash
art run --pure arquivo.art
```

## Regras atuais do modo pure

Quando `--pure` esta ativo, o runtime bloqueia operacoes impuras e emite erro de runtime:

- `println`
- `io_read_text`
- `io_write_text`
- `time_now`
- `rand_seed`
- `rand_next`

## Objetivo

Evitar efeitos colaterais e fontes de nao-determinismo durante a avaliacao de scripts que devem ser estritamente declarativos.

## Exemplo rapido

```art
try {
    io_write_text("/tmp/out.txt", "x");
} catch err {
    // em --pure, err recebe a mensagem de bloqueio
}
```
