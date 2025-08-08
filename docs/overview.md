# Visão Geral do Projeto Artcode

Artcode é uma linguagem de programação experimental construída em Rust com foco em Complexidade Progressiva: iniciantes têm uma sintaxe simples; usuários avançados ganham mecanismos explícitos (enums, pattern matching, interpolação rica, escopos lexicais, etc.).

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
- `interpreter`: Avaliador; executa programa, gerencia escopos, pattern matching, enum inference, f-strings, pseudo-métodos em arrays.
- `cli`: Interface de linha de comando (REPL simples e execução de arquivo).

## Fluxo de Execução
1. Ler fonte (arquivo ou REPL)
2. Lexer gera tokens (inclui InterpolatedString para f-strings)
3. Parser converte tokens em AST (`Program = Vec<Stmt>`)
4. Interpreter percorre statements, avaliando expressões, gerindo ambientes aninhados
5. Funções criam closures capturando ambiente pai
6. Pattern matching resolve variantes de enum e bindings

## Feature Flags Planejadas
- [ ] JIT/AOT híbrido com coleta de perfil
- [ ] Analisador de ciclos de referência (ferramenta externa de teste)
- [ ] Sistema de tipos gradual / generics reais
- [ ] Time-travel debugging (record & replay)

## Decisões de Design
| Tema | Decisão | Racional |
|------|---------|----------|
| Memória | ARC + Rc/RefCell | Simplicidade inicial e segurança; GC evitado em produção |
| Erros | Enum Result + pattern matching | Explícito, evita exceções invisíveis |
| Interpolação | Parser recursivo por expressão | Reutilização do pipeline existente |
| Enum shorthand | Inferência única por variante | Ergonomia mantendo determinismo |
| Field access arrays | Métodos builtin (`sum`, `count`) | Protótipo; futuro: traits ou impl blocks |

## Limitações Atuais
- Panics em erros de parser de f-string (não relatam posição detalhada)
- Inferência de enum ambígua gera panic (transformar em erro relatável)
- Sem otimizações de execução (interpretação direta)
- Sem persistência de último valor para REPL amigável

## Contribuindo
1. Abrir issue ou RFC para mudanças estruturais
2. Manter separação de responsabilidades entre crates
3. Adicionar testes de integração no crate `interpreter` para comportamento de linguagem
4. Documentar decisões no diretório `docs/`.

---
Próximo: veja `docs/prelude.md` para detalhes sobre o prelude e Result.
