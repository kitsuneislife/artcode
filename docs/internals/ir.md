## IR textual — especificação mínima

Esta página documenta a forma textual mínima da IR usada pelo pipeline JIT/AOT.

Objetivo: fornecer um formato legível, estável para golden tests e para inspeção humana.

Tipos básicos
- `i64`, `f64`, `void`

Instruções suportadas (subset inicial)
- `const <type> <value>` — materializa constante em um nome/temporário
- `add/sub/mul/div <type> <a>, <b>` — operações aritméticas inteiras
- `call <fn> (<args...>)` — chamada de função (dest é um nome de temp no emitter)
- `br <label>` — branch incondicional
- `br_cond <pred>, <if_true>, <if_false>` — branch condicional (pred é i64 truthy)
- `phi <type> [ <val>, <bb> ], ...` — seleção de valor em merge
- `ret <val?>` — retorno (opcional)

Formato de função

func @<name>(<type> <param>, ...) -> <ret> {
  entry:
    <instrs...>
}

Exemplo simples (soma):

func @add(i64 a, i64 b) -> i64 {
  entry:
    %add_0 = add i64 a, b
    ret %add_0
}

Semântica de `--emit-ir`
- `art run --emit-ir -` : imprime a IR textual gerada para cada função encontrada no programa (stdout).
- `art run --emit-ir out.ir` : escreve a IR textual em `out.ir`.

Notas de design
- A IR é gerada em forma próxima de SSA; o `crates/ir::ssa` contém uma renaming pass simples.
- A IR textual serve para golden tests (`crates/ir/src/bin/irgen.rs`) e para inspeção humana.

Referências
- RFC: `docs/rfcs/0004-ir-architecture.md`
# Especificação (mínima) — IR textual

Esta página descreve a forma textual mínima da IR usada pelo projeto Artcode.

Objetivo
- Formato legível por humanos para inspeção e golden-tests.
- Representação por função com blocos nomeados e instruções simples (estilo SSA temporário).

Tipos básicos
- i64, f64, bool, ptr, void

Instruções suportadas (subset inicial)
- const <type> <value>
- add/sub/mul/div <type> %a %b
- fadd/fsub/fmul/fdiv <type> %a %b
- load <type> %ptr
- store <type> %value, %ptr
- call @fn %arg1 %arg2 ...
- br <label>
- br_cond %pred <if_true> <if_false>
- phi <type> %dst = phi [ %v1, %bb1 ], [ %v2, %bb2 ]
- ret <val?>
- intrinsics: gc_alloc <bytes>, gc_write_barrier %ptr %val, arena_alloc <arena_id> <bytes>

Formato de função

func @name(<params...>) -> <ret-type> {
  <block_label>:
    %tmp0 = add i64 %a %b
    ret %tmp0
}

Observações
- Esta IR é intencionalmente pequena. Durante lowering o sistema pode gerar temporários e renomear valores.
- Golden-files devem usar este formato mínimo para facilitar revisão humana.

Exemplo

func @add(i64 %a, i64 %b) -> i64 {
entry:
  %0 = add i64 %a %b
  ret %0
}

Referências
- RFC: `docs/rfcs/0004-ir-architecture.md`

IR textual format (guidelines)

- Function header: `func @name(<type> <param>, ...) -> <ret> {` on a single line.
- Basic blocks: a label followed by `:` and instructions indented by two spaces.
- Temporary names: use `%<symbol>_<n>` for temporaries created during lowering (e.g. `%add_0`).
- Operands: prefer using parameter names when available (e.g. `a`, `b`) rather than numbered temps.
- Phi nodes: must appear in a block that joins control-flow and use the form: `x = phi <type> [ %v1, bb1 ], [ %v2, bb2 ]`.

Golden-files guidance

- Place golden files under `crates/ir/golden/` with the extension `.ir`.
- Golden files should be stable and human-reviewable. When changing lowering rules, update goldens with `cargo run -p xtask -- irgen --write` and open a PR.
- Use `cargo run -p xtask -- irgen --check` in CI to ensure no unintended diffs.

Lowering contract

- The lowering entrypoint `crates/ir::lowering::lower_stmt` accepts a `core::ast::Stmt` and returns `Option<ir::Function>` for supported constructs (arithmetic functions, simple calls, if-then-else patterns).
- The interpreter is canonical; lowering must preserve semantics validated by golden-tests and unit tests.
