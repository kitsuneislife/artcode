# Notes — Problemas Conhecidos e Limitações

## JIT/AOT

- O crate `jit` está scaffolded mas não funciona sem LLVM instalado e `--features=jit` ativo.
- `art aot` gera planos de compilação, mas a compilação nativa real ainda não é funcional.
- Os exemplos `45_jit_fallback.art` e `46_aot_compilation.art` são demonstrações conceituais.

## LSP

- `art lsp` implementa o protocolo base (diagnósticos, hover simples).
- Autocomplete, goto-definition, rename e semantic tokens ainda não funcionam.
- Documentação do README descreve o estado alvo, não o estado atual.

## Generics

- O parser e o AST suportam parâmetros de tipo (`func foo<T>(x: T) -> T`).
- O interpreter **ignora** os type params em v0.2 — sem monomorphização.
- Programas com generics compilam mas os tipos não são verificados em runtime.

## Módulos

- Resolução de módulos é MVP: suporte a caminhos locais e git básico.
- Sem cache de rede, sem lockfile completo para repositórios externos.

## `impl Type { }` syntax

- Ainda não implementado. Métodos devem ser declarados com `func Tipo.metodo(self) {}`.
- `impl` como bloco agrupador está planejado para v0.3.

## Diagnósticos

- Erros de parse não incluem linha/coluna precisa na maioria dos casos.
- Planejado para v0.3.

---

_Atualize este arquivo ao descobrir limitações não óbvias que futuros colaboradores precisariam conhecer._
