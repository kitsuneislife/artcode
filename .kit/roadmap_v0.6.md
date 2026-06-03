# Roadmap v0.6 — LLVM AOT + WASM + Generics

Objetivo central: tornar o Artcode um compilador de produção —
`art aot` produzindo binários nativos reais via LLVM (sem gcc/clang externo),
`art build --target wasm` funcional via pipeline IR→C→emcc,
generics básicos no interpreter/type checker,
e diagnósticos com posição precisa em todo o pipeline.

Estado: v0.5.1 concluído. v0.6 em desenvolvimento.

---

## Progresso geral  [██░░░░░░░░]  10 / 48

---

## A — LLVM AOT Funcional  [█████████░]  10 / 11

`art build-aot --llvm` produz binário nativo real via LLVM. **Decisão de arquitetura:**
em vez de `inkwell` (acoplado à C-API do LLVM 15, incompatível com toolchains atuais
como LLVM 22), o backend emite **LLVM IR textual (`.ll`)** e compila com `clang`. Isso é
mais minimalista (sem dependência pesada), portável e desacoplado da versão do LLVM.

**IR Lowering — expansão do subset suportado**
- [x] `if/else` → basic blocks com `BrCond`/`Phi` no IR (desembrulho de blocos + materialização de condição bool; validado via execução nativa)
- [x] `while` → loop blocks com back-edge no IR (motor geral `lower_fn`; `for` over collections deferido — Artcode não tem range syntax)
- [x] Variáveis locais no IR: `let` binding → `alloca`/`store`; leitura → `load`; rebind no mesmo slot = "reassignment" natural no modelo de memória
- [x] Chamadas de função aninhadas e recursão no IR: `lower_fn` lida com `Expr::Call` aninhado; desbloqueado por variáveis locais

**Backend LLVM textual (`crates/ir/src/llvm_emitter.rs`)**
- [x] `IR::Function → LLVM IR textual`: tradução completa das instruções existentes (ConstI64, Add, Sub, Mul, Div, Call, Br, BrCond, Phi, Ret, Deopt)
- [x] `art build-aot <file> --llvm`: compila via `clang`, emite binário nativo sem `inkwell`
- [x] `art build-aot --emit-llvm-ir`: emite `.ll` textual para inspeção (validável por `llvm-as`)

**Otimizações & cache**
- [x] `art build-aot --llvm`: compila com `clang -O2`
- [x] Cache de artefatos: hash do IR `.ll` → binário em `$TMPDIR/.artcache/` — cache hit evita recompilação
- [ ] Inlining de hot paths: funções com score ≥ 3 no `aot_plan.json` (PGO) marcadas `alwaysinline` antes da emissão

**Testes**
- [x] 4 testes de roundtrip em `crates/ir/tests/golden_llvm.rs`: emissão válida + execução nativa de aritmética, if/else (com `phi`) e chamada entre funções (guardados por disponibilidade de `clang`)
- [x] 4 testes do motor geral em `crates/ir/tests/golden_lower_general.rs`: estrutura alloca/load/store, roundtrip `let`+add (42), while counter (7), if com local (clamp_pos → 0)

---

## W — WASM Target  [░░░░░░░░░░]  0 / 8

`art build --target wasm` deixa de ser stub — pipeline IR→C→emcc funcional.
Componente ArtKit exportável para WASM + glue JS gerado.

**Pipeline IR→C→emcc**
- [ ] `art build --target wasm <file.art>`: chama IR lowering → C emitter → `emcc -O2` — remove o `eprintln!` de stub em `main.rs:1051`
- [ ] ABI exportada: funções Art `@wasm_export` geram `EMSCRIPTEN_KEEPALIVE` no C emitido
- [ ] Glue JS gerado automaticamente: `<name>.js` com `init()` + wrappers para cada função exportada

**Standalone WASI**
- [ ] `art build --target wasm --standalone`: emite `.wasm` executável com wasmtime sem browser (via WASI imports em vez de Emscripten)
- [ ] Exemplo `examples/wasm/fib.art`: `func fib(n: Int) -> Int` exportado; executado com `wasmtime fib.wasm --invoke fib 20`

**ArtKit → WASM**
- [ ] Exemplo `examples/wasm/counter_wasm.art`: `component Counter` compilado para WASM + JS glue; montado no DOM via `counter_wasm.js`
- [ ] `docs/guides/wasm_target.md`: guia passo a passo — instalar emcc/wasmtime, compilar, rodar no browser e em CLI

**CI**
- [ ] Job `wasm-smoke` em `ci.yml`: compila `fib.art --target wasm --standalone` e valida com `wasmtime` — condicional a `emcc` disponível no runner

---

## G — Generics no Interpreter  [░░░░░░░░░░]  0 / 9

RFC 0007 já existe (`docs/rfcs/0007-generics.md`). Implementação no interpreter e type checker.

**Interpreter — monomorphização lazy**
- [ ] `func foo<T>(x: T) -> T`: instanciação em call site — cria cópia concreta do body com `T` substituído pelo tipo inferido do argumento
- [ ] Memoização de instâncias: `HashMap<(fn_name, Vec<TypeName>), CompiledFn>` — mesma instância não é recompilada
- [ ] Structs genéricas: `struct Pair<A, B> { first: A, second: B }` — instanciadas em construção
- [ ] Enums genéricos: `enum Option<T> { Some(T), None }` — suporte no interpreter e no match

**Type checker — constraints**
- [ ] `func sum<T: Numeric>(items: Array<T>)`: verificação de constraint em call site — erro se `T` não satisfaz a trait
- [ ] Erro claro: `type error: 'String' does not satisfy constraint 'Numeric' for type param 'T'`

**Codegen JS**
- [ ] `func foo<T>` transpilado para JS via duck typing — sem emissão de versões concretas (JS é dinâmico)

**Exemplos & testes**
- [ ] `examples/30_generics.art`: funções, structs e enums genéricos cobrindo Int, Float, String
- [ ] 6 testes no interpreter: monomorphização com 3 tipos distintos + 2 testes de constraint + 1 teste de memoização

---

## D — Diagnósticos Precisos  [░░░░░░░░░░]  0 / 6

Erros com posição exata em todo o pipeline — interpreter, type checker e formatter.

- [ ] Span propagado no interpreter: `RuntimeError` carrega `(line: usize, col: usize)` — todos os builtins passam o `call_span` já existente
- [ ] Erros de parse com posição exata: `unexpected token '<tok>' at line X, column Y` em vez de mensagem sem coordenada
- [ ] Erros de tipo com sugestão: `did you mean 'Int'?` quando o tipo informado tem distância de edição ≤ 2 do tipo esperado
- [ ] `art lint --explain <CODE>`: imprime descrição detalhada + exemplo do código de diagnóstico (ex: `art lint --explain E0042`)
- [ ] Formatter idempotente auditado: property test round-trip `source → parse → format → parse → format` — segundo resultado === primeiro; falha bloqueia CI
- [ ] Highlight de subpattern em erros de match: span do branch com falha destacado, não do `match` inteiro

---

## T — TTD Fase 2: Debug Shell Interativo  [░░░░░░░░░░]  0 / 5

Fase 1 (record/replay event sourcing) foi entregue em v0.4. Fase 2 fecha o loop interativo.

- [ ] `art debug --replay <trace.artlog>`: abre shell REPL com histórico carregado e posição inicial no último tick
- [ ] Comando `step-back [N]`: recua N passos (default 1) — restaura snapshot mais próximo e reaplica eventos até `tick - N`
- [ ] Comando `state-at <tick>`: imprime dump de todas as variáveis vivas no tick especificado
- [ ] Comando `inspect mailbox <actor_id>`: lista mensagens no mailbox de um actor num tick
- [ ] Integração LSP mínima: `art lsp` reporta tick atual de replay como posição de "breakpoint ativo" via `textDocument/publishDiagnostics`

---

## P — Performance & Benchmarks  [░░░░░░░░░░]  0 / 5

Infraestrutura de medição contínua — sem mais baseline manual.

- [ ] Suite micro: `bench/cases/arith.art`, `bench/cases/match.art`, `bench/cases/method_dispatch.art` — cada um executa 100k iterações e reporta stmt/s
- [ ] Suite macro: `bench/cases/macro_suite.art` — interpreta `examples/04_functions_closures.art` e `examples/20_actors_simple.art` encadeados
- [ ] Baseline contínuo: `bench/perf_history.csv` atualizado pelo CI a cada push em `main` (append de linha com `date,commit,stmt_s`)
- [ ] Detector de regressão CI: se stmt/s cair > 5% vs. linha anterior no CSV, falha o job `perf-regression`
- [ ] Relatório de regressão em PR: `scripts/perf_report.sh` gera tabela markdown comparando branch vs. main e posta como step summary no GitHub Actions

---

## S — Stdlib & Tooling  [░░░░░░░░░░]  0 / 4

Itens pendentes do checklist operacional que completam a stdlib mínima.

- [ ] `std.time`: `time_now() -> Int` (nanosegundos monotônicos), `time_elapsed(start: Int) -> Int` — necessário para debug determinístico e benchmarks internos
- [ ] `std.random`: `random_seed(n: Int)`, `random_int(min: Int, max: Int) -> Int`, `random_float() -> Float` — gerador LCG com seed configurável para testes reproduzíveis
- [ ] Docs gerados automaticamente dos builtins do prelude: `art doc std` varre `crates/interpreter/src/prelude.rs` e gera `docs/generated/stdlib.html` com assinaturas e docstrings
- [ ] `docs/guides/contributing.md` ligado ao processo RFC: link para `docs/rfcs/0000-template.md` e checklist de submissão de RFC

---

## Ordem de implementação

```
A (LLVM AOT)     ──► W (WASM — depende de IR Lowering expandido em A)
G (Generics)     ──► D (Diagnósticos — span propagation mais fácil depois de generics)
T (TTD Fase 2)   ─── independente
P (Performance)  ─── independente
S (Stdlib)       ─── independente
```

```
Sequência recomendada:
1. A.1–A.4  (IR Lowering expandido)    ← desbloqueia W e A.5–A.7
2. A.5–A.7  (inkwell + art aot --llvm)
3. A.8–A.10 (otimizações, cache, testes LLVM)
4. W.1–W.8  (WASM pipeline completo)
5. G.1–G.9  (Generics)
6. D + T + P + S  (paralelo — sem dependência entre si)
```

---

## Critérios de saída para v0.6

- [x] `cargo test --all` verde; `cargo clippy -- -D warnings` limpo; `cargo build --release` limpo
- [x] `art build-aot <file> --llvm` produz binário nativo via `clang` (LLVM IR textual, sem `inkwell`)
- [x] `art build-aot --emit-llvm-ir <file>` emite `.ll` válido verificado por `llvm-as`
- [ ] `art build --target wasm examples/wasm/fib.art --standalone` produz `.wasm` executado por `wasmtime`
- [ ] Generics: `func id<T>(x: T) -> T`, `struct Pair<A,B>`, `enum Option<T>` funcionando no interpreter
- [ ] Todos os erros de runtime carregam `(line, col)` — zero mensagens de erro sem posição
- [ ] `art debug --replay` abre shell, aceita `step-back`, `state-at`, `inspect mailbox`
- [ ] `bench/perf_history.csv` atualizado no CI; regressão > 5% bloqueia merge
- [ ] Todos os exemplos de v0.5 funcionando sem regressão
- [ ] `cargo test --all` cobre LLVM AOT (com `--features=jit` no CI quando LLVM disponível)

---

## Riscos Ativos

- LLVM dev libs no CI runner: inkwell requer LLVM 15 instalado — job AOT deve ser condicional via `[build-llvm]` label ou runner dedicado
- emcc no CI: job WASM condicional a Emscripten disponível — runner `ubuntu-latest` precisa de `apt install emscripten` ou action dedicada
- Monomorphização sem LRU: explosão de instâncias genéricas em código pathológico — limitar a 256 instâncias por função e emitir warning se excedido
- TTD delta snapshots + step-back: risco de inconsistência se keyframe não cobre o tick solicitado — fallback para replay completo desde tick 0

---

Atualize continuamente. Abra RFC antes de mudanças estruturais profundas.
