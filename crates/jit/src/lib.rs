//! Crate `jit` - scaffold
//!
//! Esta crate provê um scaffold mínimo para o JIT. A implementação real deve ficar
//! atrás da feature `jit` (dependência opcional `inkwell`). O propósito é:
//! - permitir que o workspace compile sem LLVM;
//! - documentar a API pública e os pontos de extensão para uma futura integração
//!   com LLVM/ORC.
//!
//! Nota (pt-br): Por padrão esta crate compila como stub. Para ativar a implementação
//! real, habilite a feature `jit` e instale as bibliotecas de desenvolvimento do LLVM
//! no seu sistema.

#[cfg(feature = "jit")]
mod enabled {
    // Aqui, futuramente, colocaremos a integração com `inkwell` e ORC
    pub fn compile_function(_name: &str, _ir: &str) -> Result<*const u8, String> {
        // placeholder: implementação real dependerá de inkwell/LLVM
        Err("JIT feature not yet implemented".to_string())
    }

    /// Minimal typed builder used by higher-level code to request JIT compilation.
    pub struct JitBuilder {}

    impl JitBuilder {
        pub fn new() -> Self { JitBuilder {} }
        pub fn compile(&self, name: &str, ir: &str) -> Result<*const u8, String> {
            compile_function(name, ir)
        }
    }
}

#[cfg(not(feature = "jit"))]
mod disabled {
    pub fn compile_function(_name: &str, _ir: &str) -> Result<*const u8, String> {
        Err("JIT feature not enabled; build with --features=jit".to_string())
    }

    pub struct JitBuilder {}

    impl JitBuilder {
        pub fn new() -> Self { JitBuilder {} }
        pub fn compile(&self, _name: &str, _ir: &str) -> Result<*const u8, String> {
            Err("JIT feature not enabled".to_string())
        }
    }
}

#[cfg(feature = "jit")]
pub use enabled::{compile_function, JitBuilder};
#[cfg(not(feature = "jit"))]
pub use disabled::{compile_function, JitBuilder};

/// Public API: convenience stub that returns None if JIT not enabled or compilation
/// fails. Useful for higher-level integration tests.
pub fn compile_function_stub(name: &str, ir_text: &str) -> Option<usize> {
    match compile_function(name, ir_text) {
        Ok(ptr) => Some(ptr as usize),
        Err(_) => None,
    }
}

pub mod llvm_builder;
pub use llvm_builder::LlvmBuilder;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_returns_none_when_disabled() {
        let p = compile_function_stub("f", "func @f { entry: ret }");
        assert!(p.is_none());
    }

    #[test]
    fn builder_returns_error_message() {
        let jb = JitBuilder::new();
        let res = jb.compile("f", "func @f { entry: ret }");
        assert!(res.is_err());
    }
}
