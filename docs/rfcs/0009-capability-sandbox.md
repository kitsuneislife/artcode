# RFC 0009 — Autoridade por Capability

- **Feature Name:** `capability_sandbox`
- **Start Date:** 2026-08-08
- **Status:** Proposto

## Resumo

Hoje qualquer código Artcode pode ler arquivos, abrir sockets e executar comandos de
shell. Não existe distinção entre o código que você escreveu e o código que veio de
`art add`. Esta RFC propõe que toda operação de efeito externo exija um **token de
capability inforjável**, concedido de fora do programa, e que módulos importados
recebam autoridade explícita em vez de herdarem a do processo.

O resultado é poder rodar um pacote de terceiro sabendo o que ele pode alcançar.

## Motivação

Artcode tem gerenciador de pacotes (`art add <git>`), e importar um módulo executa o
código de topo dele. Medido no estado atual:

```art
// evil.art, instalado como dependência
$ cmd /c echo ARBITRARY_SHELL_FROM_IMPORTED_MODULE;
```

```art
// main.art
println("[MAIN] antes do import");
import evil;
```

```
ARBITRARY_SHELL_FROM_IMPORTED_MODULE     ← roda antes da primeira linha do main
[MAIN] antes do import
```

Três lacunas produzem isso:

1. **`--pure` não é fronteira de segurança.** `docs/language/pure_mode.md` o define como
   modo de determinismo, e ele bloqueia `println`, `time_now` e `rand` justamente por
   isso. Não serve como "executar código não confiável": sem `println`, quase nada roda.
   É tudo-ou-nada, e o "nada" é inutilizável.

2. **Capabilities não conferem autoridade.** `capability_acquire("FileSystem")` pode ser
   chamado por qualquer código, inclusive pelo módulo importado. Nenhum builtin de IO
   consulta capability antes de agir — `io_read_text` verifica um booleano global e mais
   nada. O `examples/42_capability_tokens.art` afirma "enforcement no runtime"; não há.

3. **`.art-lock` não verifica integridade.** Registra `name`, `version`, `source`, `path`
   e `commit`, sendo `commit` opcional. Nenhum hash de conteúdo, nenhuma verificação na
   resolução seguinte.

A boa notícia é que o primitivo certo já existe: há `ArtValue::Capability`, com
move-semantics rastreado pelo type checker e `MovedCapability` para uso após consumo.
Falta ligá-lo à autoridade real.

## Design Detalhado

### Capabilities passam a ser inforjáveis

`capability_acquire` deixa de cunhar autoridade. Ele passa a **derivar** de uma
capability existente, sempre estreitando:

```art
// Só o escopo raiz recebe capabilities, e elas vêm do host.
func main(fs: Capability<FileSystem>) {
    let config_only = capability_narrow(fs, "./config");
    let content = io_read_text(config_only, "./config/app.toml");
}
```

Regras:

- `capability_narrow(cap, escopo)` devolve uma capability estritamente menor. Nunca
  amplia. Estreitar uma capability já movida é erro, pela mesma regra que hoje detecta
  reuso.
- Não existe forma de construir `Capability` a partir de string, número ou literal.
  A única origem é o host.
- Uma capability é consumida ao ser passada (move-semantics existente). Para usar duas
  vezes, derive duas.

### Builtins de IO exigem capability

A assinatura de toda operação de efeito externo ganha um primeiro parâmetro:

| Antes | Depois | Capability |
|---|---|---|
| `io_read_text(path)` | `io_read_text(cap, path)` | `FileSystem` com escopo de leitura |
| `io_write_text(path, s)` | `io_write_text(cap, path, s)` | `FileSystem` com escopo de escrita |
| `http_get_text(url)` | `http_get_text(cap, url)` | `Network` com escopo de host |
| `$ cmd args` | `$(cap) cmd args` | `Process` |

O runtime verifica, na chamada: a capability é do tipo certo, não foi movida, e o
caminho ou host solicitado está dentro do escopo dela. Falha vira `Result::Err`, não
panic — a linguagem já modela erro por `Result` no shell.

`println` **não** exige capability. Escrever em stdout não é autoridade que se queira
negar a um pacote, e tratá-lo como tal foi o que tornou `--pure` inutilizável.

### Módulos importados recebem autoridade explícita

Import não herda mais a autoridade do processo. O padrão é nenhuma:

```art
import json;                          // sem autoridade: puro cálculo
import telemetry with { net: metrics_endpoint };   // recebe uma capability nomeada
```

O código de topo de um módulo importado roda com o conjunto que a declaração de import
concede. Um módulo sem `with` que tente IO recebe `Err` — não é um erro de compilação,
porque a autoridade é dinâmica, mas é observável e testável.

### `--pure` vira caso particular

`art run --pure` passa a significar "conceder nenhuma capability". Deixa de bloquear
`println`, `time_now` e `rand`, que voltam a ser regidos apenas pelo determinismo do
modo de replay. A flag ganha companhia:

```bash
art run app.art                                    # sem capabilities (padrão novo)
art run --allow-fs=./data --allow-net=api.x.com app.art
art run --allow-all app.art                        # comportamento de hoje, explícito
```

**Mudança de padrão:** hoje `art run` concede tudo. Passa a conceder nada. É a quebra
mais visível desta RFC e a razão principal para ela existir.

### Integridade de pacote

`.art-lock` passa a registrar `sha256` do conteúdo resolvido. `art add` grava; a
resolução seguinte verifica e recusa divergência. `commit` deixa de ser opcional para
fontes git.

## Cenários de Interoperação

**Atores.** Uma capability é um `ArtValue` movido para o mailbox como qualquer outro. O
type checker já rastreia move para capabilities e já valida send-safety; capability
movida para um ator sai do escopo do remetente pela regra existente. Um ator sem
capability no corpo não faz IO.

**Arenas.** Capabilities são tokens pequenos e sem heap composto; não interagem com
`performant` nem com finalizers. A regra de escape de arena não muda.

**FFI.** `art_syscall_unsafe` continua fora deste modelo — é C-ABI, alcançável apenas
por quem já linka o runtime, não por código Artcode. A RFC 0008 permanece o documento
dessa fronteira. Vale registrar explicitamente que carregar uma biblioteca nativa é
conceder autoridade total.

**AOT/WASM.** No alvo WASM a autoridade já é dada pelo host (imports WASI). Este modelo
se alinha bem: `--allow-fs` mapeia para um preopen WASI.

## Alternativas

**Manter `--pure` e só documentar o risco.** Barato e honesto, mas deixa a linguagem sem
resposta para "posso rodar esse pacote?". Para o objetivo de viabilidade comercial, é
adiar o problema para quando houver usuários com dados reais.

**Sandbox por processo (rodar módulo importado em subprocesso isolado).** Isolamento mais
forte e independente da linguagem, mas custa um processo por módulo, complica a passagem
de valores e não funciona no alvo WASM. Não descarta capabilities: seria uma camada
extra sobre elas.

**Permissões declarativas em `Art.toml`** (estilo manifesto). Mais legível que passar
tokens, mas concede no nível do pacote e não da chamada, e não sobrevive a um módulo que
repassa autoridade a outro. Pode ser adicionado depois como açúcar que deriva as
capabilities do escopo raiz.

**Não fazer nada.** O estado atual é adequado para scripts pessoais e inadequado para
qualquer cenário com código de terceiro. Como `art add` já existe e o instalador
one-liner já é público, a exposição cresce com a adoção.

## Questões não resolvidas

- **Granularidade de escopo de `FileSystem`.** Prefixo de caminho é simples e suficiente
  para o começo. Glob e distinção leitura/escrita provavelmente são necessários; decidir
  se entram já ou depois.
- **Ergonomia.** Passar capability em toda chamada de IO é verboso. Um escopo implícito
  (`with cap { ... }`) reduziria ruído, mas torna a autoridade menos visível no ponto de
  uso. Precisa de exemplos reais antes de decidir.
- **Migração.** A mudança de padrão quebra todo script existente que faça IO. Provavelmente
  exige `--allow-all` como padrão em v0.6 com aviso, invertendo em v0.7. Definir a janela.
- **Capabilities no REPL.** Sessão interativa concede o quê? Provavelmente tudo, sendo o
  usuário o autor de cada linha — mas então `import` dentro do REPL precisa de regra.
- **Custo.** Verificar escopo em toda chamada de IO é irrelevante perto do custo da
  syscall. Confirmar com benchmark que a passagem de token não pesa no caminho quente.
