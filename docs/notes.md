# Notes — Problemas Conhecidos e Limitações

## JIT/AOT

- O crate `jit` está scaffolded mas não funciona sem LLVM instalado e `--features=jit` ativo.
- `art aot` gera planos de compilação, mas a compilação nativa real ainda não é funcional.
- Os exemplos `45_jit_fallback.art` e `46_aot_compilation.art` são demonstrações conceituais.

## Componentes reativos (v0.5)

- `component {}` blocks, `state`/`prop`/`memo`/`ref` são validados em compile time e geram JavaScript.
- O interpreter trata `ComponentBlock`/`QualifiedBinding` como no-ops: componentes são construtos de compilação, não de execução direta.
- `art run component.art` não produz saída visível — use `art build --target js --bundle` e abra no browser.

## Generics

- O parser e o AST suportam parâmetros de tipo (`func foo<T>(x: T) -> T`).
- O interpreter **ignora** os type params em runtime — sem monomorphização.
- Programas com generics compilam mas os tipos não são verificados em runtime.

## Módulos

- Resolução de módulos é MVP: suporte a caminhos locais e git básico.
- Sem cache de rede, sem lockfile completo para repositórios externos.

## Diagnósticos

- Erros de parse não incluem linha/coluna precisa na maioria dos casos.
- Spans são emitidos corretamente pelo lexer mas nem sempre propagados até o diagnóstico final.

## LSP

- `art lsp` implementa o protocolo completo: completion, goto-definition, hover, rename, semantic tokens.
- O servidor LSP é funcional; a qualidade do completion depende do contexto analisado.

---

_Atualize este arquivo ao descobrir limitações não óbvias que futuros colaboradores precisariam conhecer._
