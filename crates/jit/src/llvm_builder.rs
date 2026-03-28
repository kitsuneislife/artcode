// Lightweight LLVM builder trait skeleton. Real implementation lives behind
// the `jit` feature and depends on `inkwell`.

pub trait LlvmBuilder {
    /// Initialize the builder/runtime. Returns an opaque handle or error.
    fn initialize() -> Result<(), String>
    where
        Self: Sized;

    /// Lower textual IR to an LLVM module representation and return the textual
    /// LLVM IR. The real implementation will construct an in-memory module.
    fn lower_ir_to_module(ir_text: &str) -> Result<String, String>;

    /// Compile the module (text) and return a function pointer for `name`.
    fn compile_module_get_symbol(module_text: &str, name: &str) -> Result<usize, String>;
}

#[cfg(not(feature = "jit"))]
pub struct DummyLlvmBuilder;

#[cfg(not(feature = "jit"))]
impl LlvmBuilder for DummyLlvmBuilder {
    fn initialize() -> Result<(), String> {
        Ok(())
    }

    fn lower_ir_to_module(_ir_text: &str) -> Result<String, String> {
        Err("lowering not available: build with --features=jit".to_string())
    }

    fn compile_module_get_symbol(_module_text: &str, _name: &str) -> Result<usize, String> {
        Err("jit not available: build with --features=jit".to_string())
    }
}

#[cfg(feature = "jit")]
mod enabled {
    use super::LlvmBuilder;
    use inkwell::context::Context;
    use inkwell::execution_engine::JitFunction;
    use inkwell::OptimizationLevel;
    use once_cell::sync::Lazy;
    use std::sync::Mutex;

    pub struct InkwellLlvmBuilder;

    // Global registry used to hold Modules/Engines so their pointers remain valid
    // Store only the module textual representation to identify modules; we
    // avoid attempting to store `Module<'static>` which has complex lifetimes.
    static GLOBAL_JIT_REGISTRY: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(Vec::new()));

    impl InkwellLlvmBuilder {
        // Minimal parser helpers: extract function name and parameter count
        fn parse_fn_header(ir_text: &str) -> Option<(String, usize)> {
            // expect: func @name(<params>) -> <ret>
            let s = ir_text;
            let start = s.find("func @")? + 6;
            let rest = &s[start..];
            let name_end = rest.find('(')?;
            let name = rest[..name_end].trim().to_string();
            let params_start = start + name_end + 1;
            let after = &s[params_start..];
            if let Some(params_end_rel) = after.find(')') {
                let params = &after[..params_end_rel].trim();
                if params.is_empty() {
                    return Some((name, 0));
                }
                // count commas
                let count = params.split(',').count();
                return Some((name, count));
            }
            None
        }

        // Create a module with a single function that either returns a const i64
        // or performs an add of the first two params. This is intentionally small
        // but enough to try the end-to-end flow.
        fn build_module(ir_text: &str) -> Result<inkwell::module::Module, String> {
            // Create a new context and module. Keep all usage local so we don't need
            // to extend lifetimes artificially.
            let context = Context::create();
            let module = context.create_module("jit_module");
            let i64_t = context.i64_type();

            // Try to parse header
            let (name, param_count) = match Self::parse_fn_header(ir_text) {
                Some(v) => v,
                None => ("_anon".to_string(), 0),
            };

            // ABI: first param is always *mut i64 (for the success return value)
            let ptr_type = i64_t.ptr_type(inkwell::AddressSpace::from(0)).into();
            let fn_type = match param_count {
                0 => i64_t.fn_type(&[ptr_type], false),
                n => {
                    let mut params = Vec::with_capacity(n + 1);
                    params.push(ptr_type);
                    for _ in 0..n {
                        params.push(i64_t.into());
                    }
                    i64_t.fn_type(&params, false)
                }
            };

            let function = module.add_function(&name, fn_type, None);
            let bb = context.append_basic_block(function, "entry");
            let builder = context.create_builder();
            builder.position_at_end(bb);

            let out_ptr = function.get_nth_param(0).unwrap().into_pointer_value();
            let status_ok = i64_t.const_zero(); // 0
            let status_deopt = i64_t.const_int(1, false); // 1

            // If IR explicitly contains `deopt`, return early with deopt status
            if ir_text.contains("deopt") {
                builder.build_return(Some(&status_deopt));
                return Ok(module);
            }

            // If IR contains `const i64 <N>` use that
            if let Some(idx) = ir_text.find("const i64") {
                let after = &ir_text[idx + "const i64".len()..];
                if let Some(num_str) = after
                    .split(|c: char| !c.is_numeric() && c != '-')
                    .find(|s| !s.is_empty())
                {
                    if let Ok(v) = num_str.trim().parse::<i64>() {
                        let constv = i64_t.const_int(v as u64, true);
                        builder.build_store(out_ptr, constv);
                        builder.build_return(Some(&status_ok));
                        return Ok(module);
                    }
                }
            }

            // If contains `add` then add first two params (shifted by 1 due to out_ptr)
            if ir_text.contains(" add ") || ir_text.contains("add i64") {
                if param_count >= 2 {
                    let a = function.get_nth_param(1).unwrap().into_int_value();
                    let b = function.get_nth_param(2).unwrap().into_int_value();
                    let sum = builder.build_int_add(a, b, "sum");
                    builder.build_store(out_ptr, sum);
                    builder.build_return(Some(&status_ok));
                    return Ok(module);
                }
            }

            if ir_text.contains(" sub ") || ir_text.contains("sub i64") {
                if param_count >= 2 {
                    let a = function.get_nth_param(1).unwrap().into_int_value();
                    let b = function.get_nth_param(2).unwrap().into_int_value();
                    let diff = builder.build_int_sub(a, b, "diff");
                    builder.build_store(out_ptr, diff);
                    builder.build_return(Some(&status_ok));
                    return Ok(module);
                }
            }

            if ir_text.contains("br_cond") {
                if let Some(line) = ir_text.lines().find(|l| l.contains("br_cond")) {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        let then_label = parts[2].trim_end_matches(',');
                        let else_label = parts[3].trim_end_matches(',');

                        let then_bb = context.append_basic_block(function, then_label);
                        let else_bb = context.append_basic_block(function, else_label);
                        let merge_label = "f_merge";
                        let merge_bb = context.append_basic_block(function, merge_label);

                        builder.position_at_end(context.append_basic_block(function, "entry_tmp"));
                        builder.build_unconditional_branch(then_bb);

                        builder.position_at_end(then_bb);
                        if let Some(pos) = ir_text.find(&format!("{}:", then_label)) {
                            let sub = &ir_text[pos..];
                            if let Some(idx) = sub.find("const i64") {
                                if let Some(num_str) = sub[idx + "const i64".len()..]
                                    .split(|c: char| !c.is_numeric() && c != '-')
                                    .find(|s| !s.is_empty())
                                {
                                    if let Ok(_v) = num_str.trim().parse::<i64>() {
                                        builder.build_unconditional_branch(merge_bb);
                                    }
                                }
                            }
                        }

                        builder.position_at_end(else_bb);
                        if let Some(pos) = ir_text.find(&format!("{}:", else_label)) {
                            let sub = &ir_text[pos..];
                            if let Some(idx) = sub.find("const i64") {
                                if let Some(num_str) = sub[idx + "const i64".len()..]
                                    .split(|c: char| !c.is_numeric() && c != '-')
                                    .find(|s| !s.is_empty())
                                {
                                    if let Ok(_v) = num_str.trim().parse::<i64>() {
                                        builder.build_unconditional_branch(merge_bb);
                                    }
                                }
                            }
                        }

                        builder.position_at_end(merge_bb);
                        let mut then_val = i64_t.const_zero();
                        let mut else_val = i64_t.const_zero();
                        if let Some(pos) = ir_text.find(&format!("{}:\n", then_label)) {
                            let sub = &ir_text[pos..];
                            if let Some(idx) = sub.find("const i64") {
                                if let Some(num_str) = sub[idx + "const i64".len()..]
                                    .split(|c: char| !c.is_numeric() && c != '-')
                                    .find(|s| !s.is_empty())
                                {
                                    if let Ok(v) = num_str.trim().parse::<i64>() {
                                        then_val = i64_t.const_int(v as u64, true);
                                    }
                                }
                            }
                        }
                        if let Some(pos) = ir_text.find(&format!("{}:\n", else_label)) {
                            let sub = &ir_text[pos..];
                            if let Some(idx) = sub.find("const i64") {
                                if let Some(num_str) = sub[idx + "const i64".len()..]
                                    .split(|c: char| !c.is_numeric() && c != '-')
                                    .find(|s| !s.is_empty())
                                {
                                    if let Ok(v) = num_str.trim().parse::<i64>() {
                                        else_val = i64_t.const_int(v as u64, true);
                                    }
                                }
                            }
                        }

                        let phi = builder.build_phi(i64_t, "phi_tmp");
                        phi.add_incoming(&[(&then_val, then_bb), (&else_val, else_bb)]);
                        builder.build_store(out_ptr, phi.as_basic_value().into_int_value());
                        builder.build_return(Some(&status_ok));
                        return Ok(module);
                    }
                }
            }

            // Default: return 0
            let zero = i64_t.const_zero();
            builder.build_store(out_ptr, zero);
            builder.build_return(Some(&status_ok));
            Ok(module)
        }
    }

    impl LlvmBuilder for InkwellLlvmBuilder {
        fn initialize() -> Result<(), String> {
            Ok(())
        }

        fn lower_ir_to_module(ir_text: &str) -> Result<String, String> {
            let hash = crate::cache::ArtCache::compute_hash(ir_text);
            let cache = crate::cache::ArtCache::new();
            if let Some(cached) = cache.get("llvm", &hash, "ll") {
                GLOBAL_JIT_REGISTRY.lock().unwrap().push(cached.clone());
                return Ok(cached);
            }

            let module = Self::build_module(ir_text)?;
            let s = module.print_to_string().to_string();
            GLOBAL_JIT_REGISTRY.lock().unwrap().push(s.clone());
            cache.set("llvm", &hash, "ll", &s);
            Ok(s)
        }

        fn compile_module_get_symbol(module_text: &str, name: &str) -> Result<usize, String> {
            // Find a persisted module text or build a new one
            // Build a fresh module and create an execution engine to lookup symbol.
            let m = Self::build_module(module_text)?;
            match m.create_jit_execution_engine(OptimizationLevel::None) {
                Ok(engine) => match engine.get_function_address(name) {
                    Ok(addr) => Ok(addr as usize),
                    Err(e) => Err(format!("symbol {} not found: {:?}", name, e)),
                },
                Err(e) => Err(format!("failed to create execution engine: {:?}", e)),
            }
        }
    }

    // Re-export the concrete builder so higher-level code can use it when the
    // feature is enabled.
    pub use InkwellLlvmBuilder as ActiveLlvmBuilder;
}

#[cfg(feature = "jit")]
pub use enabled::ActiveLlvmBuilder as LlvmBuilderImpl;
