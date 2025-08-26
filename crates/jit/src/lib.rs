// Minimal scaffold for a JIT crate. Feature-gated real implementation (inkwell) should
// be behind the `jit` feature. This file provides lightweight stubs so the workspace
// can build and tests run for contributors without LLVM.

#[cfg(feature = "jit")]
mod impls {
    // real implementation will go here, using inkwell to lower IR -> LLVM -> native
    // ... implement compile_function(fn: &ir::Function) -> *const c_void
}

/// Public API: compile a function to a native pointer. Returns None when JIT feature
/// is disabled or compilation fails. Concrete implementations live behind the `jit`
/// feature.
pub fn compile_function_stub(_name: &str, _ir_text: &str) -> Option<usize> {
    // Stub: no-op, indicates JIT not enabled.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_returns_none_when_disabled() {
        let p = compile_function_stub("f", "func @f { entry: ret }");
        assert!(p.is_none());
    }
}
