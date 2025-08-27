//! Crate `jit` - scaffold
//!
//! Implementação mínima para permitir que o workspace compile sem habilitar `--features=jit`.

#[cfg(feature = "jit")]
mod enabled {
    // Aqui, futuramente, colocaremos a integração com `inkwell` e ORC
    pub fn compile_function(_name: &str, _ir: &str) -> Result<*const u8, String> {
        // placeholder: implementação real dependerá de inkwell/LLVM
        Err("JIT feature not yet implemented".to_string())
    }
}

#[cfg(not(feature = "jit"))]
mod disabled {
    pub fn compile_function(_name: &str, _ir: &str) -> Result<*const u8, String> {
        Err("JIT feature not enabled; build with --features=jit".to_string())
    }
}

#[cfg(feature = "jit")]
pub use enabled::compile_function;
#[cfg(not(feature = "jit"))]
pub use disabled::compile_function;
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
