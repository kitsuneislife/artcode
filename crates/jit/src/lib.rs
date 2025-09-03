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

/// Try to compile and run a zero-arg i64 function via JIT, otherwise call the
/// provided interpreter fallback closure. This helper centralizes the safe
/// fallback behavior so higher-level code can prefer native code when
/// available but remain correct without LLVM.
pub fn compile_and_run_or_interpret<F>(name: &str, ir_text: &str, interpret: F) -> Result<i64, String>
where
    F: FnOnce() -> i64,
{
    // Basic sanity: ensure the IR signature matches the expected native ABI we will
    // call (zero-arg -> i64). This prevents transmuting to an incompatible
    // function pointer when JIT has a different prototype.
    match parse_ir_signature(ir_text) {
        Ok((param_count, ret_ty)) => {
            if param_count != 0 || ret_ty != "i64" {
                return Err(format!("IR signature mismatch: params={} ret={}", param_count, ret_ty));
            }
        }
        Err(e) => return Err(format!("failed to parse IR signature: {}", e)),
    }

    // If JIT is enabled, try to compile and execute.
    #[cfg(feature = "jit")]
    {
        // initialize builder/runtime
        if let Err(e) = <LlvmBuilder as llvm_builder::LlvmBuilder>::initialize() {
            // fallback to interpret
            return Ok(interpret());
        }

        match llvm_builder::LlvmBuilder::lower_ir_to_module(ir_text) {
            Ok(module_text) => match llvm_builder::LlvmBuilder::compile_module_get_symbol(&module_text, name) {
                Ok(addr) => {
                    // SAFETY: assume compiled function has signature extern "C" fn() -> i64
                    let f: extern "C" fn() -> i64 = unsafe { std::mem::transmute(addr) };
                    // Call the compiled code and return result. If it faults, so be it.
                    let res = unsafe { f() };
                    return Ok(res);
                }
                Err(_) => return Ok(interpret()),
            },
            Err(_) => return Ok(interpret()),
        }
    }

    // If JIT feature not compiled in, always fallback to interpreter.
    #[cfg(not(feature = "jit"))]
    {
        Ok(interpret())
    }
}

/// Parse a minimal signature from the textual IR. Returns (param_count, return_type)
/// for lines like: `func @f() -> i64 { ... }` or `func @f(i64, i64) -> i64 {`.
pub fn parse_ir_signature(ir_text: &str) -> Result<(usize, String), String> {
    // Find 'func @'
    let idx = ir_text.find("func @").ok_or("missing 'func @' prefix")?;
    let after = &ir_text[idx + "func @".len()..];
    // find first '(' after name
    let open = after.find('(').ok_or("missing '(' in signature")?;
    let name = &after[..open].trim();
    if name.is_empty() {
        return Err("empty function name".to_string());
    }
    let rest = &after[open + 1..];
    let close = rest.find(')').ok_or("missing ')' in signature")?;
    let params = &rest[..close].trim();
    let param_count = if params.is_empty() { 0 } else { params.split(',').count() };
    // look for '->' after close
    let after_close = &rest[close + 1..];
    let arrow_pos = after_close.find("->").ok_or("missing '->' return type")?;
    let after_arrow = &after_close[arrow_pos + 2..];
    // the return type may be followed by space and '{' or '{' directly
    let ret_ty = after_arrow.split_whitespace().next().ok_or("missing return type")?;
    Ok((param_count, ret_ty.to_string()))
}

pub mod llvm_builder;
#[cfg(feature = "jit")]
pub use llvm_builder::LlvmBuilderImpl as LlvmBuilder;
#[cfg(not(feature = "jit"))]
pub use llvm_builder::DummyLlvmBuilder as LlvmBuilder;

// expose the analyzer/loader to callers and tests
pub mod ir_analyzer;
pub mod ir_loader;

/// Convenience: compile textual IR and return a raw function pointer (usize) when
/// the JIT feature is enabled. Returns Err when not available or compilation fails.
pub fn jit_compile_text(_name: &str, _ir_text: &str) -> Result<usize, String> {
    #[cfg(feature = "jit")]
    {
    let _ = <LlvmBuilder as llvm_builder::LlvmBuilder>::initialize();
    llvm_builder::LlvmBuilder::compile_module_get_symbol(_ir_text, _name)
    }
    #[cfg(not(feature = "jit"))]
    {
        Err("jit feature not enabled".to_string())
    }
}

/// Load AOT plan JSON into a serde_json::Value (public helper for tests/tools)
pub fn load_aot_plan(path: &std::path::Path) -> Result<serde_json::Value, String> {
    let s = std::fs::read_to_string(path).map_err(|e| format!("read plan: {}", e))?;
    serde_json::from_str(&s).map_err(|e| format!("parse plan: {}", e))
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

    #[test]
    fn compile_and_run_falls_back_when_disabled() {
        // Provide an IR that if interpreted returns 7
        let ir = "func @f() -> i64 { entry: %c = const i64 7 br end\nend: ret %c }";
        let v = compile_and_run_or_interpret("f", ir, || 7).expect("should return 7");
        assert_eq!(v, 7);
    }

    #[test]
    fn parse_signature_ok() {
        let ir = "func @sum(i64, i64) -> i64 { entry: ret }";
        let (pc, rt) = parse_ir_signature(ir).expect("parse");
        assert_eq!(pc, 2);
        assert_eq!(rt, "i64");
    }

    #[test]
    fn parse_signature_errors() {
        let ir = "func f() { entry: ret }"; // missing @
        assert!(parse_ir_signature(ir).is_err());
        let ir2 = "func @g(i64 -> i64 {"; // missing )
        assert!(parse_ir_signature(ir2).is_err());
    }

    #[test]
    fn reject_non_i64_signature() {
        let ir = "func @f() -> i32 { entry: ret }";
        let res = compile_and_run_or_interpret("f", ir, || 0);
        assert!(res.is_err());
    }

}
