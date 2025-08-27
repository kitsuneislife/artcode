// Lightweight LLVM builder trait skeleton. Real implementation should live behind
// the `jit` feature and depend on `inkwell`.

pub trait LlvmBuilder {
    /// Initialize the builder/runtime. Returns an opaque handle or error.
    fn initialize() -> Result<(), String> where Self: Sized { Ok(()) }

    /// Lower textual IR to an LLVM module representation.
    fn lower_ir_to_module(ir_text: &str) -> Result<String, String> {
        // placeholder: return a textual representation of LLVM IR in the real impl
        Err("lowering not implemented".to_string())
    }

    /// Compile the module and return a function pointer for `name`.
    fn compile_module_get_symbol(_module: &str, _name: &str) -> Result<usize, String> {
        Err("compile not implemented".to_string())
    }
}
