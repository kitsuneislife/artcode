// Smoke test for the JIT builder. This test is compiled only when the crate
// is built with `--features=jit`. It is ignored by default because it
// requires LLVM/inkwell available on the system.

#![cfg(feature = "jit")]

#[ignore]
#[test]
fn smoke_compile_const_function() {
    // Use the public re-exported LlvmBuilder (available when feature=jit)
    // Build a minimal textual IR that the prototype builder understands.
    let ir = "func @f() -> i64 { entry: %c = const i64 42 br end\nend: ret %c }";

    // Initialize and lower/compile
    let _ = jit::LlvmBuilder::initialize().expect("initialize");
    let module_text = jit::LlvmBuilder::lower_ir_to_module(ir).expect("lower to module");
    let addr = jit::LlvmBuilder::compile_module_get_symbol(&module_text, "f").expect("compile symbol");

    // Call the function pointer (unsafe). This expects the JIT to produce a
    // callable function returning i64.
    let f: extern "C" fn() -> i64 = unsafe { std::mem::transmute(addr) };
    let res = unsafe { f() };
    assert_eq!(res, 42);
}
