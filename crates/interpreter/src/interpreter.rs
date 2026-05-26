use crate::type_registry::TypeRegistry;
use crate::values::{Result, RuntimeError};
use core::ast::{ArtValue, Expr, Function, ObjHandle, Program, Stmt};
use core::environment::Environment;
use diagnostics::{Diagnostic, DiagnosticKind, Span};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::Arc;


/// Computes the Levenshtein distance between two strings
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let mut d = vec![vec![0; b_chars.len() + 1]; a_chars.len() + 1];

    for i in 0..=a_chars.len() {
        d[i][0] = i;
    }
    for j in 0..=b_chars.len() {
        d[0][j] = j;
    }

    for i in 1..=a_chars.len() {
        for j in 1..=b_chars.len() {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
        }
    }
    d[a_chars.len()][b_chars.len()]
}

/// Helper to find the closest match from an iterator of strings
fn did_you_mean<'a>(target: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let mut best_match = None;
    let mut best_dist = usize::MAX;

    for cand in candidates {
        let dist = levenshtein(target, cand);
        // Only consider it a typo if distance is less than a certain threshold
        // e.g., max distance of 3 allows up to 3 insertions/deletions/substitutions
        if dist < best_dist && dist <= 3 {
            best_dist = dist;
            best_match = Some(cand);
        }
    }
    best_match
}
pub mod actors;
pub use actors::{ActorState, Mailbox, decode_val, encode_val};
pub mod builtins;
pub mod cycle_detection;
pub use cycle_detection::{CycleDetectionResult, CycleInfo, CycleReport};
pub mod gc;
pub mod exec;
pub mod eval;

#[cfg(test)]
pub mod test_helpers;

pub struct Interpreter {
    environment: Rc<RefCell<Environment>>,
    type_registry: TypeRegistry,
    pure_mode: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub last_value: Option<ArtValue>,
    pub handled_errors: usize,
    pub executed_statements: usize,
    heap_objects: HashMap<u64, crate::heap::HeapObject>,
    next_heap_id: u64,
    next_capability_id: u64,
    // Métricas de memória (protótipo)
    pub weak_created: usize,
    pub weak_upgrades: usize,
    pub weak_dangling: usize,
    pub unowned_created: usize,
    pub unowned_dangling: usize,
    pub cycle_reports_run: Cell<usize>,
    pub cycle_leaks_detected: usize,
    pub strong_increments: usize,
    pub strong_decrements: usize,
    pub objects_finalized: usize,
    // Per-arena finalized objects counter (experimental)
    pub objects_finalized_per_arena: std::collections::HashMap<u32, usize>,
    // New metrics / debug helpers
    pub finalizer_promotions: usize,
    // Perfil: contadores simples por função name (hotness)
    pub call_counters: std::collections::HashMap<String, u64>,
    // Perfil: contadores de arestas (caller -> callee) para PGO simples
    pub edge_counters: std::collections::HashMap<String, u64>,
    // runtime stack of currently executing named functions (None for top-level)
    pub fn_stack: Vec<Option<String>>,
    // Per-arena allocation counters (experimental)
    pub arena_alloc_count: std::collections::HashMap<u32, usize>,
    // Per-arena promotions counter (experimental)
    pub finalizer_promotions_per_arena: std::collections::HashMap<u32, usize>,
    // transient: currently finalizing arena id to attribute promotions
    pub current_finalizing_arena: Option<u32>,
    pub tracer: Option<crate::tracer::Tracer>,
    pub replayer: Option<crate::replayer::Replayer>,
    pub debug_mode: bool,
    pub fast_forward_until: Option<usize>,
    pub invariant_checks: bool,
    finalizers: HashMap<u64, Rc<Function>>, // finalizers por objeto composto
    // Arena support
    pub current_arena: Option<u32>,
    pub next_arena_id: u32,
    pub in_performant_block: bool,
    pub arena_with_active: bool,
    pub reusable_arenas: std::collections::HashSet<u32>,
    // Actor support (Fase 9 MVP)
    pub actors: HashMap<u32, ActorState>,
    pub next_actor_id: u32,
    // Currently running actor id (set by scheduler during actor execution)
    pub current_actor: Option<u32>,
    // Default mailbox limit (simple global backpressure setting for MVP)
    pub actor_mailbox_limit: usize,
    // Temporarily holds the actor state being executed by the scheduler so builtins
    // that need to access the running actor can find it even while the actor is
    // removed from `actors` to avoid mutable borrow conflicts.
    pub executing_actor: Option<ActorState>,
    // Random State (LCG)
    pub rng_state: u64,
    // Recursion depth guard for evaluate() — prevents stack overflow on pathological AST inputs
    eval_depth: usize,
    // Pilha de arenas para ARC Adaptativo Implícito
    pub arena_stack: Vec<u32>,
    // Span of the most recent field-access call site, used by builtins for error reporting
    pub call_span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ast::ArtValue;
    use std::rc::Rc;

    #[test]
    fn atomic_add_overflow_emits_diag() {
        let mut interp = Interpreter::new();
        let hv = interp.heap_create_atomic(ArtValue::Int(i64::MAX - 1));
        if let ArtValue::Atomic(h) = hv {
            let res = interp.heap_atomic_add(h, 10);
            assert!(res.is_none());
            let diags = interp.take_diagnostics();
            assert!(diags.iter().any(|d| d.message.contains("overflow")));
        } else {
            panic!("expected atomic handle");
        }
    }

    #[test]
    fn finalizer_skipped_for_atomic_and_mutex() {
        let mut interp = Interpreter::new();
        let a = interp.heap_create_atomic(ArtValue::Int(1));
        let m = interp.heap_create_mutex(ArtValue::Int(2));
        if let ArtValue::Atomic(h) = a {
            interp.finalizers.insert(
                h.0,
                Rc::new(Function {
                    name: Some("f".to_string()),
                    type_params: None,
                    params: vec![],
                    body: Rc::new(Stmt::Block { statements: vec![] }),
                    closure: std::rc::Weak::new(),
                    retained_env: None,
                }),
            );
        }
        if let ArtValue::Mutex(h) = m {
            interp.finalizers.insert(
                h.0,
                Rc::new(Function {
                    name: Some("g".to_string()),
                    type_params: None,
                    params: vec![],
                    body: Rc::new(Stmt::Block { statements: vec![] }),
                    closure: std::rc::Weak::new(),
                    retained_env: None,
                }),
            );
        }
        for id in interp.heap_objects.keys().cloned().collect::<Vec<u64>>() {
            interp.force_heap_strong_to_one(id);
            interp.dec_object_strong_recursive(id);
        }
        let diags = interp.take_diagnostics();
        // ensure we did not add a runtime diag complaining about finalizer execution (skip is allowed)
        assert!(
            !diags
                .iter()
                .any(|d| d.message.contains("Finalizer skipped"))
        );
    }

    #[test]
    fn write_profile_emits_functions_and_edges() {
        let mut interp = Interpreter::new();
        // simulate two functions and some edges
        interp.call_counters.insert("foo".to_string(), 5);
        interp.call_counters.insert("bar".to_string(), 2);
        interp.edge_counters.insert("<root>->foo".to_string(), 3);
        interp.edge_counters.insert("foo->bar".to_string(), 4);
        let tmp = std::env::temp_dir().join("art_profile_test.json");
        let _ = interp.write_profile(&tmp).expect("write profile");
        let s = std::fs::read_to_string(&tmp).expect("read profile");
        let v: serde_json::Value = serde_json::from_str(&s).expect("parse profile json");
        assert!(v.get("functions").is_some());
        assert!(v.get("edges").is_some());
        // New: also emit a compact edges_map object
        assert!(v.get("edges_map").is_some());
        // cleanup
        let _ = std::fs::remove_file(&tmp);
    }
}


thread_local! {
    pub(crate) static PRELUDE_VALUES: HashMap<&'static str, ArtValue> = {
        let mut m = HashMap::with_capacity(Interpreter::PRELUDE_NAMES.len());
        for &name in Interpreter::PRELUDE_NAMES {
            m.insert(name, ArtValue::Builtin(Interpreter::name_to_builtin(name)));
        }
        m
    };

    static PRELUDE_TYPES: TypeRegistry = {
        let mut registry = TypeRegistry::new();
        use core::Token;
        let name = Token::dummy("Result");
        let variants = vec![
            (Token::dummy("Ok"), Some(vec!["T".to_string()])),
            (Token::dummy("Err"), Some(vec!["E".to_string()])),
        ];
        registry.register_enum(name, variants);
        let opt_name = Token::dummy("Option");
        let opt_variants = vec![
            (Token::dummy("Some"), Some(vec!["T".to_string()])),
            (Token::dummy("None"), None),
        ];
        registry.register_enum(opt_name, opt_variants);
        registry.register_struct(
            Token::dummy("Envelope"),
            vec![
                (Token::dummy("sender"), "Optional<Int>".to_string()),
                (Token::dummy("payload"), "Any".to_string()),
                (Token::dummy("priority"), "Int".to_string()),
            ],
        );
        registry
    };
}

impl Interpreter {
    pub const PRELUDE_NAMES: &'static [&'static str] = &[
        "println",
        "len",
        "type_of",
        "weak",
        "weak_get",
        "unowned",
        "unowned_get",
        "on_finalize",
        "actor_send",
        "actor_receive",
        "actor_receive_envelope",
        "actor_yield",
        "actor_set_mailbox_limit",
        "envelope",
        "make_envelope",
        "run_actors",
        "atomic_new",
        "atomic_load",
        "atomic_store",
        "atomic_add",
        "mutex_new",
        "mutex_lock",
        "mutex_unlock",
        "arena_new",
        "arena_release",
        "arena_with",
        "idl_schema",
        "idl_validate",
        "buffer_new",
        "serialize",
        "deserialize",
        "capability_acquire",
        "capability_kind",
        "map_new",
        "map_set",
        "map_get",
        "map_has",
        "set_new",
        "set_add",
        "set_has",
        "math_abs",
        "math_pow",
        "math_clamp",
        "dag_topo_sort",
        "time_now",
        "io_read_text",
        "io_write_text",
        "http_get_text",
        "rand_seed",
        "rand_next",
        "stream",
        "map",
        "filter",
        "collect",
        "count",
        "gc_stats",
        "str_split",
        "str_join",
        "str_contains",
        "str_starts_with",
        "str_replace",
        "str_slice",
        "str_to_int",
        "str_to_float",
    ];

    #[inline]
    fn name_to_builtin(name: &str) -> core::ast::BuiltinFn {
        use core::ast::BuiltinFn;
        match name {
            "println" => BuiltinFn::Println,
            "len" => BuiltinFn::Len,
            "type_of" => BuiltinFn::TypeOf,
            "weak" => BuiltinFn::WeakNew,
            "weak_get" => BuiltinFn::WeakGet,
            "unowned" => BuiltinFn::UnownedNew,
            "unowned_get" => BuiltinFn::UnownedGet,
            "on_finalize" => BuiltinFn::OnFinalize,
            "actor_send" => BuiltinFn::ActorSend,
            "actor_receive" => BuiltinFn::ActorReceive,
            "actor_receive_envelope" => BuiltinFn::ActorReceiveEnvelope,
            "actor_yield" => BuiltinFn::ActorYield,
            "actor_set_mailbox_limit" => BuiltinFn::ActorSetMailboxLimit,
            "envelope" => BuiltinFn::EnvelopeNew,
            "make_envelope" => BuiltinFn::MakeEnvelope,
            "run_actors" => BuiltinFn::RunActors,
            "atomic_new" => BuiltinFn::AtomicNew,
            "atomic_load" => BuiltinFn::AtomicLoad,
            "atomic_store" => BuiltinFn::AtomicStore,
            "atomic_add" => BuiltinFn::AtomicAdd,
            "mutex_new" => BuiltinFn::MutexNew,
            "mutex_lock" => BuiltinFn::MutexLock,
            "mutex_unlock" => BuiltinFn::MutexUnlock,
            "arena_new" => BuiltinFn::ArenaNew,
            "arena_release" => BuiltinFn::ArenaRelease,
            "arena_with" => BuiltinFn::ArenaWith,
            "idl_schema" => BuiltinFn::IdlSchema,
            "idl_validate" => BuiltinFn::IdlValidate,
            "buffer_new" => BuiltinFn::BufferNew,
            "serialize" => BuiltinFn::Serialize,
            "deserialize" => BuiltinFn::Deserialize,
            "capability_acquire" => BuiltinFn::CapabilityAcquire,
            "capability_kind" => BuiltinFn::CapabilityKind,
            "map_new" => BuiltinFn::MapNew,
            "map_set" => BuiltinFn::MapSet,
            "map_get" => BuiltinFn::MapGet,
            "map_has" => BuiltinFn::MapHas,
            "set_new" => BuiltinFn::SetNew,
            "set_add" => BuiltinFn::SetAdd,
            "set_has" => BuiltinFn::SetHas,
            "math_abs" => BuiltinFn::MathAbs,
            "math_pow" => BuiltinFn::MathPow,
            "math_clamp" => BuiltinFn::MathClamp,
            "dag_topo_sort" => BuiltinFn::DagTopoSort,
            "time_now" => BuiltinFn::TimeNow,
            "io_read_text" => BuiltinFn::IOReadText,
            "io_write_text" => BuiltinFn::IOWriteText,
            "http_get_text" => BuiltinFn::HttpGetText,
            "rand_seed" => BuiltinFn::RandomSeed,
            "rand_next" => BuiltinFn::RandomNext,
            "stream" => BuiltinFn::StreamNew,
            "map" => BuiltinFn::StreamMap,
            "filter" => BuiltinFn::StreamFilter,
            "collect" => BuiltinFn::StreamCollect,
            "count" => BuiltinFn::StreamCount,
            "gc_stats" => BuiltinFn::GCStats,
            "str_split" => BuiltinFn::StrSplit,
            "str_join" => BuiltinFn::StrJoin,
            "str_contains" => BuiltinFn::StrContains,
            "str_starts_with" => BuiltinFn::StrStartsWith,
            "str_replace" => BuiltinFn::StrReplace,
            "str_slice" => BuiltinFn::StrSlice,
            "str_to_int" => BuiltinFn::StrToInt,
            "str_to_float" => BuiltinFn::StrToFloat,
            _ => unreachable!("Unknown builtin name: {}", name),
        }
    }
    pub fn new() -> Self {
        let global_env = PRELUDE_VALUES.with(|prelude| {
            Rc::new(RefCell::new(Environment::with_values(
                None,
                prelude.clone(),
                0,
                None,
            )))
        });

        Interpreter {
            environment: global_env,
            type_registry: TypeRegistry::new(),
            pure_mode: false,
            diagnostics: Vec::new(),
            last_value: None,
            handled_errors: 0,
            executed_statements: 0,
            heap_objects: HashMap::with_capacity(512),
            next_heap_id: 1,
            next_capability_id: 1,
            weak_created: 0,
            weak_upgrades: 0,
            weak_dangling: 0,
            unowned_created: 0,
            unowned_dangling: 0,
            cycle_reports_run: Cell::new(0),
            cycle_leaks_detected: 0,
            strong_increments: 0,
            strong_decrements: 0,
            objects_finalized: 0,
            objects_finalized_per_arena: std::collections::HashMap::new(),
            finalizer_promotions: 0,
            call_counters: std::collections::HashMap::new(),
            edge_counters: std::collections::HashMap::new(),
            fn_stack: Vec::new(),
            arena_alloc_count: std::collections::HashMap::new(),
            finalizer_promotions_per_arena: std::collections::HashMap::new(),
            current_finalizing_arena: None,
            tracer: None,
            replayer: None,
            debug_mode: false,
            fast_forward_until: None,
            invariant_checks: false,
            finalizers: HashMap::new(),
            current_arena: None,
            next_arena_id: 1,
            in_performant_block: false,
            arena_with_active: false,
            reusable_arenas: std::collections::HashSet::new(),
            actors: HashMap::new(),
            next_actor_id: 1,
            current_actor: None,
            actor_mailbox_limit: 1000,
            executing_actor: None,
            rng_state: 0x12345678, // deterministic for v0.2.0 testing
            eval_depth: 0,
            arena_stack: Vec::new(),
            call_span: Span::new(0, 0, 0, 0),
        }
    }

    pub fn with_prelude() -> Self {
        let mut interp = Self::new();
        interp.type_registry = PRELUDE_TYPES.with(|types| types.clone());
        interp
    }

    pub fn set_pure_mode(&mut self, pure: bool) {
        self.pure_mode = pure;
    }

    fn ensure_pure_allowed(&mut self, op_name: &str) -> bool {
        if self.pure_mode {
            self.diagnostics.push(Diagnostic::new(
                DiagnosticKind::Runtime,
                format!("Operation '{}' is not allowed in --pure mode", op_name),
                Span::new(0, 0, 0, 0),
            ));
            false
        } else {
            true
        }
    }

    fn runtime_type_label(&self, value: &ArtValue) -> String {
        let resolved = self.resolve_composite(value);
        match resolved {
            ArtValue::Int(_) => "Int".to_string(),
            ArtValue::Float(_) => "Float".to_string(),
            ArtValue::Bool(_) => "Bool".to_string(),
            ArtValue::String(_) => "String".to_string(),
            ArtValue::Array(_) => "Array".to_string(),
            ArtValue::Tuple(_) => "Tuple".to_string(),
            ArtValue::Optional(_) => "Optional".to_string(),
            ArtValue::StructInstance { struct_name, .. } => struct_name.clone(),
            ArtValue::EnumInstance { enum_name, .. } => enum_name.clone(),
            ArtValue::Function(_) => "Function".to_string(),
            ArtValue::Builtin(_) => "Builtin".to_string(),
            ArtValue::WeakRef(_) => "WeakRef".to_string(),
            ArtValue::UnownedRef(_) => "UnownedRef".to_string(),
            ArtValue::Atomic(_) => "Atomic".to_string(),
            ArtValue::Mutex(_) => "Mutex".to_string(),
            ArtValue::Actor(_) => "Actor".to_string(),
            ArtValue::Map(_) => "Map".to_string(),
            ArtValue::Set(_) => "Set".to_string(),
            ArtValue::Capability { kind, .. } => format!("Capability[{}]", kind),
            ArtValue::MovedCapability => "MovedCapability".to_string(),
            ArtValue::HeapComposite(_) => "Composite".to_string(),
            ArtValue::Buffer(_) => "Buffer".to_string(),
        }
    }

    fn value_matches_declared_type(&self, value: &ArtValue, expected_type: &str) -> bool {
        let expected = expected_type.trim();
        if expected.is_empty() || expected == "Any" {
            return true;
        }

        let resolved = self.resolve_composite(value);
        match expected {
            "Int" => matches!(resolved, ArtValue::Int(_)),
            "Float" => matches!(resolved, ArtValue::Float(_)),
            "Bool" => matches!(resolved, ArtValue::Bool(_)),
            "String" => matches!(resolved, ArtValue::String(_)),
            "Array" => matches!(resolved, ArtValue::Array(_)),
            "Tuple" => matches!(resolved, ArtValue::Tuple(_)),
            _ => {
                if expected.starts_with("Optional<") && expected.ends_with('>') {
                    let inner = &expected[9..expected.len() - 1];
                    return match resolved {
                        ArtValue::Optional(opt) => match &**opt {
                            Some(v) => self.value_matches_declared_type(v, inner),
                            None => true,
                        },
                        ArtValue::EnumInstance {
                            enum_name,
                            variant,
                            values,
                        } if enum_name == "Option" => {
                            if variant == "None" {
                                true
                            } else if variant == "Some" {
                                values
                                    .first()
                                    .map(|v| self.value_matches_declared_type(v, inner))
                                    .unwrap_or(false)
                            } else {
                                false
                            }
                        }
                        _ => false,
                    };
                }

                if expected.starts_with("Array<") && expected.ends_with('>') {
                    let inner = &expected[6..expected.len() - 1];
                    return match resolved {
                        ArtValue::Array(items) => items
                            .iter()
                            .all(|item| self.value_matches_declared_type(item, inner)),
                        _ => false,
                    };
                }

                if expected.starts_with("Capability[") && expected.ends_with(']') {
                    let inner = &expected[11..expected.len() - 1];
                    return match resolved {
                        ArtValue::Capability { kind, .. } => kind.as_ref() == inner,
                        _ => false,
                    };
                }
                if expected.starts_with("Capability<") && expected.ends_with('>') {
                    let inner = &expected[11..expected.len() - 1];
                    return match resolved {
                        ArtValue::Capability { kind, .. } => kind.as_ref() == inner,
                        _ => false,
                    };
                }

                match resolved {
                    ArtValue::StructInstance { struct_name, .. } => struct_name == expected,
                    ArtValue::EnumInstance { enum_name, .. } => enum_name == expected,
                    _ => false,
                }
            }
        }
    }

    fn build_idl_schema_map(&self, struct_name: &str) -> Option<ArtValue> {
        let struct_def = self.type_registry.get_struct(struct_name)?;
        let mut map = std::collections::HashMap::new();
        for (field_name, field_ty) in &struct_def.fields {
            map.insert(
                field_name.clone(),
                ArtValue::String(Arc::from(field_ty.clone())),
            );
        }
        Some(ArtValue::Map(core::ast::MapRef(std::sync::Arc::new(
            std::sync::Mutex::new(map),
        ))))
    }

    fn run_shell_stages(
        &mut self,
        program: &str,
        args: &[String],
    ) -> std::result::Result<std::process::Output, String> {
        let mut stages: Vec<Vec<String>> = Vec::new();
        let mut current = vec![program.to_string()];
        for arg in args {
            if arg == "|>" {
                if current.is_empty() {
                    return Err("Empty shell pipeline stage before '|>'".to_string());
                }
                stages.push(current);
                current = Vec::new();
            } else {
                current.push(arg.clone());
            }
        }
        if !current.is_empty() {
            stages.push(current);
        }
        if stages.is_empty() {
            return Err("Shell command is empty".to_string());
        }

        let mut piped_input: Option<Vec<u8>> = None;
        let mut last_output: Option<std::process::Output> = None;

        for stage in stages {
            if stage.is_empty() {
                return Err("Empty shell pipeline stage".to_string());
            }
            let cmd = &stage[0];
            let cmd_args = &stage[1..];

            if let Some(input_bytes) = piped_input.take() {
                let mut child = Command::new(cmd)
                    .args(cmd_args)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .map_err(|e| format!("Failed to spawn shell stage '{}': {}", cmd, e))?;

                if let Some(stdin) = child.stdin.as_mut() {
                    stdin
                        .write_all(&input_bytes)
                        .map_err(|e| format!("Failed to write to stdin of '{}': {}", cmd, e))?;
                }

                let output = child
                    .wait_with_output()
                    .map_err(|e| format!("Failed to wait shell stage '{}': {}", cmd, e))?;
                piped_input = Some(output.stdout.clone());
                last_output = Some(output);
            } else {
                let output = Command::new(cmd)
                    .args(cmd_args)
                    .output()
                    .map_err(|e| format!("Failed to run shell command '{}': {}", cmd, e))?;
                piped_input = Some(output.stdout.clone());
                last_output = Some(output);
            }
        }

        last_output.ok_or_else(|| "Shell pipeline produced no output".to_string())
    }

    fn shell_result_ok(stdout: String) -> ArtValue {
        ArtValue::EnumInstance {
            enum_name: "Result".to_string(),
            variant: "Ok".to_string(),
            values: vec![ArtValue::String(Arc::from(stdout))],
        }
    }

    fn shell_result_err(stderr: String) -> ArtValue {
        ArtValue::EnumInstance {
            enum_name: "Result".to_string(),
            variant: "Err".to_string(),
            values: vec![ArtValue::String(Arc::from(stderr))],
        }
    }

    fn publish_shell_result(&mut self, result: ArtValue) {
        self.last_value = Some(result.clone());
        self.environment.borrow_mut().define("shell_result", result);
    }

    fn run_shell_function_call(&mut self, program: &str, arguments: Vec<Expr>) -> Result<ArtValue> {
        if !self.ensure_pure_allowed("shell") {
            let blocked = Self::shell_result_err(
                "Operation 'shell' is not allowed in --pure mode".to_string(),
            );
            self.publish_shell_result(blocked.clone());
            return Ok(blocked);
        }

        let mut args = Vec::new();
        for arg in arguments {
            let v = self.evaluate(arg)?;
            match v {
                ArtValue::String(s) => args.push(s.to_string()),
                other => args.push(other.to_string()),
            }
        }

        let result = match self.run_shell_stages(program, &args) {
            Ok(output) => {
                if output.status.success() {
                    Self::shell_result_ok(String::from_utf8_lossy(&output.stdout).to_string())
                } else if output.stderr.is_empty() {
                    Self::shell_result_err(format!(
                        "Shell command '{}' exited with status {:?}",
                        program,
                        output.status.code()
                    ))
                } else {
                    Self::shell_result_err(String::from_utf8_lossy(&output.stderr).to_string())
                }
            }
            Err(e) => {
                self.diagnostics.push(Diagnostic::new(
                    DiagnosticKind::Runtime,
                    e.clone(),
                    Span::new(0, 0, 0, 0),
                ));
                Self::shell_result_err(e)
            }
        };

        self.publish_shell_result(result.clone());
        Ok(result)
    }

    fn decode_stream_value(
        &self,
        value: ArtValue,
    ) -> std::result::Result<(Vec<ArtValue>, Vec<ArtValue>), String> {
        let resolved = self.resolve_composite(&value).clone();
        if let ArtValue::StructInstance {
            struct_name,
            fields,
        } = resolved
        {
            if struct_name != "__Stream" {
                return Err("Expected stream value".to_string());
            }
            let source = match fields.get("source") {
                Some(ArtValue::Array(v)) => v.clone(),
                _ => return Err("Malformed stream: missing source array".to_string()),
            };
            let ops = match fields.get("ops") {
                Some(ArtValue::Array(v)) => v.clone(),
                _ => return Err("Malformed stream: missing ops array".to_string()),
            };
            Ok((source, ops))
        } else {
            Err("Expected stream value".to_string())
        }
    }

    fn build_stream_value(&self, source: Vec<ArtValue>, ops: Vec<ArtValue>) -> ArtValue {
        let mut fields = HashMap::new();
        fields.insert("source".to_string(), ArtValue::Array(source));
        fields.insert("ops".to_string(), ArtValue::Array(ops));
        ArtValue::StructInstance {
            struct_name: "__Stream".to_string(),
            fields,
        }
    }

    fn stream_op(op_name: &str, callable: ArtValue) -> ArtValue {
        ArtValue::Tuple(vec![
            ArtValue::String(Arc::from(op_name.to_string())),
            callable,
        ])
    }

    fn invoke_callable_with_values(
        &mut self,
        callable: ArtValue,
        args: Vec<ArtValue>,
    ) -> Result<ArtValue> {
        let arg_exprs: Vec<Expr> = args.into_iter().map(Expr::Literal).collect();
        match callable {
            ArtValue::Function(func) => self.call_function(func, None, arg_exprs),
            ArtValue::Builtin(b) => self.call_builtin(b, arg_exprs),
            _ => {
                self.diagnostics.push(Diagnostic::new(
                    DiagnosticKind::Runtime,
                    "Stream operation expects a callable argument".to_string(),
                    Span::new(0, 0, 0, 0),
                ));
                Ok(ArtValue::none())
            }
        }
    }

    fn run_stream_pipeline(
        &mut self,
        source: Vec<ArtValue>,
        ops: Vec<ArtValue>,
    ) -> Result<Vec<ArtValue>> {
        let mut out = Vec::new();
        'outer: for mut item in source {
            for op in &ops {
                match op {
                    ArtValue::Tuple(parts) if parts.len() == 2 => {
                        let op_name = match &parts[0] {
                            ArtValue::String(s) => s.as_ref(),
                            _ => {
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    "Malformed stream op: invalid operation name".to_string(),
                                    Span::new(0, 0, 0, 0),
                                ));
                                return Ok(Vec::new());
                            }
                        };
                        let callable = parts[1].clone();
                        match op_name {
                            "map" => {
                                item = self.invoke_callable_with_values(callable, vec![item])?;
                            }
                            "filter" => {
                                let keep =
                                    self.invoke_callable_with_values(callable, vec![item.clone()])?;
                                if !self.is_truthy(&keep) {
                                    continue 'outer;
                                }
                            }
                            _ => {
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    format!("Unsupported stream operation '{}'", op_name),
                                    Span::new(0, 0, 0, 0),
                                ));
                                return Ok(Vec::new());
                            }
                        }
                    }
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "Malformed stream operation payload".to_string(),
                            Span::new(0, 0, 0, 0),
                        ));
                        return Ok(Vec::new());
                    }
                }
            }
            out.push(item);
        }
        Ok(out)
    }

    /// Exposto para testes / prototipagem: registra struct dinâmica.
    pub fn register_struct_for_test(&mut self, name: &str, fields: Vec<(core::Token, String)>) {
        self.type_registry
            .register_struct(core::Token::dummy(name), fields);
    }

    pub fn interpret(&mut self, program: Program) -> Result<()> {
        self.last_value = None;
        for statement in program {
            if let Err(RuntimeError::Return(_)) = self.execute(statement) {
                break;
            }
        }
        Ok(())
    }
    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        self.handled_errors += self.diagnostics.len();
        std::mem::take(&mut self.diagnostics)
    }

    // --- Heap helpers (protótipo Fase 8) ---
    fn heap_register(&mut self, val: ArtValue) -> u64 {
        let id = self.next_heap_id;
        self.next_heap_id += 1;
        self.heap_objects
            .insert(id, crate::heap::HeapObject::new(id, val.clone()));
        // Ensure children strong counts are incremented for any composites contained
        // in the registered value so that tests using debug_heap_register mirror
        // real runtime semantics (which call inc_children_strong via heapify).
        self.inc_children_strong(&val);
        id
    }
    fn heap_register_in_arena(&mut self, val: ArtValue, arena_id: u32) -> u64 {
        let id = self.next_heap_id;
        self.next_heap_id += 1;
        self.heap_objects.insert(
            id,
            crate::heap::HeapObject::new_in_arena(id, val.clone(), arena_id),
        );
        // Mirror heap_register behavior for arena-allocated objects as well.
        self.inc_children_strong(&val);
        // record arena allocation
        *self.arena_alloc_count.entry(arena_id).or_insert(0) += 1;
        id
    }
    pub fn debug_create_arena(&mut self) -> u32 {
        (self.next_heap_id as u32).wrapping_add(1)
    }

    fn push_implicit_arena(&mut self) -> u32 {
        let aid = self.next_arena_id;
        self.next_arena_id += 1;
        self.arena_stack.push(aid);
        self.current_arena = Some(aid);
        aid
    }

    fn pop_implicit_arena(&mut self) {
        if let Some(aid) = self.arena_stack.pop() {
            self.finalize_arena(aid);
            self.current_arena = self.arena_stack.last().cloned();
        }
    }

    fn promote_if_escaping(&mut self, target_aid: Option<u32>, value: &mut ArtValue) {
        match value {
            ArtValue::HeapComposite(h) => {
                let mut inner_val = None;
                let mut needs_promotion = false;

                if let Some(obj) = self.heap_objects.get(&h.0) {
                    if let Some(obj_aid) = obj.arena_id {
                        needs_promotion = match target_aid {
                            None => true,                              // escapando para o global
                            Some(ta) => obj_aid != ta && obj_aid > ta, // escapando para pai/global
                        };
                        if needs_promotion {
                            inner_val = Some(obj.value.clone());
                        }
                    }
                }

                if let Some(mut iv) = inner_val {
                    // Deep recursion: promote children first
                    self.promote_if_escaping(target_aid, &mut iv);

                    // Register the promoted object in the target arena
                    let new_id = if let Some(ta) = target_aid {
                        self.heap_register_in_arena(iv, ta)
                    } else {
                        self.heap_register(iv)
                    };

                    // Update current reference to the new promoted ID
                    h.0 = new_id;
                    self.finalizer_promotions += 1;
                }
            }
            ArtValue::Array(arr) => {
                for item in arr.iter_mut() {
                    self.promote_if_escaping(target_aid, item);
                }
            }
            ArtValue::StructInstance { fields, .. } => {
                for item in fields.values_mut() {
                    self.promote_if_escaping(target_aid, item);
                }
            }
            ArtValue::Tuple(tup) => {
                for item in tup.iter_mut() {
                    self.promote_if_escaping(target_aid, item);
                }
            }
            ArtValue::EnumInstance { values, .. } => {
                for item in values.iter_mut() {
                    self.promote_if_escaping(target_aid, item);
                }
            }
            ArtValue::Optional(inner) => {
                if let Some(iv) = inner.as_mut() {
                    self.promote_if_escaping(target_aid, iv);
                }
            }
            _ => {}
        }
    }

    fn heap_upgrade_weak(&self, id: u64) -> Option<ArtValue> {
        self.heap_objects
            .get(&id)
            .and_then(|o| if o.alive { Some(o.value.clone()) } else { None })
    }

    pub fn debug_heap_set(&mut self, id: u64, value: ArtValue) {
        if let Some(obj) = self.heap_objects.get_mut(&id) {
            obj.value = value;
        }
    }

    fn heap_get_unowned(&self, id: u64) -> Option<ArtValue> {
        self.heap_objects
            .get(&id)
            .and_then(|o| if o.alive { Some(o.value.clone()) } else { None })
    }

    #[inline]
    fn is_object_alive(&self, id: u64) -> bool {
        self.heap_objects.get(&id).map(|o| o.alive).unwrap_or(false)
    }

    #[inline]
    fn note_composite_child(&mut self, v: &ArtValue) {
        if matches!(
            v,
            ArtValue::Array(_) | ArtValue::StructInstance { .. } | ArtValue::EnumInstance { .. }
        ) {
            self.strong_increments += 1; // placeholder: ainda não incrementa contador real em heap porque composites não são heap alocados neste estágio
        }
    }

    #[inline]
    fn heapify_composite(&mut self, v: ArtValue) -> ArtValue {
        match v {
            ArtValue::Array(_)
            | ArtValue::StructInstance { .. }
            | ArtValue::EnumInstance { .. } => {
                let id = if let Some(aid) = self.current_arena {
                    self.heap_register_in_arena(v, aid)
                } else {
                    self.heap_register(v)
                };
                // Clona valor armazenado para evitar empréstimo simultâneo (valor geralmente pequeno / compartilhado)
                if let Some(obj) = self.heap_objects.get(&id) {
                    let snapshot = obj.value.clone();
                    self.inc_children_strong(&snapshot);
                }
                ArtValue::HeapComposite(ObjHandle(id))
            }
            other => other,
        }
    }

    /// Create a heap-backed atomic integer and return an ArtValue::Atomic handle.
    fn heap_create_atomic(&mut self, initial: ArtValue) -> ArtValue {
        // store as a StructInstance-like value internally but expose as Atomic handle
        let mut fields = std::collections::HashMap::new();
        fields.insert(
            "kind".to_string(),
            ArtValue::String(std::sync::Arc::from("atomic")),
        );
        fields.insert("value".to_string(), initial);
        let id = if let Some(aid) = self.current_arena {
            self.heap_register_in_arena(
                ArtValue::StructInstance {
                    struct_name: "Atomic".to_string(),
                    fields,
                },
                aid,
            )
        } else {
            self.heap_register(ArtValue::StructInstance {
                struct_name: "Atomic".to_string(),
                fields,
            })
        };
        // mark kind for downstream logic
        if let Some(obj) = self.heap_objects.get_mut(&id) {
            obj.kind = Some(crate::heap::HeapKind::Atomic);
        }
        ArtValue::Atomic(ObjHandle(id))
    }

    fn heap_atomic_load(&self, h: ObjHandle) -> Option<ArtValue> {
        self.heap_objects.get(&h.0).and_then(|obj| {
            if let ArtValue::StructInstance { fields, .. } = &obj.value {
                fields.get("value").cloned()
            } else {
                None
            }
        })
    }

    fn heap_atomic_store(&mut self, h: ObjHandle, val: ArtValue) -> bool {
        if let Some(obj) = self.heap_objects.get_mut(&h.0) {
            if let ArtValue::StructInstance { fields, .. } = &mut obj.value {
                fields.insert("value".to_string(), val);
                return true;
            }
        }
        false
    }

    fn heap_atomic_add(&mut self, h: ObjHandle, delta: i64) -> Option<i64> {
        if let Some(obj) = self.heap_objects.get_mut(&h.0) {
            if let ArtValue::StructInstance { fields, .. } = &mut obj.value {
                match fields.get("value") {
                    Some(ArtValue::Int(curr)) => {
                        if let Some(new) = curr.checked_add(delta) {
                            fields.insert("value".to_string(), ArtValue::Int(new));
                            return Some(new);
                        } else {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                format!("atomic_add: overflow when adding {} to {}", delta, curr),
                                Span::new(0, 0, 0, 0),
                            ));
                            return None;
                        }
                    }
                    Some(other) => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!(
                                "atomic_add: underlying atomic value is not an Int: {:?}",
                                other
                            ),
                            Span::new(0, 0, 0, 0),
                        ));
                        return None;
                    }
                    None => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "atomic_add: atomic has no 'value' field".to_string(),
                            Span::new(0, 0, 0, 0),
                        ));
                        return None;
                    }
                }
            }
        }
        None
    }

    fn heap_create_mutex(&mut self, initial: ArtValue) -> ArtValue {
        let mut fields = std::collections::HashMap::new();
        fields.insert(
            "kind".to_string(),
            ArtValue::String(std::sync::Arc::from("mutex")),
        );
        fields.insert("locked".to_string(), ArtValue::Bool(false));
        fields.insert("value".to_string(), initial);
        let id = if let Some(aid) = self.current_arena {
            self.heap_register_in_arena(
                ArtValue::StructInstance {
                    struct_name: "Mutex".to_string(),
                    fields,
                },
                aid,
            )
        } else {
            self.heap_register(ArtValue::StructInstance {
                struct_name: "Mutex".to_string(),
                fields,
            })
        };
        if let Some(obj) = self.heap_objects.get_mut(&id) {
            obj.kind = Some(crate::heap::HeapKind::Mutex);
        }
        ArtValue::Mutex(ObjHandle(id))
    }

    fn heap_mutex_lock(&mut self, h: ObjHandle) -> bool {
        if let Some(obj) = self.heap_objects.get_mut(&h.0) {
            if let ArtValue::StructInstance { fields, .. } = &mut obj.value {
                match fields.get("locked") {
                    Some(ArtValue::Bool(true)) => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "mutex_lock: mutex already locked".to_string(),
                            Span::new(0, 0, 0, 0),
                        ));
                        return false;
                    }
                    _ => {
                        fields.insert("locked".to_string(), ArtValue::Bool(true));
                        return true;
                    }
                }
            }
        }
        false
    }

    fn heap_mutex_unlock(&mut self, h: ObjHandle) -> bool {
        if let Some(obj) = self.heap_objects.get_mut(&h.0) {
            if let ArtValue::StructInstance { fields, .. } = &mut obj.value {
                match fields.get("locked") {
                    Some(ArtValue::Bool(false)) => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "mutex_unlock: mutex was not locked".to_string(),
                            Span::new(0, 0, 0, 0),
                        ));
                        return false;
                    }
                    _ => {
                        fields.insert("locked".to_string(), ArtValue::Bool(false));
                        return true;
                    }
                }
            }
        }
        false
    }
    /// Finaliza (libera) todos objetos alocados na arena especificada.
    fn finalize_arena(&mut self, arena_id: u32) {
        // Coletar ids vivos pertencentes à arena (ordenados para determinismo)
        let mut ids: Vec<u64> = self
            .heap_objects
            .iter()
            .filter_map(|(id, obj)| {
                if obj.alive && obj.arena_id == Some(arena_id) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        ids.sort_unstable();
        // attribute promotions during finalization to this arena
        let prev_promo_target = self.current_finalizing_arena;
        self.current_finalizing_arena = Some(arena_id);
        for id in ids {
            // Forçar queda de strong para 0 e disparar finalização recursiva
            // limitar o escopo do borrow mutável para evitar conflitos durante a recursão
            // garantir que pelo menos um dec fará com que alive=false
            self.force_heap_strong_to_one(id);
            self.dec_object_strong_recursive(id);
        }
        // Passo de limpeza: remover entradas mortas da arena que já não têm weaks.
        // Fazemos isso em uma segunda etapa para evitar mutabilidade concorrente durante
        // a recursão de finalizadores.
        let dead_ids: Vec<u64> = self
            .heap_objects
            .iter()
            .filter_map(|(id, obj)| {
                if obj.arena_id == Some(arena_id) && !obj.alive && obj.weak == 0 {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for id in &dead_ids {
            if let Some(obj_to_die) = self.heap_objects.get_mut(id) {
                obj_to_die.value = ArtValue::none();
            }
        }
        for id in dead_ids {
            self.heap_objects.remove(&id);
        }
        // Additional stabilization: perform a few sweep passes to remove objects that
        // became dead as a result of finalizer-promoted changes or temporary references.
        // This reduces the chance of leaving transient dead objects referenced only
        // by other dead objects.
        for _ in 0..3 {
            let before = self.heap_objects.len();
            self.debug_sweep_dead();
            if self.heap_objects.len() == before {
                break;
            }
        }
        // restore previous promotion target
        self.current_finalizing_arena = prev_promo_target;
        // Hardening: normalizar invariantes após finalização da arena.
        // Se por alguma razão existirem objetos com strong==0 mas alive==true,
        // marcamos como mortos para que a varredura os remova corretamente.
        for obj in self.heap_objects.values_mut() {
            if obj.strong == 0 && obj.alive {
                obj.alive = false;
            }
        }
        // Executar uma varredura adicional para remover quaisquer objetos mortos
        // que agora não tenham weak refs. Isto evita deixar objetos mortos no heap
        // por causa de finalizadores que fizeram mudanças transientes.
        self.debug_sweep_dead();
    }

    #[inline]
    pub fn resolve_composite<'a>(&'a self, v: &'a ArtValue) -> &'a ArtValue {
        if let ArtValue::HeapComposite(h) = v {
            if let Some(obj) = self.heap_objects.get(&h.0) {
                &obj.value
            } else {
                v
            }
        } else {
            v
        }
    }

    fn normalize_for_serialization(&self, v: &ArtValue) -> ArtValue {
        let resolved = self.resolve_composite(v);
        match resolved {
            ArtValue::Optional(opt) => ArtValue::Optional(Box::new(
                opt.as_ref()
                    .as_ref()
                    .map(|inner| self.normalize_for_serialization(inner)),
            )),
            ArtValue::Array(items) => ArtValue::Array(
                items
                    .iter()
                    .map(|item| self.normalize_for_serialization(item))
                    .collect(),
            ),
            ArtValue::Tuple(items) => ArtValue::Tuple(
                items
                    .iter()
                    .map(|item| self.normalize_for_serialization(item))
                    .collect(),
            ),
            ArtValue::StructInstance {
                struct_name,
                fields,
            } => {
                let mut out = std::collections::HashMap::new();
                for (k, v) in fields {
                    out.insert(k.clone(), self.normalize_for_serialization(v));
                }
                ArtValue::StructInstance {
                    struct_name: struct_name.clone(),
                    fields: out,
                }
            }
            ArtValue::EnumInstance {
                enum_name,
                variant,
                values,
            } => ArtValue::EnumInstance {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                values: values
                    .iter()
                    .map(|v| self.normalize_for_serialization(v))
                    .collect(),
            },
            ArtValue::Map(map_ref) => {
                let map = map_ref.0.lock().unwrap_or_else(|e| e.into_inner());
                let mut out = std::collections::HashMap::new();
                for (k, v) in map.iter() {
                    out.insert(k.clone(), self.normalize_for_serialization(v));
                }
                ArtValue::Map(core::ast::MapRef(std::sync::Arc::new(
                    std::sync::Mutex::new(out),
                )))
            }
            ArtValue::Set(set_ref) => {
                let set = set_ref.0.lock().unwrap_or_else(|e| e.into_inner());
                let out: Vec<ArtValue> = set
                    .iter()
                    .map(|v| self.normalize_for_serialization(v))
                    .collect();
                ArtValue::Set(core::ast::SetRef(std::sync::Arc::new(
                    std::sync::Mutex::new(out),
                )))
            }
            _ => resolved.clone(),
        }
    }


    /// Test helper: define valor no ambiente global
    pub fn debug_define_global(&mut self, name: &str, val: ArtValue) {
        // Mimic the real `let` semantics: if a previous value exists, decrement its heap refs
        let old_opt = self.environment.borrow().get(name);
        if let Some(old) = old_opt {
            self.dec_value_if_heap(&old);
        }
        // define and register strong handle if heap composite (mirror `let`)
        let mut env = self.environment.borrow_mut();
        if let ArtValue::HeapComposite(h) = &val {
            env.strong_handles.push(*h);
        }
        env.define(name, val);
    }

    pub fn get_global(&self, name: &str) -> Option<ArtValue> {
        let env = self.environment.borrow();
        env.get(name)
    }

    pub fn enable_tracer(&mut self, path: &str) -> std::io::Result<()> {
        let tracer = crate::tracer::Tracer::new(path)?;
        self.tracer = Some(tracer);
        Ok(())
    }

    pub fn enable_replayer(&mut self, path: &str) -> std::io::Result<()> {
        let replayer = crate::replayer::Replayer::new(path)?;
        self.replayer = Some(replayer);
        Ok(())
    }

    pub fn set_debug_mode(&mut self, d: bool) {
        self.debug_mode = d;
    }

    fn maybe_record_checkpoint(&mut self) {
        const CHECKPOINT_INTERVAL: usize = 10;
        if self.tracer.is_none() {
            return;
        }
        if self.executed_statements % CHECKPOINT_INTERVAL != 0 {
            return;
        }
        if let Some(tracer) = &mut self.tracer {
            let _ = tracer.record_checkpoint(self.executed_statements, self.rng_state);
        }
    }

    pub fn env_ref(&self) -> Rc<RefCell<Environment>> {
        self.environment.clone()
    }

    pub fn debug_get_global(&self, name: &str) -> Option<ArtValue> {
        self.environment.borrow().get(name)
    }

    // Protótipo: sumariza refs weak/unowned presentes acessíveis do ambiente global.
    pub fn cycle_report(&self) -> CycleReport {
        // Safety: contador mutável requer RefCell ou interior mutability; reaproveitamos via cast mutável temporário
        self.cycle_reports_run.set(self.cycle_reports_run.get() + 1);
        let mut weak_total = 0;
        let mut weak_alive = 0;
        let mut weak_dead = 0;
        let mut unowned_total = 0;
        let mut unowned_dangling = 0;
        fn scan(
            v: &ArtValue,
            this: &Interpreter,
            wt: &mut usize,
            wa: &mut usize,
            wd: &mut usize,
            ut: &mut usize,
            ud: &mut usize,
        ) {
            match v {
                ArtValue::WeakRef(h) => {
                    *wt += 1;
                    if this.is_object_alive(h.0) {
                        *wa += 1
                    } else {
                        *wd += 1
                    }
                }
                ArtValue::UnownedRef(h) => {
                    *ut += 1;
                    if !this.is_object_alive(h.0) {
                        *ud += 1
                    }
                }
                ArtValue::HeapComposite(h) => {
                    if let Some(obj) = this.heap_objects.get(&h.0) {
                        scan(&obj.value, this, wt, wa, wd, ut, ud);
                    }
                }
                ArtValue::Array(a) => {
                    for e in a {
                        scan(e, this, wt, wa, wd, ut, ud)
                    }
                }
                ArtValue::StructInstance { fields, .. } => {
                    for val in fields.values() {
                        scan(val, this, wt, wa, wd, ut, ud)
                    }
                }
                ArtValue::EnumInstance { values, .. } => {
                    for val in values {
                        scan(val, this, wt, wa, wd, ut, ud)
                    }
                }
                _ => {}
            }
        }
        for (_k, v) in self.environment.borrow().values.iter() {
            scan(
                v,
                self,
                &mut weak_total,
                &mut weak_alive,
                &mut weak_dead,
                &mut unowned_total,
                &mut unowned_dangling,
            );
        }
        let mut out_deg_sum = 0usize;
        let mut in_deg_sum = 0usize;
        let mut in_counts: std::collections::HashMap<u64, usize> = std::collections::HashMap::new();
        for obj in self.heap_objects.values() {
            if !obj.alive {
                continue;
            }
            match &obj.value {
                ArtValue::Array(a) => {
                    let mut c = 0;
                    for ch in a {
                        if let ArtValue::HeapComposite(h) = ch
                            && self.is_object_alive(h.0)
                        {
                            c += 1;
                            *in_counts.entry(h.0).or_insert(0) += 1;
                        }
                    }
                    out_deg_sum += c;
                }
                ArtValue::StructInstance { fields, .. } => {
                    let mut c = 0;
                    for ch in fields.values() {
                        if let ArtValue::HeapComposite(h) = ch
                            && self.is_object_alive(h.0)
                        {
                            c += 1;
                            *in_counts.entry(h.0).or_insert(0) += 1;
                        }
                    }
                    out_deg_sum += c;
                }
                ArtValue::EnumInstance { values, .. } => {
                    let mut c = 0;
                    for ch in values {
                        if let ArtValue::HeapComposite(h) = ch
                            && self.is_object_alive(h.0)
                        {
                            c += 1;
                            *in_counts.entry(h.0).or_insert(0) += 1;
                        }
                    }
                    out_deg_sum += c;
                }
                _ => {}
            }
        }
        for (_id, c) in in_counts.iter() {
            in_deg_sum += *c;
        }
        let heap_alive = self.heap_objects.iter().filter(|(_, o)| o.alive).count();
        let (avg_out_degree, avg_in_degree) = if heap_alive > 0 {
            (
                out_deg_sum as f32 / heap_alive as f32,
                in_deg_sum as f32 / heap_alive as f32,
            )
        } else {
            (0.0, 0.0)
        };
        let mut candidate_owner_edges = Vec::new();
        for (id, obj) in self.heap_objects.iter() {
            if !obj.alive {
                continue;
            }
            if let ArtValue::StructInstance { fields, .. } = &obj.value {
                for (fname, val) in fields {
                    let lname = fname.to_lowercase();
                    if (lname.contains("parent") || lname.contains("owner"))
                        && let ArtValue::HeapComposite(h) = val
                        && self.is_object_alive(h.0)
                    {
                        candidate_owner_edges.push((*id, h.0));
                    }
                }
            }
        }
        CycleReport {
            weak_total,
            weak_alive,
            weak_dead,
            unowned_total,
            unowned_dangling,
            objects_finalized: self.objects_finalized,
            heap_alive,
            avg_out_degree,
            avg_in_degree,
            candidate_owner_edges,
        }
    }

    pub fn write_profile(&self, path: &std::path::Path) -> std::result::Result<(), std::io::Error> {
        // Emit both an edges array (backwards compatible) and an edges_map object
        // for easier programmatic consumption.
        let mut out = String::new();
        out.push_str("{\n");
        // functions
        out.push_str("  \"functions\": {\n");
        let mut first = true;
        for (k, v) in &self.call_counters {
            if !first {
                out.push_str(",\n");
            }
            first = false;
            out.push_str(&format!("    \"{}\": {}", k.replace('"', "\\\""), v));
        }
        out.push_str("\n  },\n");

        // edges as array of { caller, callee, count } (backwards compatible)
        out.push_str("  \"edges\": [\n");
        let mut first_e = true;
        for (k, v) in &self.edge_counters {
            if !first_e {
                out.push_str(",\n");
            }
            first_e = false;
            // parse key "caller->callee" into parts
            let parts: Vec<&str> = k.split("->").collect();
            let (caller, callee) = if parts.len() == 2 {
                (parts[0], parts[1])
            } else {
                ("<unknown>", k.as_str())
            };
            out.push_str(&format!(
                "    {{\"caller\": \"{}\", \"callee\": \"{}\", \"count\": {}}}",
                caller.replace('"', "\\\""),
                callee.replace('"', "\\\""),
                v
            ));
        }
        out.push_str("\n  ],\n");

        // edges_map object keyed by "caller->callee" for easy lookup
        out.push_str("  \"edges_map\": {\n");
        let mut first_m = true;
        for (k, v) in &self.edge_counters {
            if !first_m {
                out.push_str(",\n");
            }
            first_m = false;
            out.push_str(&format!("    \"{}\": {}", k.replace('"', "\\\""), v));
        }
        out.push_str("\n  }\n}\n");
        std::fs::write(path, out)
    }


    /// Run actors in a simple round-robin scheduler. Each actor executes at most one
    /// statement per turn. Actors with empty body but non-empty mailbox will be considered runnable
    /// (so user code can `actor_receive()` in the body to consume messages). max_steps limits total turns.
    fn construct_enum_variant(
        &mut self,
        enum_name: String,
        variant: String,
        arguments: Vec<Expr>,
    ) -> Result<ArtValue> {
        let mut evaluated_args = Vec::new();
        for arg in arguments {
            evaluated_args.push(self.evaluate(arg)?);
        }
        Ok(ArtValue::EnumInstance {
            enum_name,
            variant,
            values: evaluated_args,
        })
    }

    fn call_fallback(
        &mut self,
        original_expr: Expr,
        value: ArtValue,
        arguments: &[Expr],
    ) -> Result<ArtValue> {
        if arguments.is_empty()
            && let Expr::FieldAccess { .. } = original_expr
        {
            return Ok(value);
        }
        self.diagnostics.push(Diagnostic::new(
            DiagnosticKind::Runtime,
            format!("'{}' is not a function.", value),
            Span::new(0, 0, 0, 0),
        ));
        Ok(ArtValue::none())
    }

    fn is_truthy(&self, value: &ArtValue) -> bool {
        match value {
            ArtValue::Bool(b) => *b,
            ArtValue::Optional(opt) => opt.is_some(),
            ArtValue::Int(n) => *n != 0,
            ArtValue::Float(f) => *f != 0.0,
            ArtValue::String(s) => !s.is_empty(),
            ArtValue::Array(arr) => !arr.is_empty(),
            _ => true,
        }
    }

    fn is_equal(&self, a: &ArtValue, b: &ArtValue) -> bool {
        a == b
    }

    fn binary_num_op<F>(&self, left: ArtValue, right: ArtValue, op: F) -> Result<ArtValue>
    where
        F: Fn(f64, f64) -> f64,
    {
        match (left, right) {
            (ArtValue::Int(l), ArtValue::Int(r)) => {
                Ok(ArtValue::Int(op(l as f64, r as f64) as i64))
            }
            (ArtValue::Float(l), ArtValue::Float(r)) => Ok(ArtValue::Float(op(l, r))),
            (ArtValue::Int(l), ArtValue::Float(r)) => Ok(ArtValue::Float(op(l as f64, r))),
            (ArtValue::Float(l), ArtValue::Int(r)) => Ok(ArtValue::Float(op(l, r as f64))),
            _ => {
                // Type mismatch in numeric op
                // Without operator token context here; caller handles some cases explicitly.
                // We fallback to neutral Optional(None).
                // (Future: enrich with span info by passing operator token.)
                Ok(ArtValue::none())
            }
        }
    }

    fn binary_cmp_op<F>(&self, left: ArtValue, right: ArtValue, op: F) -> Result<ArtValue>
    where
        F: Fn(f64, f64) -> bool,
    {
        match (left, right) {
            (ArtValue::Int(l), ArtValue::Int(r)) => Ok(ArtValue::Bool(op(l as f64, r as f64))),
            (ArtValue::Float(l), ArtValue::Float(r)) => Ok(ArtValue::Bool(op(l, r))),
            (ArtValue::Int(l), ArtValue::Float(r)) => Ok(ArtValue::Bool(op(l as f64, r))),
            (ArtValue::Float(l), ArtValue::Int(r)) => Ok(ArtValue::Bool(op(l, r as f64))),
            _ => Ok(ArtValue::none()),
        }
    }
}


impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

// (Removed unused infer_type helper; now handled in dedicated type_infer module)
