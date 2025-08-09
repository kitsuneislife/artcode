# Visão Geral do Projeto Artcode

Artcode é uma linguagem de programação experimental construída em Rust com foco em Complexidade Progressiva: iniciantes têm uma sintaxe simples; usuários avançados ganham mecanismos explícitos (enums, pattern matching com guards, interpolação rica (f-strings com specs), métodos em structs/enums, métricas de execução, etc.).

## Objetivos
- Construir uma base clara e modular (lexer, parser, core AST, interpreter, CLI) para evoluir em direção a compilação JIT/AOT futura.
- Fornecer Result e enums para modelagem de erros sem exceções implícitas.
- Oferecer interpolação de strings poderosa (f-strings) sem sacrificar legibilidade.
- Manter execução determinística e transparente.

## Arquitetura em Camadas
```
cli  --> parser ----> core (AST + tokens + env)
  \        ^            ^
   \       |            |
    \--> lexer ---------/
          |
          +--> interpreter (usa AST + registry de tipos)
```

### Crates
- `core`: AST, valores, tokens, ambiente léxico, padrões de match.
- `lexer`: Tokenizador simples com suporte a números, strings, f-strings (prefixo f"...").
- `parser`: Parser recursivo descendente; constrói AST; suporta f-strings (delegando sub-expressões a novo lexer/parser).
- `interpreter`: Avaliador; executa programa, gerencia escopos, pattern matching (com guards), inferência de enum shorthand, f-strings (com specs de formato), métodos reais em structs/enums, métricas.
- `cli`: Interface de linha de comando (REPL simples e execução de arquivo).

## Fluxo de Execução
1. Ler fonte (arquivo ou REPL)
2. Lexer gera tokens (inclui InterpolatedString para f-strings)
3. Parser converte tokens em AST (`Program = Vec<Stmt>`)
4. Interpreter percorre statements, avaliando expressões, gerindo ambientes aninhados
5. Funções criam closures capturando ambiente pai
6. Pattern matching resolve variantes de enum e bindings

## Feature Flags / Roadmap
- [ ] JIT/AOT híbrido com coleta de perfil
- [ ] Analisador de ciclos de referência (ferramenta externa de teste)
- [ ] Sistema de tipos gradual / generics reais
- [ ] Time-travel debugging (record & replay)
- [ ] Blocos `impl Tipo {}` para agrupamento de métodos
- [ ] Blocos `performant {}` e análise de concorrência
- [ ] Aumento progressivo do threshold de cobertura (>=80%)
- [ ] Inline caching para chamadas de método

## Decisões de Design
| Tema | Decisão | Racional |
|------|---------|----------|
| Memória | ARC + Rc/RefCell | Simplicidade inicial e segurança; GC evitado em produção |
| Erros | Diagnostics acumulativos + Result | Erros reportados sem panics, favorecendo IDE tooling |
| Interpolação | Parser recursivo + specs (`upper`, `lower`, `trim`, `hex`, `padN`, `debug`) | Poder expressivo e reutilização do pipeline |
| Enum shorthand | Inferência única por variante | Ergonomia mantendo determinismo |
| Métodos | Registro em `TypeRegistry` com auto-binding de `self` | Clareza, evita açúcar impl complexo cedo |
| Métricas | `handled_errors`, `executed_statements`, `crash_free%` | Observabilidade e qualidade contínua |
| Qualidade | CI (fmt, clippy -D warnings, testes, coverage badge) | Reforça saúde do projeto e confiança |

## Estado Atual & Limitações
- Parser e lexer não panica em entradas malformadas; erros viram diagnostics acumulados.
- Estrutura de métodos existe, mas ainda não há bloco `impl` agrupador (definição via `func Tipo.metodo(self) {}`).
- Sistema de tipos ainda básico (sem generics, sem checagem de campo em tempo de parse para struct init além de aridade/nome runtime).
- Execução ainda interpretada (sem JIT/AOT ainda).
- REPL básico (sem histórico de último valor exposto por enquanto).

## Contribuindo
1. Abrir issue ou RFC para mudanças estruturais
2. Manter separação de responsabilidades entre crates
3. Adicionar testes de integração no crate `interpreter` para comportamento de linguagem
4. Documentar decisões no diretório `docs/`.

### Qualidade & Scripts
- `cargo xtask ci` roda pipeline local (fmt, clippy, testes, scan de panics/unwrap/expect, cobertura)
- `scripts/devcheck.sh` atalho rápido de verificação
- `scripts/gen_coverage_badge.sh` gera badge SVG atualizado
- `scripts/check_ast_docs.sh` garante atualização de docs quando AST mudar

### Métricas de Execução
Interpreter expõe contadores internos:
- `handled_errors`: quantos diagnostics de runtime foram capturados sem crash
- `executed_statements`: número de statements executados na sessão
- `crash_free%`: derivado para monitorar estabilidade (usado em relatórios futuros)

---
Próximo: veja `docs/prelude.md` para detalhes sobre o prelude e Result.
