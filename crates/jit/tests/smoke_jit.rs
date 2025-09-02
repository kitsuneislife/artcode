#![cfg(feature = "jit")]

use jit::LlvmBuilder;

#[test]
fn smoke_compile_add_function() {
    let ir = r#"func @add(i64 a, i64 b) -> i64 {
  entry:
  %t0 = add i64 a, b
  ret %t0
} "#;

    // should return an address when LLVM is present
    let res = jit::jit_compile_text("add", ir);
    assert!(res.is_ok(), "JIT smoke compile failed: {:?}", res);
}
