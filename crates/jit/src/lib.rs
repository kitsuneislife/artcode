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

use std::fmt;

/// Erro público do JIT. Substitui retornos `String` para uma API mais forte.
#[derive(Debug, Clone)]
pub enum JitError {
    NotEnabled,
    LoweringNotAvailable,
    SymbolNotFound(String),
    EngineCreation(String),
    Other(String),
}

impl From<String> for JitError {
    fn from(s: String) -> Self {
        JitError::Other(s)
    }
}

impl fmt::Display for JitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JitError::NotEnabled => write!(f, "jit feature not enabled"),
            JitError::LoweringNotAvailable => write!(f, "lowering not available"),
            JitError::SymbolNotFound(s) => write!(f, "symbol not found: {}", s),
            JitError::EngineCreation(s) => write!(f, "engine creation failed: {}", s),
            JitError::Other(s) => write!(f, "{}", s),
        }
    }
}

impl std::error::Error for JitError {}

#[cfg(feature = "jit")]
mod enabled {
    // Aqui, futuramente, colocaremos a integração com `inkwell` e ORC
    pub fn compile_function(_name: &str, _ir: &str) -> Result<*const u8, crate::JitError> {
        // Use the prototype LLVM builder to lower textual IR and compile a symbol.
        // The LLVM builder prototype returns `String` errors; map them into `JitError`.
        // Note: this code is only compiled when feature `jit` is enabled.
        match crate::llvm_builder::LlvmBuilderImpl::initialize() {
            Ok(()) => {}
            Err(e) => return Err(crate::JitError::EngineCreation(e)),
        }

        let module_text = match crate::llvm_builder::LlvmBuilderImpl::lower_ir_to_module(_ir) {
            Ok(m) => m,
            Err(e) => return Err(crate::JitError::Other(e)),
        };

        match crate::llvm_builder::LlvmBuilderImpl::compile_module_get_symbol(&module_text, _name) {
            Ok(addr) => Ok(addr as *const u8),
            Err(e) => Err(crate::JitError::Other(e)),
        }
    }

    /// Minimal typed builder used by higher-level code to request JIT compilation.
    pub struct JitBuilder {}

    impl JitBuilder {
        pub fn new() -> Self {
            JitBuilder {}
        }
        pub fn compile(&self, name: &str, ir: &str) -> Result<*const u8, JitError> {
            compile_function(name, ir)
        }
    }
}

#[cfg(not(feature = "jit"))]
mod disabled {
    pub fn compile_function(_name: &str, _ir: &str) -> Result<*const u8, crate::JitError> {
        Err(crate::JitError::NotEnabled)
    }

    pub struct JitBuilder {}

    impl JitBuilder {
        pub fn new() -> Self {
            JitBuilder {}
        }
        pub fn compile(&self, _name: &str, _ir: &str) -> Result<*const u8, crate::JitError> {
            Err(crate::JitError::NotEnabled)
        }
    }
}

#[cfg(not(feature = "jit"))]
pub use disabled::{compile_function, JitBuilder};
#[cfg(feature = "jit")]
pub use enabled::{compile_function, JitBuilder};

/// Public API: convenience stub that returns None if JIT not enabled or compilation
/// fails. Useful for higher-level integration tests.
pub fn compile_function_stub(name: &str, ir_text: &str) -> Option<usize> {
    match compile_function(name, ir_text) {
        Ok(ptr) => Some(ptr as usize),
        Err(_) => None,
    }
}

pub mod llvm_builder;
#[cfg(not(feature = "jit"))]
pub use llvm_builder::DummyLlvmBuilder as LlvmBuilder;
#[cfg(feature = "jit")]
pub use llvm_builder::LlvmBuilderImpl as LlvmBuilder;

// expose the analyzer/loader to callers and tests
pub mod ir_analyzer;
pub mod ir_loader;

/// Convenience: compile textual IR and return a raw function pointer (usize) when
/// the JIT feature is enabled. Returns Err when not available or compilation fails.
pub fn jit_compile_text(_name: &str, _ir_text: &str) -> Result<usize, JitError> {
    #[cfg(feature = "jit")]
    {
        let _ = <LlvmBuilder as llvm_builder::LlvmBuilder>::initialize().map_err(|e| e)?;
        llvm_builder::LlvmBuilder::compile_module_get_symbol(_ir_text, _name)
    }
    #[cfg(not(feature = "jit"))]
    {
        Err(JitError::NotEnabled)
    }
}

/// Load AOT plan JSON into a serde_json::Value (public helper for tests/tools)
pub fn load_aot_plan(path: &std::path::Path) -> Result<serde_json::Value, JitError> {
    let s =
        std::fs::read_to_string(path).map_err(|e| JitError::Other(format!("read plan: {}", e)))?;
    serde_json::from_str(&s).map_err(|e| JitError::Other(format!("parse plan: {}", e)))
}

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

    #[test]
    fn aot_inspect_loads_sample_files() {
        // Attempt to load repository root sample profile and plan if present
        let prof = std::path::Path::new("./profile.json");
        let plan = std::path::Path::new("./aot_plan.json");
        if prof.exists() && plan.exists() {
            // reuse the utility in a lightweight manner by using the loader functions
            let s = std::fs::read_to_string(prof).unwrap();
            let _p: serde_json::Value = serde_json::from_str(&s).unwrap();
            let s2 = std::fs::read_to_string(plan).unwrap();
            let _q: serde_json::Value = serde_json::from_str(&s2).unwrap();
        } else {
            // no-op if samples not available in this environment
        }
    }

    #[test]
    fn load_normalized_plan_smoke() {
        let p = std::path::Path::new("./aot_plan.normalized.json");
        if p.exists() {
            let v = load_aot_plan(p).expect("should parse normalized plan");
            assert!(v.get("inline_candidates").is_some());
        }
    }
}
