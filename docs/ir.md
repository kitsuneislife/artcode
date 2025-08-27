# IR textual format

This document describes the minimal textual IR emitted by `crates/ir`.

- Functions are emitted using `func @name(<type> <param>, ...) -> <ret> { ... }`.
- Basic blocks end with a label followed by instructions. If no labels are present, an implicit `entry:` label is emitted.
- Instructions supported in the MVP:
  - `const i64 <value>` (represented as `ConstI64` with named temp)
  - `add/sub/mul/div` for i64 integer arithmetic
  - `call` call returning a value
  - `br` unconditional branch
  - `br_cond` conditional branch: `br_cond %pred, then_bb, else_bb`
  - `phi` SSA join: `x = phi i64 [ v1, bb1 ], [ v2, bb2 ]`
  - `ret` returns a value or void

Example:

func @add(i64 a, i64 b) -> i64 {
  entry:
  %add_0 = add i64 a, b
  ret %add_0
}

Lowering: `crates/ir::lowering::lower_stmt` accepts `core::ast::Stmt` and returns `Option<ir::Function>` for supported patterns (arithmetic expressions, simple calls, if-then-else patterns). Golden tests in `crates/ir/tests/` validate the expected textual output.
