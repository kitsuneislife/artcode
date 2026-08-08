# Changelog

Todas as mudancas relevantes deste projeto serao documentadas neste arquivo.
O formato segue [Keep a Changelog](https://keepachangelog.com) e SemVer.

## [Unreleased]

### Fixed
- **CI *Metrics Validation* voltou a passar.** A etapa `Build workspace` compila com `--locked`, mas o `Cargo.lock` versionado declarava `cli 0.5.0` enquanto os manifestos diziam `0.4.0`; `cargo` recusava atualizar o lock e falhava antes de compilar qualquer coisa. Todos os 11 crates foram unificados em `0.5.1` e o lock regenerado.
- **Versões alinhadas em `0.5.1`** entre manifestos, `Cargo.lock`, README e website. Estavam em três valores distintos (`0.4.0` nos manifestos e README, `0.5.0` no website), apesar de o CHANGELOG já documentar `0.5.1`.
- **Recursão deixou de estourar a pilha do processo.** O interpretador percorre a pilha do Rust uma vez por frame de chamada Art e gasta ~200 KB por frame em build debug, então a stack padrão da thread principal (1 MB no Windows, 8 MB no Linux) estourava a partir de profundidade ~5 e ~40 respectivamente — antes de o guarda de nesting 128 do próprio interpretador ser atingido. A CLI agora executa numa thread dedicada com 256 MB de stack. Isso também corrigiu `cli --test stream_pipeline`, que falhava por essa causa.
- **LSP corrigida no Windows.** `decode_file_uri_path` só decodificava `%20`, `%23` e `%25`, então o `file:///c%3A/…` que o VS Code envia nunca resolvia para um caminho válido; e `to_file_uri` emitia `file://\\?\C:\…` porque `std::fs::canonicalize` devolve caminhos extended-length. Agora há decodificação percent genérica, tratamento de drive letter e remoção do prefixo `\\?\`, com teste de round-trip URI↔path.
- **`art add` e o resolver de imports concordam sobre o diretório de cache.** `art add` usava `env::var("HOME")` com fallback `"."` enquanto o resolver usava `dirs::home_dir()`; no Windows, onde `HOME` normalmente não existe e `dirs` lê a pasta de perfil pela API do Win32, os pacotes eram instalados onde o resolver nunca procurava. Ambos passam a usar `resolver::cache_dir()`, com override explícito via `ARTCODE_HOME`.
- **Golden `crates/ir/golden/if.ir` regenerado.** Continuava registrando a saída antiga e inválida (`br_cond %f_0`, sobre um temp nunca definido) mesmo depois de a correção de lowering ter mudado a emissão. O passo `xtask gen-golden --check` — o primeiro do job `build-and-test` — teria falhado assim que os commits de IR chegassem ao CI.
- **53 warnings de clippy e 1 erro de compilação** eliminados no workspace (`assert!(false, …)` → `panic!`, `useless_conversion`, `for_kv_map`, transmutes de ponteiro de função, casts redundantes, `approx_constant`). Workspace inteiro reformatado com `cargo fmt`.
- **CI *perf-regression* passou pela primeira vez.** O job existia desde `0ce2e6f` e nunca havia passado. A causa não era desempenho: `.gitignore` casa `*.json` globalmente e engolia `baseline/perf_fib20.json`, que existia apenas nas cópias de trabalho e nunca foi versionado. Todo run abortava em `ERROR: baseline file not found` antes de medir qualquer coisa. O padrão passou a ser negado para `baseline/` e o arquivo foi commitado.
- **CI *coverage* voltou a passar.** `integration_example_13` e `integration_example_99` procuravam o interpretador no caminho fixo `target/debug/art`, mas `cargo llvm-cov` compila em `target/llvm-cov-target` — onde esse caminho nunca existe. Ambos passam a derivar o diretório de build do próprio executável de teste, via helper compartilhado em `crates/interpreter/tests/common/mod.rs`, e compilam `cli` no mesmo target dir quando o binário falta.
- **`-D unused-mut` falhava só fora do Windows:** o `mut` do local de `to_file_uri` é usado exclusivamente dentro de um bloco `#[cfg(windows)]`, então em Linux a variável nunca é mutada. Substituído por rebind sob o mesmo `cfg`.
- **`release.yml` compila a tag informada.** Ambos os jobs faziam `actions/checkout@v4` sem `ref`, ou seja, compilavam o branch em que o workflow foi disparado e publicavam o resultado sob o nome da tag escolhida — os binários podiam não corresponder à tag. Agora usam `ref: ${{ github.event.inputs.tag }}`.
- **Lowering de `if`/`else` para IR:** ramos embrulhados em bloco pelo parser (`if c { return x }`) agora são corretamente desembrulhados (`unwrap_single`), e condições `Bool` literais são materializadas como constante i64 (antes geravam `BrCond` sobre um temp não definido → IR inválido). `if`/`else` agora baixam para `BrCond` + `phi` e executam nativamente via LLVM.
- **E0004 em `ssa.rs` (`rename_temps`):** match não-exaustivo em Pass 2 — variantes `ICmp`, `Alloca`, `Load`, `Store` adicionadas.
- **E0004 no loader de IR** (hoje `crates/ir/src/loader.rs`): mesmo match adicionado com as 4 novas variantes, mantendo contagem de instruções correta.

### Added
- **Job `examples` no CI:** executa todos os 56 exemplos via `xtask run-examples`. Antes disso eles eram efetivamente não verificados — dois rodavam pelos testes de integração do interpretador, dois eram apenas compilados para JS e um rodava sob `|| true`. Ligar o job revelou dois exemplos que nunca haviam parseado (`22_fmt_test.art` e `45_jit_fallback.art` usavam `fn` em vez de `func`; o segundo ainda usava `return` dentro de `performant`, rejeitado pelo type checker).
- **`xtask run-examples`:** runner nativo em Rust, roda no Windows e percorre `examples/` recursivamente — o glob anterior (`examples/[0-9][0-9]_*.art`) pulava `artkit/` e `modules/` inteiros. Saída vai para `target/example-output/`.
- **Job `lint` no CI:** `cargo clippy --workspace --all-targets --locked -- -D warnings` e `cargo fmt --all -- --check`. O CI não executava clippy em lugar nenhum — a alegação de "zero warnings" do commit `b7ba5c3` havia regredido sem detecção. `build-and-test` também passa a usar `--locked`.
- **`ARTCODE_HOME`:** variável de ambiente que relocaliza o cache de pacotes (`<home>/.artcode/cache`), útil para testes, sandboxes e CI.
- **`.gitattributes`:** normaliza fim de linha (`eol=lf`). Sem ele, todo checkout no Windows convertia a árvore inteira para CRLF e marcava o repositório como modificado. `.art-lock` e `docs/generated/` passam a ser ignorados — são artefatos reescritos por testes e por `art doc`.
- **Backend LLVM AOT (`crates/ir/src/llvm_emitter.rs`):** novo emissor de LLVM IR textual (`.ll`) a partir do IR interno. Compila para binário nativo via `clang` — sem dependência de `inkwell` e desacoplado da versão da C-API do LLVM.
  - `art build-aot <file> --llvm [--out <bin>]`: emite LLVM IR, compila com `clang -O2`, gera binário nativo. Cache por hash do IR em `$TMPDIR/.artcache/` (CACHE HIT em recompilações).
  - `art build-aot <file> --emit-llvm-ir [--out <file.ll>]`: escreve o LLVM IR textual para inspeção; validável com `llvm-as`.
  - 4 testes em `crates/ir/tests/golden_llvm.rs`: emissão de texto válido + roundtrips de execução nativa (aritmética, if/else com `phi`, chamada entre funções), guardados por disponibilidade de `clang`.
  - `docs/guides/aot_llvm.md` — guia do backend LLVM AOT.
- **Motor geral de lowering (`crates/ir/src/lower_fn.rs`):** motor recursivo baseado em modelo de memória (`alloca`/`load`/`store`) para baixar o subconjunto procedural completo — `let`, `if`/`else`, `while`, `return` e chamadas aninhadas. `clang -O2` (mem2reg) promove os slots para registradores. `for` sobre coleções deferido (Artcode não tem sintaxe de range inteiro).
  - `Stmt::Let` → `alloca` + `store`; rebind do mesmo nome no mesmo slot = reassignation natural.
  - `Stmt::While` → `while_head`/`while_body`/`while_exit` com back-edge.
  - `Expr::Binary` com operadores de comparação (`<`, `<=`, `>`, `>=`, `==`, `!=`) → `ICmp` + `BrCond`.
  - 4 testes em `crates/ir/tests/golden_lower_general.rs`: estrutura IR, roundtrip `let`+add (42), while counter (7), if com local (clamp_pos → 0).
- **Novos opcodes IR:** `Instr::ICmp`, `Instr::Alloca`, `Instr::Load`, `Instr::Store` — emitidos por `llvm_emitter.rs` (LLVM IR válido), `c_emitter.rs` (C equivalente) e registrados em `ssa.rs` (`rename_temps`).

### Fixed
- **Interner deixou de vazar em processos longevos (Bloco M).** Interning fazia `Box::leak` de
  todo símbolo distinto num pool global que nunca era liberado. O comentário do código assumia
  que "o conjunto de símbolos é pequeno comparado ao tempo de vida do processo" — verdade para
  `art run script.art`, falso para `art lsp`, que re-lexa o arquivo a cada tecla, e para os
  fuzzers, que executam milhares de programas sem reiniciar. Eram três fontes:
  - `Token.symbol` era preenchido via `intern` para todo identificador e keyword, e **nenhum
    código lia o campo**. Removê-lo não exigiu alteração em nenhum outro arquivo.
  - `Environment::define` internava todo nome de binding — medido em 360 entradas para 120
    programas de 3 variáveis. `Environment.values` passou de `HashMap<&'static str, ArtValue>`
    para `HashMap<Arc<str>, ArtValue>`, liberando os nomes junto com o escopo;
    `Arc<str>: Borrow<str>` mantém as buscas por `&str` sem alocar.
  - A promoção de valores ao root em `gc.rs` fazia `Box::leak` de cada nome promovido por
    finalizer executado.

  Com `intern` sem chamadores, a função foi removida. `intern_arc` passou a guardar `Weak<str>`,
  varrendo entradas mortas quando o mapa cresce além de um limiar móvel: o dedup continua onde
  vale (`type_of` devolve de um conjunto fechado de ~15 nomes de tipo) sem reter nada além do
  último uso. Medição local no formato do fuzzer: custo por lote de 2000 iterações ficou plano
  (25ms → 20ms) contra a degradação de ~6x anterior, com o pool oscilando entre 15 e 230
  entradas. Efeito colateral: `define` deixou de travar um mutex global por binding.

### Removed
- **Crate `jit` removido; o que tinha valor foi absorvido por `ir`.** A parte "JIT" nunca compilou código: sob `--features=jit`, `llvm_builder.rs` procurava substrings no texto do IR (`ir_text.contains(" add ")`, `contains("const i64")`) e caía num `return 0` silencioso para qualquer construção fora desses quatro padrões — errado, não apenas incompleto. Nenhum workflow jamais compilou a feature, e `lib.rs:103` referenciava `LlvmBuilderImpl` fora de escopo, então ela provavelmente nem compilava. O caminho nativo que funciona é AOT via `crates/ir/src/llvm_emitter.rs`, entregue nesta versão.
  - Movidos para `ir`: `analyzer` (métrica de custo), `loader` (parser de IR textual), `cache` (`ArtCache`, FNV-1a por conteúdo), `trampolines` (ABI de chamada nativa e protocolo de deopt), `parse_ir_signature`, e os binários `aot_inspect`, `aot_consumer` e `calibrate`. `xtask` passou a invocá-los por `-p ir`.
  - Removidos: `llvm_builder.rs`, a dependência `inkwell`, a feature `jit`, `.github/docker/llvm/`, `scripts/run_jit_smoke_in_docker.sh` e `bench/run_jit_micro.sh` — todos órfãos que só existiam para sustentar a feature.
  - `docs/internals/dev/jit.md` e `llvm-docker.md` substituídos por `docs/internals/dev/aot_tooling.md`.
- **Diretório `tests/` da raiz removido.** A raiz declara `[workspace]` sem `[package]`, então nada ali era alvo de build — confirmado com `cargo test --workspace --no-run`. `cli_metrics.rs` era um subconjunto de `cli/tests/metrics_integration.rs` e apontava para `examples/99_weak_unowned_demo.art`, que não existe; `interpolation.rs` duplicava `crates/interpreter/tests/runtime.rs`; `examples_runner.rs` só chamava um script.
- **`scripts/devcheck.sh` e `scripts/test_examples.sh` aposentados** em favor de `xtask devcheck` e `xtask run-examples`. O primeiro rodava `cargo fmt --check` e `cargo clippy` sob `|| true`, ou seja, o "atalho de verificação" que a documentação recomendava não conseguia reportar falha em nenhuma das duas checagens que o CI exige com `-D warnings`.
- **`.gitmessage` e `.githooks/prepare-commit-msg` removidos.** Nunca estiveram ativos (`core.hooksPath` e `commit.template` não configurados) e o hook escrevia `\n` literal, já que `echo` do bash não interpreta escapes sem `-e`.
- **`examples/_outputs/` removido do versionamento** — seis arquivos gerados (logs de replay e IR emitido) estavam commitados. O `.gitignore` interno listava `stdout/*.out` e `stderr/*.err`, subdiretórios que não existem mais, logo não cobria nenhum dos arquivos versionados.

### Changed
- **`scripts/perf_regression.sh` reescrito.** Independente da causa da falha do job (ver *Fixed*), o script deixou de depender de `python3`, passou a medir o melhor de 5 execuções (uma amostra única num runner compartilhado não é representativa), preserva o `stderr` do binário em vez de descartá-lo com `2>/dev/null`, e imprime o JSON medido quando uma verificação falha.
- **`Fuzz CI` passa `-timeout=25` ao libFuzzer.** O job vinha sendo cancelado por estouro de tempo desde que existe. A causa não era orçamento: o log mostra o build terminando em ~40s, o fuzzer emitindo saída por 41 segundos e então silêncio absoluto até o cancelamento — ou seja, um input que trava o alvo. O `-timeout` padrão do libFuzzer é 1200s por input, tempo suficiente para consumir o job inteiro e parecer problema de infraestrutura em vez de bug. Com 25s, uma entrada travada vira finding reportado.
- **`Fuzz CI` roda os alvos em matriz paralela**, com `timeout-minutes: 45` e cache de `fuzz/cargo_fuzz/target` — um workspace separado que o cache padrão (`./target`) nunca cobriu. `cargo install cargo-fuzz` passou a usar `--locked`.
- **Etapa `coverage` usa `--no-fail-fast`.** Sem isso o `cargo test` para no primeiro alvo que falha e os demais nunca executam, o que transforma uma suposição errada compartilhada por vários testes em um ciclo de CI por teste.
- **`xtask devcheck` passou a poder falhar.** `fmt` e `clippy` descartavam o exit status com `let _ = run(..)`, então o portão local ficava verde enquanto uma regressão de clippy chegava ao CI. O clippy local também rodava sem `--all-targets` nem `--locked`, e `scan_panics` varria `crates` e um diretório `src` inexistente na raiz, nunca tocando em `cli`.
- **Varredura de panics reduzida de 707 para 15 achados.** Testes, benches e módulos `#[cfg(test)]` são ignorados — 606 dos 707 estavam em código de teste, onde uma asserção que aborta é o mecanismo de falha, não um defeito. `expect(` é contado à parte de `unwrap()` e dos macros de abort, já que o CONTRIBUTING o aceita quando a mensagem documenta a invariante.
- **`CONTRIBUTING.md` e `docs/guides/contributing.md` fundidos** num único documento na raiz. O da raiz mandava rodar só `cargo check` e `cargo test --all`, sem citar clippy nem fmt, e apontava para `.kit/checklist-v0.2.0.md`, arquivo inexistente. Passou a documentar a convenção de commits e o fato de que o `tests/` da raiz não é alvo de build.
- **`.gitignore`: `/.kit` virou `/.kit/*` com `!/.kit/roadmap_*.md`.** O roadmap versionado só continuava rastreado por ter sido adicionado com `--force`; o próximo sumiria do `git status` sem aviso — a mesma armadilha que manteve `baseline/perf_fib20.json` fora do repositório. Os roadmaps v0.4 e v0.5 entraram no versionamento junto.
- Testes que dependem de ferramentas POSIX (`echo`, `tr`, `sh`) marcados com `#![cfg(unix)]`: a sintaxe shell do Artcode executa o programa nomeado diretamente, sem interpretador de shell, e no Windows `echo` é um builtin do `cmd.exe`, não um executável.

### Removed
- Arquivos `.bak` de testes (`golden_lower_call.rs.bak`, `golden_ssa_unique.rs.bak`) e `eprintln!` de depuração remanescente no passe SSA (`crates/ir/src/ssa.rs`).
- `cli/.art-lock` do versionamento: é reescrito pelos testes de integração com caminhos absolutos de `/tmp`, portanto é estado local e não código-fonte.

## [0.5.1] - 2026-05-27

### Fixed
- **B.4 — Type annotation enforcement in component bindings:** `state`, `prop` e `memo` com anotação de tipo agora emitem `type error` quando o initializer tem tipo incompatível (ex: `state count: Int = "hello"` → erro). Resolve o último gap dos pré-requisitos ArtKit no Bloco B. 4 testes adicionados em `crates/typeck/`.
- **D.3 — Component create retorna setters:** `Name_create(host)` agora retorna `{ set_X, ... }` com todos os setters de state, permitindo composição entre componentes pai/filho sem referências globais. 2 testes adicionados em `crates/codegen_js/tests/scheduler_lifecycle.rs`.

## [0.5.0] - 2026-05-27

### Added
- **Bloco G — Pendências e completude:**
  - `Deque<T>` no prelude: `deque_new`, `deque_push_front`, `deque_push_back`, `deque_pop_front`, `deque_pop_back`, `deque_len`; variante `ArtValue::Deque` e `DequeRef` em `crates/core`; 6 testes em `interpreter/tests/deque_stdlib.rs`
  - TTD delta snapshots: `Tracer::record_delta` armazena apenas as chaves alteradas desde o último snapshot, reduzindo tamanho dos traces
  - DAP mínimo: `cli/src/dap.rs` com `send_initialized`, `send_stopped`, `send_terminated`; flag `--dap` no comando `art debug`
  - `art doc <path>` agora gera HTML em `docs/generated/<name>.html` (antes: `docs.html` no diretório atual)
  - `docs/guides/migration_v0.4_to_v0.5.md` — guia de migração com breaking changes (novos keywords, output do `art doc`)
  - GitHub Releases: workflow `release.yml` e `install.sh` para Linux x86_64 e macOS arm64

- **Bloco F — ArtKit v0.1 — primeiro componente real:**
  - `examples/artkit/counter.art` — Counter com `state count`, event handler `on:click`, view com `<p>{count}</p>`
  - `examples/artkit/todo.art` — TodoItem com `prop label`, TodoList com `state items`
  - `art build --bundle examples/artkit/counter.art` gera bundle autocontido e executa sem erros em Node.js
  - `docs/guides/artkit_quickstart.md` — guia passo a passo: instalar, criar componente, compilar, rodar no browser
  - Job CI `artkit-smoke` em `.github/workflows/ci.yml`: compila counter.art e verifica bundle com Node.js

- **Bloco E — Runtime UI: scheduler assíncrono, lifecycle hooks, dom helpers:**
  - `JS_RUNTIME` em `cli/src/bundler.rs` expandido com: scheduler assíncrono (`__schedule`/`__flush` via `queueMicrotask`), `tick(fn)`, registry de lifecycle (`on_mount`, `on_destroy`, `on_update`), helpers DOM (`dom.create`, `dom.text`, `dom.append`, `dom.set_attr`, `dom.set_text`, `dom.remove`, `dom.on`, `dom.off`, `dom.query`)
  - Codegen: `set_X(v)` usa `__schedule` para batching assíncrono de updates DOM
  - Codegen: `Name_create` expõe `Name_component` e chama `tick(() => __run_mount(...))` após DOM inserido
  - 4 testes em `codegen_js/tests/scheduler_lifecycle.rs`

- **Bloco D — ReactivityPass e grafo de dependências (`crates/reactivity/`):**
  - Novo crate `reactivity`: `DepGraph` com nós `Source`/`Derived`/`Sink`, arestas tipadas
  - `ReactivityPass::analyse` visita `Stmt::ComponentBlock` e constrói DAG de dependências
  - Tarjan SCC para detecção de ciclos em memos — emite diagnóstico `reactive cycle detected`
  - Codegen JS atualizado: `set_X(v)` recomputa memos em ordem topológica e atualiza apenas os text nodes afetados (text nodes nomeados `__txt_X_N` para cada referência reativa no template)
  - 4 testes: DAG acíclico válido (×2), ciclo entre memos detectado (×2)

- **Bloco B — `component {}` e qualificadores de binding:**
  - Lexer: 6 novos tokens — `component`, `view`, `state`, `prop`, `memo`, `ref`
  - AST: `Stmt::ComponentBlock { name, bindings, view }`, `Stmt::QualifiedBinding { qualifier, name, type_ann, value }`, `enum BindingQualifier { State, Prop, Memo, Ref }`
  - Parser: `component Name { state/prop/memo/ref x: T = expr; view { <template> } }` → `Stmt::ComponentBlock`
  - Type checker: 4 regras de diagnóstico — prop imutável, memo possivelmente stale, state fora de escopo, ref em view
  - Codegen JS: `emit_component` gera `function Name_create(host)` com criação DOM inicial
  - Interpreter: `ComponentBlock`/`QualifiedBinding` são no-ops em runtime (compile-time only)
  - 8 testes novos: 4 parser + 4 typeck

## [0.4.0] - 2026-05-27

### Added
- **Parser ArtML (`crates/parser`):** suporte a templates declarativos embutidos na linguagem.
  `<tag>`, `<tag attr="val">`, `<tag attr={expr}>`, `<tag on:event={handler}>`, `<Tag />` (componente),
  `<if cond={expr}>...</if><else>...</else>`, `<for item in {items} key={expr}>...</for>`,
  `<slot name="x">...</slot>`. Nenhum novo token no lexer — `<` em posição prefix é
  inequivocamente início de template no Pratt parser. 12 testes de parser + 10 testes de codegen.
- **Codegen de templates (`crates/codegen_js`):** `Expr::Template` → IIFE com chamadas DOM
  (`document.createElement`, `setAttribute`, `addEventListener`, `createTextNode`,
  `createDocumentFragment`). Componentes PascalCase → `new Component({...})`.
- **AST (`crates/core`):** `enum TemplateNode` (`Element`, `Component`, `Text`, `Expr`, `If`,
  `For`, `Slot`), `struct TemplateAttr`, `enum TemplateAttrValue`, `Expr::Template`.
- **Diagnósticos de template em parse time:** tag sem fechamento → `error` com span exato;
  `<for>` sem atributo `key` → `warning`.

- **Codegen JavaScript (`crates/codegen_js`):** novo crate que transpila AST Artcode para
  JavaScript ES2022. Suporta `let`/`var`, funções, structs (→ `class`), enums (→ tagged union),
  `match` (→ if-else com bindings), `for`/`while`, closures, f-strings (→ template literals),
  `performant {}` (→ IIFE), `spawn actor {}` (→ Web Worker), `import` (→ ES modules).
- **`art build --target js`:** comando CLI para compilar `.art` para JavaScript.
  Flags: `--out <dir>` (padrão `dist/`), `--sourcemap` (source map V3 com VLQ encoding),
  `--bundle`. Flag `--target wasm` scaffolded para v0.5.
- **Source maps V3:** encoding VLQ completo; erros em runtime apontam para o `.art` original
  no DevTools do browser.
- **Stdlib de strings:** 8 novos builtins no prelude — `str_split`, `str_join`, `str_contains`,
  `str_starts_with`, `str_replace`, `str_slice`, `str_to_int`, `str_to_float`. Todos com
  diagnósticos de tipo corretos e 25 testes de cobertura.
- **TTD Fase 2 — Debug Shell interativo:** `art debug --replay` agora expõe shell completo
  com `step`, `step-back`, `state` (variáveis com valores, filtrando builtins), `state-at <tick>`
  (salta para tick específico via fast-forward), `mailbox` (inspeciona mailboxes e status de
  atores), `breakpoint <line>` (pausa na linha), `continue` (executa até próximo breakpoint),
  `clear` (limpa breakpoints) e `quit`. Breakpoints são persistentes entre restarts do replay.
  6 testes de cobertura.
- **Type checker (`crates/typeck`):** novo crate de verificação estática em compile time.
  Inferência local (`let x = 42` → `x: Int`), verificação de anotações de funções
  (`func f(x: Int)` rejeita argumento `String` na chamada) e inferência paramétrica
  (`func identity<T>(x: T)` infere `T = Int` na chamada com inteiro). Integrado ao pipeline
  `art build --target js` como pré-passo antes do codegen; 12 testes de cobertura.
- **`art build --bundle`: bundler completo para JS de arquivo único.**
  `cli/src/bundler.rs` resolve imports recursivamente, compila cada módulo com
  `ModuleFormat::Bundle` (que suprime `import` stmts no codegen), deduplica módulos
  visitados e concatena: runtime preamble → módulos em ordem topológica → entry file.
  O runtime preamble (`JS_RUNTIME`) define os builtins Artcode em JS nativo
  (`println = console.log`, `str_*`, `len`, `none`, etc.), tornando o bundle
  autocontido para Node.js e browser. 3 testes de integração. Job `node-smoke-test`
  no CI verifica `art build --bundle examples/00_hello.art` com Node.js.
- **Diagnósticos de template ArtML:**
  - C.1: `on:click={expr}` onde `expr` tem tipo não-callable (Int, Float, String, Bool)
    → `lint error: event handler 'on:click' has type 'Int', which is not callable`.
    Verificado no type checker (`typeck`) ao visitar `TemplateAttrValue::EventHandler`.
    2 testes: handler não-callable emite aviso; handler function não emite.
  - C.2: Componente PascalCase usado em template sem import ou struct correspondente
    → `parse error: Component '<Counter>' is used but not imported or defined`.
    Verificado em post-parse pass no parser (`check_component_imports`).
    2 testes: componente sem definição emite erro; com struct definida não emite.
- **Robustez — fuzzing, property tests e regression detector:**
  - Novo fuzz target `interpreter_valid` — executa o interpreter apenas em programas sem erros de parse, complementando o `parser_loops` existente.
  - 9 property tests de round-trip `parse(format(src)) == parse(src)` em `cli/tests/formatter_roundtrip.rs`, incluindo idempotência do formatter.
  - 2 stress tests de arena: N atores com `performant {}` e 100 iterações de alloc/finalize (`cli/tests/actor_performant_stress.rs`).
  - `scripts/perf_regression.sh` mede `executed_statements/s` de fib(20) com baseline em `baseline/perf_fib20.json`; integrado no CI como job `perf-regression` — falha se a taxa cair abaixo de 50k stmt/s ou se a contagem de statements mudar.
  - `cli` expõe crate library (`lib.rs`) para testes de integração terem acesso direto ao `formatter`.
- **LSP — completion, goto-def, hover, rename, semantic tokens:** `art lsp` agora expõe
  `textDocument/completion` (builtins + variáveis + structs/enums com kind correto),
  `textDocument/definition` (goto-def para `let`, funções, structs, enums, imports),
  `textDocument/rename` (rename em todos os arquivos do workspace), hover com assinaturas
  de função reais e campos de struct/enum, `textDocument/semanticTokens/full` (highlight
  semântico). Arquitetura refatorada com `process_request` testável; 6 smoke tests de
  harness JSON. Guias de configuração adicionados em `docs/guides/lsp_vscode.md` e
  `docs/guides/lsp_neovim.md`.

### Deferred to v0.5
- Qualificadores `state`/`prop`/`memo`/`ref` no lexer e parser — depende de `component {}`.
- Regras de diagnóstico para `prop` mutada e `memo` com dependência ausente — depende de `component {}`.
- TTD delta snapshots e integração DAP mínima — fora de escopo v0.4.

## [0.3.0] - 2026-05-21

### Added
- **`impl Type { }` syntax:** blocos `impl` como açúcar para registrar métodos em tipos,
  equivalente ao `func Type.method(self)` existente. Retrocompatível.
- **Generics no runtime:** validação de constraints de tipo (`Numeric`, `Eq`, `Hash`,
  `Comparable`) na chamada de funções genéricas; inferência do tipo concreto pelos
  argumentos quando `type_args` não são explícitos.
- **`ArtValue::type_name()`:** reflexão de tipo em runtime para todos os variants de valor.
- Capability tokens com move-semantics: `ArtValue::Capability` / `ArtValue::MovedCapability`;
  builtins `capability_acquire`, `capability_kind`; enforcement no type checker e runtime.
- Time-Travel Debugging fase 1: tracer determinístico, flag `--record <arquivo>` no CLI,
  serialização zero-copy de fontes não-determinísticas (`time_now`, `rand_next`).
- Serialização nativa zero-copy: tipo `Buffer`, builtins `buffer_new`, `serialize`,
  `deserialize`. Serialização rejeita handles opacos (actors, mutexes, capabilities).
- APIs de arena: `arena_new`, `arena_with`, `arena_release`; métricas
  `arena_alloc_count`, `objects_finalized_per_arena`, `finalizer_promotions_per_arena`.
- Política pública de versionamento (`docs/guides/versioning.md`) e comando
  `art update --check` / `art update --self`.

### Changed
- **`interpreter.rs` reduzido de 6.734 → 1.715 linhas** por extração de sete módulos:
  `builtins`, `actors`, `cycle_detection`, `gc`, `exec`, `eval`.
- Diagnósticos de builtins agora reportam linha/coluna corretos: `call_span` propagado
  do field-access call site para todos os 81 handlers de builtin (antes sempre `0:0`).
- REPL suprime saída `[metrics]` e `[mem]` por linha; exibe `=> valor` sem ruído.
- Actor runtime com mailbox e builtins: `actor_send`, `actor_receive`,
  `actor_receive_envelope`, `actor_set_mailbox_limit`, `actor_yield`,
  `envelope`, `make_envelope`, `run_actors`.
- CLI reorganizado: `art lsp`, `art doc std`, `art format`, `art lint`, `art aot`,
  `art run --pure`.

### Fixed
- 6 `unwrap()` encadeados no actor scheduler substituídos por `expect()` descritivo.
- `aot.rs`: `.to_str().unwrap()` em path substituído por passagem direta de `OsStr`.
- `lsp.rs`: `serde_json::to_string().unwrap()` substituído por early-return seguro.
- `main.rs`: três pontos de `unwrap()` críticos corrigidos (debug repl, upgrade, copy).
- Seções duplicadas `### Changed` e `### Docs` no [Unreleased] consolidadas.
- Referência morta a `.kit/checklist.md` em `docs/overview.md` corrigida.
- Reforço no fluxo de actor scheduler para `parked` / `actor_receive` rerun.
- Escape analysis de `performant` e `bind_value_to_pattern` para evitar leaks de arena.
- Replay determinístico de `time_now` e `rand_next` no tracer/replayer.

### Docs
- `docs/` reorganizada em subpastas: `language/`, `internals/`, `guides/`, `rfcs/`.
- Números de RFC duplicados corrigidos (0002×2, 0003×2, 0005×2 → 0006, 0007, 0008).
- `docs/roadmap.md` reescrito refletindo o estado real de v0.2 e metas de v0.3/v0.4.
- `docs/notes.md` expandido com limitações conhecidas (JIT, LSP, generics, diagnostics).

## [0.2.0] - 2026-03-18

### Added
- Loop statements nativos (while/for), tuplas e destructuring.
- Blocos explicitos de try/catch no parser e interpretador.
- Modo de execucao puro via flag `--pure`.
- Builtin `dag_topo_sort` para ordenacao topologica de dependencias.
- Workflow de triagem automatica de issues com labels lang-design, runtime e tooling.
- Autodoc de stdlib via comando `art doc std`.
- Politica de versionamento publico com promessas de compatibilidade para 0.2.x.

### Changed
- GOVERNANCE.md formalizado com fluxo RFC -> ADR -> implementacao.
- CONTRIBUTING.md atualizado com processo RFC, ADR e triagem automatica.
- `docs/decisions` ganhou template ADR canonicamente referenciado.

### Fixed
- Ajustes de parser/lexer/runtime para loops, tuples e semantica de try/catch.
- Correcoes de compatibilidade do linter para mudancas recentes de AST.

### Docs
- Novos guias: loops_tuples, error_handling, pure_mode, dependency_dag e versioning.
- README, docs e website sincronizados com recursos entregues da trilha 0.2.

## Convencao de Atualizacao

- Atualizar [Unreleased] a cada PR mergeado.
- Em release, mover [Unreleased] para uma secao versionada datada.
- Classificar entradas em Added, Changed, Deprecated, Removed, Fixed e Docs.
