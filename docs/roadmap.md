# Roadmap

## Estado em v0.2 (entregue)

### Linguagem
- Structs, enums, match com guards
- F-strings com format specs (`upper`, `lower`, `trim`, `hex`, `padN`, `debug`)
- Métodos em structs/enums via `func Tipo.metodo(self) {}`
- Closures, tuplas, destructuring
- `while` / `for` nativos
- `try/catch` explícito no parser e interpreter
- Pipeline operator `|>` e stream pipeline
- Pattern matching com guards
- Módulos básicos (resolução local e git)
- Modo puro (`--pure`)

### Runtime
- Actor model com mailbox e scheduler round-robin
- Arena allocation (`arena_new`, `arena_with`, `arena_release`)
- `performant` blocks com escape analysis
- Weak/unowned references e detecção de ciclos (Tarjan SCC)
- Adaptive ARC
- Capability tokens com move-semantics
- Zero-copy IPC / serialização binária (`Buffer`, `serialize`, `deserialize`)
- FFI baseline C-ABI (`art_extern!`, `art_handle_*`)

### Tooling
- CLI: `art run`, `art lsp`, `art doc std`, `art format`, `art lint`, `art aot`
- Time-Travel Debugging Fase 1 (event sourcing + replay de `time_now` e `rand_next`)
- JIT/AOT infrastructure scaffolded (requer LLVM instalado com `--features=jit`)
- LSP básico (diagnósticos, hover)
- Métricas de runtime (`handled_errors`, `executed_statements`, `crash_free%`)
- PGO profiling com `run_pgo.sh`

---

## v0.3 — Em andamento

### Correções de saúde (M1) — concluído
- [x] Eliminar `unwrap()` críticos no actor scheduler, AOT, LSP e main.rs
- [x] Consolidar seções duplicadas no CHANGELOG
- [x] Remover referências mortas na documentação

### Arquitetura (M2) — em andamento
- [x] Extrair `cycle_detection` de `interpreter.rs`
- [x] Extrair `builtins` de `interpreter.rs`
- [x] Extrair `actors` de `interpreter.rs`
- [ ] Extrair `eval` (avaliação de expressões)
- [ ] Extrair `exec` (execução de statements)
- [ ] Extrair `gc` (gerenciamento de memória)
- [ ] Extrair `arena` (lifecycle de arenas)

### Features de linguagem
- [ ] `impl Type { ... }` block syntax — sugar para registrar métodos agrupados
- [ ] Generics no interpreter — monomorphização básica na chamada de função
- [ ] Diagnósticos com linha/coluna — erros de parse mostram posição exata
- [ ] REPL: exibir último valor avaliado

---

## v0.4 — Visão de médio prazo

- Time-Travel Debugging Fase 2: debug shell interativo, checkpoints navegáveis
- JIT compilação funcional (LLVM integrado sem flags extras)
- FFI expandida para Rust ABI
- Sistema de módulos além do MVP (cache, resolução em rede)
- LSP: autocomplete, goto-definition, rename, semantic tokens
- Type checker gradual (anotações opcionais com inferência local)

---

## Horizonte (sem prazo definido)

- AOT compilation completa e portável
- WASM como target de compilação
- Generics com monomorfização completa e bounds checking
- Debugger integrado ao LSP

---

## Métricas de Sucesso

| Métrica | Alvo |
|---------|------|
| Linhas em interpreter.rs | < 2.500 (era 6.734) |
| Tempo de bootstrap | < 50ms para programa simples |
| Cobertura de testes | > 70% núcleo |
| Crash-free sessions | 99% (sem `unwrap()` não justificados) |

---

## Riscos

| Risco | Mitigação |
|-------|-----------|
| Generics invasivos quebram features existentes | Implementar monomorphização como camada opt-in; rollback fácil |
| Refatoração de interpreter.rs quebra testes | Cargo test obrigatório a cada extração de módulo |
| JIT requer LLVM como dependência pesada | Manter como feature flag; não ativar por padrão |

---

## Contribuição

Abrir RFC para features que alterem sintaxe, semântica ou runtime. Pequenos ajustes (refactors internos, bugfixes) podem ir direto com testes.

Consulte `docs/versioning.md` para a política de compatibilidade e `docs/contributing.md` para o fluxo RFC → ADR → implementação.
