# Contribuindo com o Artcode

Bem-vindo(a) ao repositório da linguagem **Artcode**. É um projeto jovem, construído
sobre complexidade progressiva a partir das premissas de Atores e Arena GC, e todo
auxílio no parser, no interpretador, na CLI e na compilação IR/AOT é valioso.

Este é o documento canônico de contribuição. Ele cobre o processo, o portão de
qualidade e as convenções.

## Princípios

1. Clareza antes de abstração
2. Evolução incremental e testada
3. Determinismo e previsibilidade

## Como contribuir

### Bugs e mudanças pequenas

Para bugs, panics, problemas de performance, vazamentos de memória ou sugestões
curtas de funcionalidade:

1. Procure uma issue existente no GitHub.
2. Abra uma nova issue se não houver.
3. Após a triagem, abra um Pull Request.

Issues recebem triagem automática via GitHub Actions nas categorias:

- `lang-design` — linguagem, parser/lexer, sintaxe, semântica, tipagem, RFC
- `runtime` — interpretador, memória (ARC), performance, JIT/AOT, FFI
- `tooling` — CLI, LSP, formatter/linter, CI e documentação

### Mudanças estruturais: processo RFC

Mudanças profundas em linguagem, runtime, arquitetura de compilação ou contratos
públicos exigem RFC antes da implementação:

1. Copie o template em [`docs/rfcs/0000-template.md`](docs/rfcs/0000-template.md).
2. Abra uma Issue ou Draft PR com motivação, design detalhado, exemplos de código,
   impacto em runtime, alternativas, riscos e plano de migração.
3. Aguarde revisão e consenso. A implementação começa depois da aceitação.
4. Se houver decisão arquitetural relevante, registre um ADR em [`docs/decisions/`](docs/decisions/).

Papéis e escopo de decisão estão em [GOVERNANCE.md](GOVERNANCE.md).

## Fluxo de um Pull Request

1. Abra issue ou RFC descrevendo a motivação.
2. Sincronize com `main`.
3. Implemente, escrevendo ou atualizando testes junto.
4. Atualize a documentação em `docs/` quando aplicável.
5. Rode o portão de qualidade (abaixo) — precisa passar.
6. Descreva os trade-offs no corpo do PR.

## Portão de qualidade

Um comando, e é o mesmo conjunto que o CI executa:

```bash
cargo run -p xtask -- devcheck
```

Ele roda, nesta ordem e abortando na primeira falha:

| Passo | Comando equivalente |
|---|---|
| Formatação | `cargo fmt --all -- --check` |
| Lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| Testes | `cargo test --workspace --locked` |
| Exemplos | `cargo run -p xtask -- run-examples` |
| Varredura de panics | `cargo run -p xtask -- scan` |

Cobertura é opcional e requer `cargo install cargo-llvm-cov`:

```bash
cargo run -p xtask -- devcheck --coverage
cargo run -p xtask -- coverage --html    # só o relatório, saída em coverage/
```

Se estiver no Windows com o repositório dentro do OneDrive, veja a nota sobre
Smart App Control em [`docs/guides/installation.md`](docs/guides/installation.md)
antes de rodar os testes.

## Convenção de commits

[Conventional Commits](https://www.conventionalcommits.org/), em **inglês**, no
formato `tipo(escopo): assunto`.

Tipos aceitos:

| Tipo | Uso |
|---|---|
| `feat` | nova capacidade visível ao usuário |
| `fix` | correção de comportamento incorreto |
| `perf` | mudança que altera performance sem mudar comportamento |
| `refactor` | reorganização interna sem mudança de comportamento |
| `test` | apenas testes |
| `docs` | apenas documentação, README ou website |
| `ci` | workflows e automação |
| `chore` | manutenção, dependências, limpeza |

O escopo é opcional e, quando presente, é o nome do crate ou da área:
`core`, `lexer`, `parser`, `ir`, `interpreter`, `typeck`, `codegen_js`,
`reactivity`, `cli`, `xtask`, `ci`.

Regras:

- Assunto no imperativo, sem ponto final, até 72 caracteres.
- O corpo explica **por que**, não o que — o diff já mostra o que mudou.
- Um commit por tema. Se a mensagem precisa de "e", provavelmente são dois commits.
- Sem trailers `Co-authored-by` gerados automaticamente.

## Estilo de código

- Nomes explícitos, sem abreviações crípticas.
- Evite `unwrap`/`expect` em código de produção.
- Panics não tratados são proibidos na camada de execução (interpretador). Use o
  sistema de `Diagnostic`, emitindo erros acumulativos.
- Comentários explicam a razão de uma decisão sutil, não repetem o código.

### Anti-patterns

| Padrão | Alternativa |
|--------|-------------|
| Função gigante monolítica | Extrair helpers nomeados |
| Duplicação de lógica de parsing | Compartilhar utilitários |
| Panics silenciosos | Erros estruturados + mensagens claras |

## Testes

- `crates/<crate>/tests/` — testes de integração por crate
- `crates/interpreter/tests/` — cenários de linguagem
- `crates/parser/tests/` — erros sintáticos e templates
- `cli/tests/` — comportamento da CLI ponta a ponta

O diretório `tests/` na raiz **não é alvo de build**: a raiz declara `[workspace]`
sem `[package]`. Testes colocados ali nunca compilam e nunca rodam.

## Exemplos

Os exemplos de linguagem ficam em `examples/` com prefixo numérico (`00_..`,
`01_..`); `examples/artkit/` e `examples/modules/` guardam os casos de componente
e de módulo.

```bash
cargo run -p xtask -- run-examples
```

Executa cada `.art` sob `examples/`, recursivamente, escreve as saídas em
`target/example-output/` e falha se algum retornar erro, imprimir `panic` ou
derrubar uma thread. O job `examples` do CI roda exatamente este comando.

Ao adicionar um recurso, inclua um exemplo novo e incremental — não reescreva os
existentes, mantenha a progressão pedagógica.

## Ao alterar a AST

Mudanças em `crates/core/src/ast.rs` exigem atualizar a documentação
correspondente (`docs/overview.md` e os guias de linguagem afetados). Para validar:

```bash
bash scripts/check_ast_docs.sh
```

## CI

`.github/workflows/ci.yml` executa: lint (fmt + clippy), build e testes, exemplos,
métricas, smoke de Node e de ArtKit, e regressão de performance. Cobertura roda em
job separado sob `workflow_dispatch`. Mantenha o build verde.

## Revisão

Critérios:

- Correção, sem quebrar exemplos existentes
- Cobertura dos casos de erro
- Legibilidade, com comentários onde a lógica é sutil
- Documentação atualizada

## Comunicação

Linguagem inclusiva e objetiva. Debates técnicos focados em fatos e medições.
