# Contribuindo

## Princípios
1. Clareza antes de abstração
2. Evolução incremental e testada
3. Determinismo e previsibilidade

## Passos para PR
1. Abra uma issue descrevendo motivação (ou RFC para mudanças de linguagem)
2. Sincronize branch `main`
3. Escreva/atualize testes (integração no crate `interpreter`)
4. Atualize documentação em `docs/` se aplicável
5. `cargo test` deve passar sem panics inesperados
6. Descreva trade-offs no corpo do PR
7. Rode `cargo run -p xtask -- ci` antes de enviar (usa fmt/clippy/test/scan)

## Estilo de Código
- Preferir nomes explícitos (sem abreviações crípticas)
- Evitar unwrap/expect em código de produção (exceto protótipo sinalizado)
- Panics temporários devem ter TODO de conversão em erro estruturado

### Ferramentas Auxiliares
- `xtask` oferece:
	- `ci`: roda format checagem, clippy, testes e scan de panics.
	- `scan`: apenas relatório de `panic!/unwrap/expect`.
	- `coverage`: se `cargo-llvm-cov` instalado, gera relatório de cobertura (use `--html` para saída HTML local).

### Exemplos Numerados
Todos os exemplos de linguagem residem em `cli/examples` com prefixo numérico (`00_..`, `01_..`, ...).

Rode manualmente:
```
scripts/test_examples.sh
```
O script gera saídas em `cli/examples/_outputs/{stdout,stderr}` e falha se houver `panic`/crash.

Eles também são validados via teste integrado (`tests/examples_runner.rs`) ao executar `cargo test`.

Ao adicionar um novo recurso, inclua um exemplo incremental novo (não reescreva os existentes, mantenha progressão pedagógica).

### Cobertura
Para gerar localmente (instale antes `cargo install cargo-llvm-cov`):
```
cargo run -p xtask -- coverage --html
```
Gerará diretório `coverage/` (padrão cargo-llvm-cov) com relatório.

### CI
Workflow GitHub Actions (`.github/workflows/ci.yml`) executa: fmt, clippy (erros em warnings), testes, scan de panics e cobertura (job separado). Mantenha build verde.

### Hook de AST
Ao alterar `crates/core/src/ast.rs`, atualize também documentação relevante (`docs/overview.md`, `docs/functions.md` ou `docs/fstrings.md`).
Para validar antes de commitar, execute:
```
scripts/check_ast_docs.sh
```
Sugestão: adicionar como pre-commit hook:
```
ln -s "$(pwd)/scripts/check_ast_docs.sh" .git/hooks/pre-commit
```

## Estrutura de Testes
- `crates/interpreter/tests` para cenários de linguagem
- Futuro: adicionar `crates/parser/tests` para erros sintáticos

## RFCs
Inclua: motivação, design detalhado, exemplos de código, impacto em runtime, riscos, plano de migração.

Veja o template de RFC em `docs/rfcs/0000-template.md` para um esqueleto padronizado ao abrir propostas.

## Revisão
Critérios:
- Correção sem quebrar exemplos existentes
- Cobertura de casos de erro
- Legibilidade e comentários onde lógica é sutil
- Documentação atualizada

## Anti-Patterns
| Padrão | Alternativa |
|--------|-------------|
| Função gigante monolítica | Extrair helpers nomeados |
| Duplicação de lógica de parsing | Compartilhar utilitários | 
| Panics silenciosos | Erros estruturados + mensagens claras |

## Comunicação
Use linguagem inclusiva e objetiva. Debates técnicos com foco em fatos/benchmark.
