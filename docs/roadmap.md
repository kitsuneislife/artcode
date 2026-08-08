# Roadmap

## Entregue em v0.4 (lançado 2026-05-27)

### Linguagem
- Structs, enums, match com guards
- F-strings com format specs (`upper`, `lower`, `trim`, `hex`, `padN`, `debug`)
- Métodos em structs/enums via `func Tipo.metodo(self) {}` e blocos `impl Type { }`
- Closures, tuplas, destructuring
- `while` / `for` nativos
- `try/catch` explícito no parser e interpreter
- Pipeline operator `|>` e stream pipeline
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

### Tooling
- `art build --target js` — transpila para JavaScript ES2022 com source maps V3
- `art build --bundle` — bundle autocontido para Node.js e browser
- Type checker — inferência local, verificação de anotações
- LSP completo — completion, goto-def, hover, rename, semantic tokens (`art lsp`)
- TTD shell — `art debug --replay` com `step`, `breakpoint`, `state-at`
- Fuzz targets, property tests, CI regression detector

---

## Entregue em v0.5.1 (lançado 2026-05-27)

### Linguagem — Bloco B: `component {}` e qualificadores de binding
- Keywords `component`, `view`, `state`, `prop`, `memo`, `ref` no lexer
- `Stmt::ComponentBlock`, `Stmt::QualifiedBinding` no AST
- Parser: `component Name { state x: T = expr; view { <template> } }`
- Type checker: 4 regras — prop imutável, memo stale, state fora de escopo, ref em view

### Compilação — Bloco D: ReactivityPass e grafo de dependências
- Novo crate `reactivity`: `DepGraph` com nós `Source`/`Derived`/`Sink`
- `ReactivityPass` analisa `ComponentBlock` e constrói DAG de dependências
- Tarjan SCC para detecção de ciclos em memos — erro de compile time
- Codegen JS: `set_X(v)` recomputa memos em ordem topológica; text nodes nomeados

### Runtime UI — Bloco E: Scheduler, Lifecycle e DOM
- `JS_RUNTIME` com scheduler assíncrono (`__schedule`/`__flush` via `queueMicrotask`)
- Lifecycle hooks: `on_mount`, `on_destroy`, `on_update`, `tick`
- Helpers DOM: `dom.create`, `dom.text`, `dom.append`, `dom.set_attr`, `dom.set_text`, etc.

### ArtKit v0.1 — Bloco F
- `examples/artkit/counter.art` — componente Counter funcional
- `examples/artkit/todo.art` — TodoItem e TodoList com state
- `docs/guides/artkit_quickstart.md` — guia passo a passo
- CI smoke test: compila counter.art e verifica bundle com Node.js

### Stdlib e Tooling — Bloco G
- `Deque<T>` no prelude: 6 funções + `ArtValue::Deque`
- TTD delta snapshots: `Tracer::record_delta` armazena diff entre keyframes
- DAP mínimo: `art debug --dap` emite eventos `initialized`/`stopped`/`terminated`
- `art doc <path>` gera HTML em `docs/generated/<name>.html`
- `docs/guides/migration_v0.4_to_v0.5.md`

### Correções — v0.5.1
- Enforcement de anotação de tipo em `state`/`prop`/`memo`
- `Name_create(host)` retorna os setters de state, permitindo composição pai/filho

---

## v0.6 — LLVM AOT + WASM + Generics (em desenvolvimento)

### Entregue

#### Backend LLVM AOT — Bloco A (parcial)
- Novo emissor de LLVM IR textual (`crates/ir/src/llvm_emitter.rs`) — traduz o IR
  interno para `.ll` válido, compilado por `clang`. Sem dependência de `inkwell`,
  desacoplado da versão da C-API do LLVM.
- `art build-aot <file> --llvm` — binário nativo via `clang -O2`, com cache por hash do IR.
- `art build-aot --emit-llvm-ir` — emite `.ll` textual (validável por `llvm-as`).
- Lowering de `if`/`else` corrigido — ramos em bloco desembrulhados e condições bool
  materializadas; gera `BrCond` + `phi` e executa nativamente.
- Motor geral de lowering (`crates/ir/src/lower_fn.rs`) — `let`, `while`, variáveis locais
  e chamadas aninhadas via modelo `alloca`/`load`/`store`.

#### Qualidade e CI — Bloco Q
- Job `lint` no CI: `cargo clippy --workspace --all-targets --locked -- -D warnings` e
  `cargo fmt --all -- --check`. Antes disso o CI não executava clippy em lugar nenhum.
- 53 warnings de clippy e 1 erro de compilação eliminados; workspace reformatado.
- Versões unificadas em `0.5.1` entre manifestos, `Cargo.lock`, README e website — a
  divergência quebrava o workflow *Metrics Validation*, que compila com `--locked`.
- Interpretador passa a rodar em thread com stack de 256 MB: recursão estourava a pilha
  do processo a partir de profundidade ~5 no Windows e ~40 no Linux.
- LSP corrigida no Windows: decodificação percent completa de URIs (`file:///c%3A/…`) e
  remoção do prefixo `\\?\` que `canonicalize` introduzia.
- `art add` e o resolver de imports passam a compartilhar `resolver::cache_dir()`, com
  override por `ARTCODE_HOME`. Antes discordavam do diretório de cache no Windows.
- `perf-regression` verde pela primeira vez: `.gitignore` casava `*.json` e mantinha
  `baseline/perf_fib20.json` fora do repositório, então o job abortava em "baseline file not
  found" antes de medir qualquer coisa.
- `coverage` verde: os testes de exemplo procuravam o binário em `target/debug/art`, caminho
  que não existe sob `cargo llvm-cov`. Passam a derivar o diretório do executável de teste.
- `release.yml` compila a tag informada, e não o branch do dispatch.

#### Auditoria de padrões de projeto — Bloco Q
- **Portão único.** `xtask devcheck` cobre fmt, clippy `-D warnings`, testes, exemplos e
  varredura de panics; é o mesmo conjunto que o CI executa. Antes ele descartava o exit
  status de `fmt` e `clippy`, então não conseguia falhar justamente nas duas checagens que
  o CI exige. `scripts/devcheck.sh`, que tinha o mesmo defeito explícito com `|| true`,
  foi aposentado.
- **Exemplos verificados.** Novo job `examples` roda os 56 exemplos; antes apenas 2 eram
  executados no CI. Ligar o job revelou dois exemplos que nunca haviam parseado. O runner
  foi portado de bash para Rust, roda no Windows e cobre `examples/` recursivamente.
- **Varredura de panics utilizável:** 707 achados (86% em código de teste) reduzidos a 15
  sítios reais em código de produção.
- **Crate `jit` absorvido por `ir`.** O caminho `inkwell` fazia string matching no texto do
  IR e devolvia `return 0` silenciosamente fora de quatro padrões; nenhum workflow jamais
  compilou a feature. `analyzer`, `loader`, `cache`, `trampolines` e os binários AOT
  (`aot_inspect`, `aot_consumer`, `calibrate`) vivem agora em `ir`.
- **Documentação unificada.** Havia dois guias de contribuição divergentes; o da raiz nem
  citava clippy ou fmt. `CONTRIBUTING.md` é o documento canônico e agora fixa a convenção
  de commits.
- **Higiene do repositório.** Saída gerada em `examples/_outputs/` e o diretório `tests/`
  da raiz — que nunca foi alvo de build — removidos do versionamento; exceções do
  `.gitignore` tornadas explícitas.

### Próximos objetivos

- **Interner com tempo de vida explícito (Bloco M — prioridade alta).** `core::interner::intern`
  faz `Box::leak` de cada símbolo novo num pool global permanente. É seguro para a CLI, que é
  efêmera, mas vaza sem limite em `art lsp`, que vive a sessão de edição inteira e re-lexa a
  cada tecla. Encontrado pelo `Fuzz CI`, que fuzza in-process e degrada ~6x até dar timeout.
- WASM target — pipeline IR→C→emcc + WASI standalone (Bloco W)
- Generics no interpreter — monomorphização básica na chamada de função (Bloco G)
- Diagnósticos com linha/coluna precisa — erros de parse mostram posição exata (Bloco D)
- Inlining de hot paths guiado por `aot_plan.json` (fecha o Bloco A)
- Benchmarks contínuos com histórico em CSV e detecção de regressão (Bloco P)

---

## Horizonte (sem prazo definido)

- AOT compilation completa e portável
- WASM como target de compilação
- Generics com monomorfização completa e bounds checking
- Type checker gradual com inferência inter-procedural

---

## Métricas de Qualidade

| Métrica | Estado atual |
|---------|------|
| `cargo test --workspace` | Verde — 370 testes, 0 falhas |
| `cargo clippy --workspace --all-targets -- -D warnings` | Limpo — 0 warnings |
| `cargo build --release` | Limpo |
| Exemplos | 56 exemplos, todos executados no CI |
| Panics em código de produção | 15 sítios |
| CI jobs | lint, build-and-test, examples, metrics, node-smoke, artkit-smoke, perf-regression, coverage |

---

## Contribuição

Abrir RFC para features que alterem sintaxe, semântica ou runtime. Pequenos ajustes (refactors internos, bugfixes) podem ir direto com testes.

Consulte `docs/guides/versioning.md` para a política de compatibilidade e `CONTRIBUTING.md` para o fluxo RFC → ADR → implementação.
