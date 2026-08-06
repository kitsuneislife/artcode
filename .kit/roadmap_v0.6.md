# Roadmap v0.6 — LLVM AOT + WASM + Generics

Objetivo central: tornar o Artcode um compilador de produção —
`art aot` produzindo binários nativos reais via LLVM (sem gcc/clang externo),
`art build --target wasm` funcional via pipeline IR→C→emcc,
generics básicos no interpreter/type checker,
e diagnósticos com posição precisa em todo o pipeline.

Estado: v0.5.1 concluído e versionado (manifestos, README e website alinhados). v0.6 em desenvolvimento.

---

## Progresso geral  [███░░░░░░░]  20 / 57

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

## T — TTD Fase 2: Debug Shell Interativo  [████░░░░░░]  2 / 5

Fase 1 (record/replay event sourcing) foi entregue em v0.4. **Correção de registro:** a maior
parte da Fase 2 já tinha sido entregue no commit `a5f4d9f` ("TTD Fase 2 — debug shell interativo
completo") e este bloco estava marcado 0/5 por engano. Status real abaixo, verificado contra
`crates/interpreter/src/interpreter/exec.rs` e `cli/src/main.rs`.

- [x] `art debug --replay <trace.artlog>`: abre shell REPL com histórico carregado (`cli/src/main.rs`)
- [~] Comando `step-back [N]`: `step-back`/`back`/`b` recuam 1 passo; falta aceitar o argumento `N`
- [x] Comando `state-at <tick>`: implementado, com alias `goto` (`exec.rs:215`)
- [~] Comando `inspect mailbox <actor_id>`: `mailbox` lista todos os actors; falta filtrar por id
- [~] Integração de breakpoints no editor: entregue via **DAP** (`art debug --dap`) em vez de
      `publishDiagnostics` — protocolo correto para o caso de uso. Item reescrito, não abandonado.

Comandos adicionais já presentes e não previstos no plano: `continue`, `quit`, `state`,
`breakpoint <line>`, `breakpoints`, `clear`, `help`.

---

## P — Performance & Benchmarks  [░░░░░░░░░░]  0 / 5

Infraestrutura de medição contínua — sem mais baseline manual.

- [ ] Suite micro: `bench/cases/arith.art`, `bench/cases/match.art`, `bench/cases/method_dispatch.art` — cada um executa 100k iterações e reporta stmt/s
- [ ] Suite macro: `bench/cases/macro_suite.art` — interpreta `examples/04_functions_closures.art` e `examples/20_actors_simple.art` encadeados
- [ ] Baseline contínuo: `bench/perf_history.csv` atualizado pelo CI a cada push em `main` (append de linha com `date,commit,stmt_s`)
- [ ] Detector de regressão CI: se stmt/s cair > 5% vs. linha anterior no CSV, falha o job `perf-regression`
- [ ] Relatório de regressão em PR: `scripts/perf_report.sh` gera tabela markdown comparando branch vs. main e posta como step summary no GitHub Actions

---

## Q — Qualidade, CI e Higiene do Repositório  [█████████░]  8 / 9

Bloco não previsto no plano original, aberto para fechar o buraco que deixou o CI vermelho e
as versões dessincronizadas. O CI não executava `clippy` em lugar nenhum, então a alegação de
"zero warnings" do commit `b7ba5c3` regrediu sem ninguém perceber.

**CI vermelho**
- [x] `Metrics Validation` verde: a etapa `Build workspace` usa `--locked` e o `Cargo.lock`
      declarava `cli 0.5.0` enquanto os manifestos diziam `0.4.0` — `cargo` recusava atualizar o
      lock e falhava antes de compilar. Versões unificadas e lock regenerado.
- [x] `CI / perf-regression` verde: o job nunca havia passado desde `0ce2e6f`. Causa real —
      `.gitignore` casava `*.json` globalmente e engolia `baseline/perf_fib20.json`, que existia
      só nas cópias de trabalho. Todo run abortava em "baseline file not found" antes de medir
      qualquer coisa; performance nunca esteve envolvida. Padrão negado para `baseline/` e
      arquivo commitado.
- [x] `CI / coverage` verde: `integration_example_13` e `_99` procuravam o binário em
      `target/debug/art`, mas `cargo llvm-cov` compila em `target/llvm-cov-target`. Helper
      compartilhado em `crates/interpreter/tests/common/mod.rs` deriva o diretório do próprio
      executável de teste. Etapa passou a usar `--no-fail-fast`, senão cada teste com a mesma
      suposição custa um ciclo de CI para aparecer.
- [x] `CI / lint` verde: `-D unused-mut` disparava só fora do Windows, porque o `mut` de
      `to_file_uri` só é usado dentro de `#[cfg(windows)]`.

**Guardas novas**
- [x] Job `lint` no `ci.yml`: `cargo clippy --workspace --all-targets --locked -- -D warnings`
      e `cargo fmt --all -- --check`
- [x] `--locked` também em `build-and-test`, travando a deriva entre `Cargo.lock` e manifestos
- [x] 53 warnings de clippy zerados (18× `assert!(false, …)` → `panic!`, 10× `useless_conversion`,
      6× `for_kv_map`, transmutes de ponteiro de função, casts redundantes) + 1 erro que quebrava
      o build (`approx_constant` em `string_builtins.rs`)
- [x] `cargo fmt` aplicado no workspace inteiro (88 arquivos fora de formato)

**Bugs reais encontrados durante a limpeza**
- [x] Recursão estourava a pilha a partir de profundidade ~5 no Windows (~40 no Linux): o
      interpretador agora roda numa thread dedicada com 256 MB de stack, então o guarda de
      profundidade 128 do próprio interpretador passa a ser atingido antes do stack overflow.
      Isso também corrigiu `cli --test stream_pipeline`, que falhava por essa causa.
- [x] LSP quebrada no Windows: `file:///c%3A/…` enviado pelo VS Code nunca resolvia (decoder
      só tratava `%20`/`%23`/`%25`) e `to_file_uri` emitia `file://\\?\C:\…` por causa de
      `canonicalize`. Decoder percent genérico + normalização de drive letter, com teste de
      round-trip URI↔path.
- [x] `art add` e o resolver de imports discordavam do diretório de cache: `art add` usava
      `env::var("HOME")` com fallback `"."`, o resolver usava `dirs::home_dir()`. No Windows o
      pacote era instalado onde o resolver nunca procurava. Unificado em `resolver::cache_dir()`,
      com override explícito via `ARTCODE_HOME`.
- [x] `release.yml` dava checkout no branch do dispatch, não na tag informada, e publicava os
      binários resultantes sob o nome dessa tag — release e artefatos podiam divergir. Ambos os
      jobs passaram a usar `ref: ${{ github.event.inputs.tag }}`.

**Higiene**
- [x] `.gitattributes` normalizando fim de linha (`eol=lf`) — sem ele todo checkout no Windows
      marcava o repositório inteiro como modificado; `core.fileMode false` elimina o ruído dos
      14 scripts alternando 755/644
- [x] `cli/.art-lock` removido do versionamento: é reescrito pelos testes com caminhos absolutos
      de `/tmp`. `.art-lock` e `docs/generated/` agora no `.gitignore`
- [x] Versões unificadas em `0.5.1` nos 11 manifestos, README e website (estavam em três valores
      diferentes: `0.4.0` nos manifestos, `0.4.0` no README, `0.5.0` no website)

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

- [x] `cargo test --workspace` verde; `cargo clippy --workspace --all-targets -- -D warnings` limpo;
      `cargo fmt --all --check` limpo — verificado no Windows e travado por job de CI
- [x] Todos os workflows do GitHub Actions verdes, `perf-regression` e `coverage` inclusive
- [x] `art build-aot <file> --llvm` produz binário nativo via `clang` (LLVM IR textual, sem `inkwell`)
- [x] `art build-aot --emit-llvm-ir <file>` emite `.ll` válido verificado por `llvm-as`
- [ ] `art build --target wasm examples/wasm/fib.art --standalone` produz `.wasm` executado por `wasmtime`
- [ ] Generics: `func id<T>(x: T) -> T`, `struct Pair<A,B>`, `enum Option<T>` funcionando no interpreter
- [ ] Todos os erros de runtime carregam `(line, col)` — zero mensagens de erro sem posição
- [~] `art debug --replay` abre shell, aceita `step-back`, `state-at`, `inspect mailbox`
      (shell e `state-at` prontos; falta o argumento `N` de `step-back` e o filtro por actor id)
- [ ] `bench/perf_history.csv` atualizado no CI; regressão > 5% bloqueia merge
- [ ] Todos os exemplos de v0.5 funcionando sem regressão
- [ ] `cargo test --all` cobre LLVM AOT (com `--features=jit` no CI quando LLVM disponível)

---

## Riscos Ativos

- **Interner global vaza sem limite** (`crates/core/src/interner.rs`): `intern` faz `Box::leak`
  de cada símbolo novo num pool permanente. Aceitável para a CLI, que é efêmera; problema real
  em `art lsp`, que vive por toda a sessão de edição e re-lexa a cada tecla. O `Fuzz CI` expõe
  isso porque fuzza in-process: o custo por iteração sobe ~6x até o libFuzzer declarar timeout,
  e o input de 2 bytes que ele reporta é apenas quem estava na vez. Enquanto não for corrigido,
  `Fuzz CI` fica vermelho — é achado legítimo, não falha de infraestrutura.

- LLVM dev libs no CI runner: inkwell requer LLVM 15 instalado — job AOT deve ser condicional via `[build-llvm]` label ou runner dedicado
- emcc no CI: job WASM condicional a Emscripten disponível — runner `ubuntu-latest` precisa de `apt install emscripten` ou action dedicada
- Monomorphização sem LRU: explosão de instâncias genéricas em código pathológico — limitar a 256 instâncias por função e emitir warning se excedido
- TTD delta snapshots + step-back: risco de inconsistência se keyframe não cobre o tick solicitado — fallback para replay completo desde tick 0

---

Atualize continuamente. Abra RFC antes de mudanças estruturais profundas.
