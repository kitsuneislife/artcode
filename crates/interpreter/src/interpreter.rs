use crate::type_registry::TypeRegistry;
use crate::values::{Result, RuntimeError};
use core::Token;
use core::ast::{ArtValue, Expr, Function, MatchPattern, ObjHandle, Program, Stmt};
use core::environment::Environment;
use diagnostics::{Diagnostic, DiagnosticKind, Span};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::io::Write;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::sync::Arc;

use std::collections::BTreeMap;

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

    fn drop_scope_heap_objects(&mut self, env: &Rc<RefCell<Environment>>) {
        let handles = env.borrow().strong_handles.clone();
        for h in handles {
            self.dec_object_strong_recursive(h.0);
        }
    }

    fn dec_value_if_heap(&mut self, v: &ArtValue) {
        if let ArtValue::HeapComposite(h) = v {
            self.dec_object_strong_recursive(h.0);
        }
    }

    #[inline]
    fn inc_children_strong(&mut self, v: &ArtValue) {
        match v {
            ArtValue::Array(a) => {
                for child in a {
                    if let ArtValue::HeapComposite(h) = child
                        && let Some(_c) = self.heap_objects.get(&h.0)
                    {
                        self.inc_heap_strong(h.0);
                    }
                }
            }
            ArtValue::StructInstance { fields, .. } => {
                for child in fields.values() {
                    if let ArtValue::HeapComposite(h) = child
                        && let Some(_c) = self.heap_objects.get(&h.0)
                    {
                        self.inc_heap_strong(h.0);
                    }
                }
            }
            ArtValue::EnumInstance { values, .. } => {
                for child in values {
                    if let ArtValue::HeapComposite(h) = child
                        && let Some(_c) = self.heap_objects.get(&h.0)
                    {
                        self.inc_heap_strong(h.0);
                    }
                }
            }
            _ => {}
        }
    }

    /// Extrai os filhos de um valor que contenham referências para objetos Heap e adiciona à fila de Drop.
    #[inline]
    fn enqueue_children_strong(&self, v: &ArtValue, queue: &mut Vec<u64>) {
        match v {
            ArtValue::Array(a) => {
                for child in a {
                    if let ArtValue::HeapComposite(h) = child {
                        queue.push(h.0);
                    }
                }
            }
            ArtValue::StructInstance { fields, .. } => {
                for child in fields.values() {
                    if let ArtValue::HeapComposite(h) = child {
                        queue.push(h.0);
                    }
                }
            }
            ArtValue::EnumInstance { values, .. } => {
                for child in values {
                    if let ArtValue::HeapComposite(h) = child {
                        queue.push(h.0);
                    }
                }
            }
            _ => {}
        }
    }

    fn dec_object_strong_recursive(&mut self, start_id: u64) {
        let mut work_queue: Vec<u64> = vec![start_id];
        let mut visited: std::collections::HashSet<u64> = std::collections::HashSet::new();
        visited.insert(start_id);

        while let Some(id) = work_queue.pop() {
            let mut snapshot_to_enqueue: Option<ArtValue> = None;
            let mut finalizer_opt = None;
            let mut skip_finalizer_due_to_kind = false;

            if let Some(obj) = self.heap_objects.get_mut(&id) {
                if obj.strong > 0 {
                    if crate::heap_utils::dec_strong_obj(obj) {
                        self.strong_decrements += 1;
                    }
                }

                let should_recurse = !obj.alive; // caiu a zero agora
                if should_recurse {
                    self.objects_finalized += 1;
                    if let Some(aid) = obj.arena_id {
                        *self.objects_finalized_per_arena.entry(aid).or_insert(0) += 1;
                    }

                    snapshot_to_enqueue = Some(obj.value.clone());

                    skip_finalizer_due_to_kind = match obj.kind {
                        Some(crate::heap::HeapKind::Atomic)
                        | Some(crate::heap::HeapKind::Mutex) => true,
                        _ => false,
                    };
                }
            } // fecha if let Some(obj) = heap_objects.get_mut(&id)

            if snapshot_to_enqueue.is_some() {
                finalizer_opt = self.finalizers.remove(&id);
            }

            if let Some(snapshot) = snapshot_to_enqueue {
                // Extraímos nós filhos de objetos complexos e rastreamos para não repetir
                let mut local_queue = Vec::new();
                self.enqueue_children_strong(&snapshot, &mut local_queue);
                for child_id in local_queue {
                    if visited.insert(child_id) {
                        work_queue.push(child_id);
                    }
                }

                // Invalidate weak/unowned wrappers that reference this object: mark as dangling
                let mut to_mark_dead: Vec<u64> = Vec::new();
                for (other_id, other_obj) in self.heap_objects.iter_mut() {
                    match &mut other_obj.value {
                        ArtValue::WeakRef(h) => {
                            if h.0 == id {
                                self.weak_dangling += 1;
                                to_mark_dead.push(*other_id);
                            }
                        }
                        ArtValue::UnownedRef(h) => {
                            if h.0 == id {
                                self.unowned_dangling += 1;
                                to_mark_dead.push(*other_id);
                            }
                        }
                        _ => {}
                    }
                }

                if let Some(func) = finalizer_opt {
                    if skip_finalizer_due_to_kind {
                        if self.invariant_checks {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Finalizer skipped for special heap-backed object (Atomic/Mutex)"
                                    .to_string(),
                                Span::new(0, 0, 0, 0),
                            ));
                        }
                    } else {
                        // chamar sem argumentos
                        // Executar função finalizer no ambiente global raiz para permitir expor flags globais
                        let previous_env = self.environment.clone();
                        // Sobe cadeia até raiz
                        let mut root = previous_env.clone();
                        loop {
                            let parent_opt = root.borrow().enclosing.clone();
                            if let Some(p) = parent_opt {
                                root = p
                            } else {
                                break;
                            }
                        }
                        // Criar um frame filho da raiz para evitar poluição direta caso finalizer crie variáveis temporárias
                        self.environment =
                            Rc::new(RefCell::new(Environment::new(Some(root.clone()), 1, None)));
                        // Executar corpo inline se for bloco para evitar criação de escopo interno que perderia variáveis
                        let body_stmt = Rc::as_ref(&func.body).clone();
                        if let Stmt::Block { statements } = body_stmt.clone() {
                            for s in statements {
                                let _ = self.execute(s);
                            }
                        } else {
                            let _ = self.execute(body_stmt);
                        }
                        // Merge simples: mover variáveis definidas neste frame para raiz
                        let local_vals: Vec<(String, ArtValue)> = self
                            .environment
                            .borrow()
                            .values
                            .iter()
                            .map(|(k, v)| ((*k).to_string(), v.clone()))
                            .collect();
                        // Transferir handles fortes deste frame para o root para preservar referências
                        let local_handles = self.environment.borrow().strong_handles.clone();
                        let promoted = local_handles.len();
                        if promoted > 0 {
                            self.finalizer_promotions += promoted;
                            if let Some(aid) = self.current_finalizing_arena {
                                *self.finalizer_promotions_per_arena.entry(aid).or_insert(0) +=
                                    promoted;
                            }
                        }
                        for h in local_handles.iter() {
                            root.borrow_mut().strong_handles.push(*h);
                        }
                        // Mover valores para o root (mantendo mesma identidade)
                        for (k, v) in local_vals {
                            root.borrow_mut()
                                .values
                                .insert(Box::leak(k.into_boxed_str()), v);
                        }
                        // Limpar handles do frame antes de dropar o escopo para evitar double-decrement
                        self.environment.borrow_mut().strong_handles.clear();
                        // Drop any remaining handles/objects in the finalizer frame
                        let finalizer_env = self.environment.clone();
                        self.drop_scope_heap_objects(&finalizer_env);
                        self.environment = previous_env;
                        // Se verificação de invariantes ativada, rodar here para capturar regressões cedo
                        if self.invariant_checks && !self.debug_check_invariants() {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Invariant check failed after finalizer promotion".to_string(),
                                Span::new(0, 0, 0, 0),
                            ));
                        }
                    }
                }
            } // fecha if let Some(snapshot)
        } // while let drop_item = work_queue.pop()

        // Segunda fase (após desempilhar completamente a work queue e rodar destruidores):
        // Agora verificamos e removemos a própria raiz se aplicável (evita dangling handles globais)
        let can_remove_root = if let Some(obj2) = self.heap_objects.get(&start_id) {
            !obj2.alive && obj2.weak == 0
        } else {
            false
        };

        if can_remove_root {
            // verificar se algum objeto vivo referencia este id
            fn referenced_in(value: &ArtValue, target: u64) -> bool {
                match value {
                    ArtValue::HeapComposite(h) => h.0 == target,
                    ArtValue::Array(a) => a.iter().any(|e| referenced_in(e, target)),
                    ArtValue::StructInstance { fields, .. } => {
                        fields.values().any(|e| referenced_in(e, target))
                    }
                    ArtValue::EnumInstance { values, .. } => {
                        values.iter().any(|e| referenced_in(e, target))
                    }
                    _ => false,
                }
            }
            let mut referenced = false;
            for (_other_id, other_obj) in self.heap_objects.iter() {
                if other_obj.alive && referenced_in(&other_obj.value, start_id) {
                    referenced = true;
                    break;
                }
            }
            if !referenced {
                if let Some(obj_to_die) = self.heap_objects.get_mut(&start_id) {
                    obj_to_die.value = ArtValue::none();
                }
                self.heap_objects.remove(&start_id);
            }
        }
    }

    /// Debug/testing: registra valor e retorna id (não otimizado; sem coleta real ainda)
    pub fn debug_heap_register(&mut self, v: ArtValue) -> u64 {
        self.heap_register(v)
    }
    /// Debug/testing: remove id simulando queda de último strong ref
    pub fn debug_heap_remove(&mut self, id: u64) {
        self.dec_heap_strong(id);
    }
    pub fn debug_heap_upgrade_weak(&self, id: u64) -> Option<ArtValue> {
        self.heap_upgrade_weak(id)
    }
    pub fn debug_heap_get_unowned(&self, id: u64) -> Option<ArtValue> {
        if self.is_object_alive(id) {
            self.heap_get_unowned(id)
        } else {
            None
        }
    }

    /// Central helper to increment weak counter on a heap object if present.
    /// Keeping this small wrapper makes it easier to audit all weak operations
    /// in one place when adapting the internal Arc semantics.
    pub fn inc_heap_weak(&mut self, id: u64) {
        use crate::heap_utils::inc_weak_obj;
        if let Some(obj) = self.heap_objects.get_mut(&id) {
            inc_weak_obj(obj);
        }
    }

    /// Central helper to decrement weak counter on a heap object if present.
    pub fn dec_heap_weak(&mut self, id: u64) {
        use crate::heap_utils::dec_weak_obj;
        if let Some(obj) = self.heap_objects.get_mut(&id) {
            if dec_weak_obj(obj) {
                // metric kept at interpreter level if callers want to track
            }
        }
    }
    /// Central helper to increment strong counter on a heap object and update metrics.
    pub fn inc_heap_strong(&mut self, id: u64) {
        use crate::heap_utils::inc_strong_obj;
        if let Some(obj) = self.heap_objects.get_mut(&id) {
            inc_strong_obj(obj);
            self.strong_increments += 1;
        }
    }

    /// Central helper to decrement strong counter on a heap object and update metrics.
    /// This is a low-level helper; high-level finalization logic remains in
    /// `dec_object_strong_recursive` which handles finalizers and sweeping.
    pub fn dec_heap_strong(&mut self, id: u64) {
        use crate::heap_utils::dec_strong_obj;
        if let Some(obj) = self.heap_objects.get_mut(&id) {
            if dec_strong_obj(obj) {
                self.strong_decrements += 1;
            }
        }
    }

    /// Inner helper that performs the decrement on an existing mutable reference
    /// to a `HeapObject`. This avoids performing multiple `get_mut` borrows when
    /// the caller already holds a mutable reference (used by finalizer flow).
    // NOTE: the previous implementation used a helper method that took
    // `&mut self` plus `&mut HeapObject`. That caused borrow-checker
    // conflicts when callers already held a mutable borrow into
    // `self.heap_objects` and then attempted to call another `&mut self`
    // method. To avoid E0499 we inline the decrement where the mutable
    // borrow is available and update metrics directly.

    /// Force the strong counter to 1 by mutating state in a single, auditable helper.
    /// This mirrors previous behavior where some paths set strong=1 to ensure a
    /// subsequent dec drops the object; centralizing makes it easier to find
    /// all write-sites to strong when adapting Arc semantics.
    fn force_heap_strong_to_one(&mut self, id: u64) {
        use crate::heap_utils::force_strong_to_one_obj;
        if let Some(obj) = self.heap_objects.get_mut(&id) {
            force_strong_to_one_obj(obj);
        }
    }
    pub fn debug_heap_dec_strong(&mut self, id: u64) {
        self.dec_heap_strong(id);
    }
    pub fn debug_heap_inc_weak(&mut self, id: u64) {
        self.inc_heap_weak(id);
    }

    /// Test helper: decrementa contador weak (para simulação em testes)
    pub fn debug_heap_dec_weak(&mut self, id: u64) {
        self.dec_heap_weak(id);
    }

    /// Test helper: coleta e remove do heap todos objetos finalizados (!alive) que
    /// não possuem weak refs (weak == 0). Útil em testes para simular uma varredura
    /// de limpeza global ou após chamadas de finalizadores.
    pub fn debug_sweep_dead(&mut self) {
        let dead_ids: Vec<u64> = self
            .heap_objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.alive && obj.weak == 0 {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        // Helper to check whether a live object references target id
        fn referenced_in(value: &ArtValue, target: u64) -> bool {
            match value {
                ArtValue::HeapComposite(h) => h.0 == target,
                ArtValue::Array(a) => a.iter().any(|e| referenced_in(e, target)),
                ArtValue::StructInstance { fields, .. } => {
                    fields.values().any(|e| referenced_in(e, target))
                }
                ArtValue::EnumInstance { values, .. } => {
                    values.iter().any(|e| referenced_in(e, target))
                }
                _ => false,
            }
        }
        for id in dead_ids {
            let mut referenced = false;
            for (_other_id, other_obj) in self.heap_objects.iter() {
                if other_obj.alive && referenced_in(&other_obj.value, id) {
                    referenced = true;
                    break;
                }
            }
            if !referenced {
                // Break deep reference cycles manually to avoid recursive implicit Drop()
                // stack overflow in deeply nested data structures at Arena GC.
                if let Some(obj_to_die) = self.heap_objects.get_mut(&id) {
                    obj_to_die.value = ArtValue::none();
                }
                self.heap_objects.remove(&id);
            }
        }
    }

    /// Test helper: forçar execução do fluxo de finalização para um id específico.
    /// Isto chama o decremento recursivo e em seguida faz sweep de mortos.
    pub fn debug_run_finalizer(&mut self, id: u64) {
        // Restore original behavior: force a decrement/sweep for the helper
        self.dec_object_strong_recursive(id);
        self.debug_sweep_dead();
    }

    /// Test helper: registra valor na arena especificada e retorna id
    pub fn debug_heap_register_in_arena(&mut self, v: ArtValue, arena_id: u32) -> u64 {
        self.heap_register_in_arena(v, arena_id)
    }

    /// Test helper: finaliza explicitamente uma arena (invoca finalize_arena)
    pub fn debug_finalize_arena(&mut self, arena_id: u32) {
        self.finalize_arena(arena_id)
    }

    /// Test helper: verifica se um id ainda existe no heap
    pub fn debug_heap_contains(&self, id: u64) -> bool {
        self.heap_objects.contains_key(&id)
    }

    /// Test helper: return the HeapKind for an object id if set.
    pub fn debug_heap_kind(&self, id: u64) -> Option<crate::heap::HeapKind> {
        self.heap_objects.get(&id).and_then(|o| o.kind.clone())
    }

    /// Habilitar checagem de invariantes em pontos críticos (útil para testes)
    pub fn enable_invariant_checks(&mut self, enable: bool) {
        self.invariant_checks = enable;
    }

    /// Getter para a métrica protótipo finalizer_promotions (útil para asserts em testes/CI)
    pub fn get_finalizer_promotions(&self) -> usize {
        self.finalizer_promotions
    }

    /// Verificação básica de invariantes do heap. Retorna true se OK.
    pub fn debug_check_invariants(&self) -> bool {
        for (_id, obj) in self.heap_objects.iter() {
            if obj.strong == 0 && obj.alive {
                return false;
            }
            // weak/strong são unsigned; garantir que não são absurdamente altas
            if obj.weak > 1_000_000 || obj.strong > 1_000_000 {
                return false;
            }
            // handles referenciem objetos existentes quando array/struct contêm HeapComposite
            fn scan(
                v: &ArtValue,
                heap: &std::collections::HashMap<u64, crate::heap::HeapObject>,
            ) -> bool {
                match v {
                    ArtValue::HeapComposite(h) => heap.contains_key(&h.0),
                    ArtValue::Array(a) => a.iter().all(|e| scan(e, heap)),
                    ArtValue::StructInstance { fields, .. } => {
                        fields.values().all(|e| scan(e, heap))
                    }
                    ArtValue::EnumInstance { values, .. } => values.iter().all(|e| scan(e, heap)),
                    _ => true,
                }
            }
            if !scan(&obj.value, &self.heap_objects) {
                return false;
            }
        }
        true
    }

    /// Debug helper: return textual descriptions of invariant violations (empty if none)
    pub fn debug_invariant_violations(&self) -> Vec<String> {
        let mut msgs = Vec::new();
        for (id, obj) in self.heap_objects.iter() {
            if obj.strong == 0 && obj.alive {
                msgs.push(format!("object {} is alive but has strong==0", id));
            }
            if obj.weak > 1_000_000 || obj.strong > 1_000_000 {
                msgs.push(format!(
                    "object {} has absurd refcounts strong={} weak={}",
                    id, obj.strong, obj.weak
                ));
            }
            // scan children for dangling handles
            fn scan(
                v: &ArtValue,
                heap: &std::collections::HashMap<u64, crate::heap::HeapObject>,
                msgs: &mut Vec<String>,
                parent: u64,
            ) {
                match v {
                    ArtValue::HeapComposite(h) => {
                        if !heap.contains_key(&h.0) {
                            msgs.push(format!(
                                "parent {} references missing child {}",
                                parent, h.0
                            ));
                        }
                    }
                    ArtValue::Array(a) => {
                        for e in a {
                            scan(e, heap, msgs, parent);
                        }
                    }
                    ArtValue::StructInstance { fields, .. } => {
                        for val in fields.values() {
                            scan(val, heap, msgs, parent);
                        }
                    }
                    ArtValue::EnumInstance { values, .. } => {
                        for val in values {
                            scan(val, heap, msgs, parent);
                        }
                    }
                    _ => {}
                }
            }
            scan(&obj.value, &self.heap_objects, &mut msgs, *id);
        }
        msgs
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

    fn bind_value_to_pattern(
        &mut self,
        pattern: &core::ast::MatchPattern,
        value: ArtValue,
    ) -> Result<()> {
        match pattern {
            core::ast::MatchPattern::Variable(name) => {
                // Runtime check: evitar que valores alocados em arena escapem para fora do bloco performant.
                if let ArtValue::HeapComposite(h) = &value
                    && let Some(obj) = self.heap_objects.get(&h.0)
                    && let Some(aid) = obj.arena_id
                    && Some(aid) != self.current_arena
                {
                    let msg = format!(
                        "Attempt to bind arena object (arena={}) into scope outside of that arena (current_arena={:?}) for variable '{}'.",
                        aid, self.current_arena, name.lexeme
                    );
                    debug_assert!(!msg.is_empty(), "{}", msg);
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        msg,
                        Span::new(name.start, name.end, name.line, name.col),
                    ));
                }

                // Captura possível valor antigo sem manter borrow mutável durante decremento
                let old_opt = {
                    self.environment
                        .borrow()
                        .values
                        .get(name.lexeme.as_str())
                        .cloned()
                };
                if let Some(old) = &old_opt {
                    self.dec_value_if_heap(old);
                }
                let mut env = self.environment.borrow_mut();
                env.define(&name.lexeme, value);
                Ok(())
            }
            core::ast::MatchPattern::Tuple(patterns) => {
                if let ArtValue::Tuple(values) = value {
                    if patterns.len() != values.len() {
                        return Err(RuntimeError::TypeError(format!(
                            "Tuple pattern length {} does not match tuple value length {}",
                            patterns.len(),
                            values.len()
                        )));
                    }
                    for (p, v) in patterns.iter().zip(values.into_iter()) {
                        self.bind_value_to_pattern(p, v)?;
                    }
                    Ok(())
                } else {
                    return Err(RuntimeError::TypeError(format!(
                        "Cannot destructure non-tuple value '{:?}' with tuple pattern",
                        value
                    )));
                }
            }
            _ => {
                // Ignore other patterns for `let` declarations for now (or throw error if unsupported)
                Ok(())
            }
        }
    }

    fn debug_prompt(&mut self, stmt: &core::ast::Stmt) -> Result<bool> {
        use std::io::{self, Write};
        loop {
            println!("\n[Tick {}] {:?}", self.executed_statements, stmt);
            print!("(art-debug) > ");
            let _ = io::stdout().flush();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                break;
            }
            let input = input.trim();

            match input {
                "" | "step" | "s" => return Ok(false),
                "back" | "b" => return Ok(true),
                "env" => {
                    println!("Environment bindings:");
                    for (k, _) in self.environment.borrow().values.iter() {
                        println!(" - {}", k);
                    }
                }
                "help" => {
                    println!("Commands:");
                    println!("  step (s)      - Avança 1 statement (Default)");
                    println!(
                        "  back (b)      - Volta 1 statement no tempo via snapshotting rápido"
                    );
                    println!(
                        "  inspect <var> - Avalia nome da variável no contexto local ou global"
                    );
                    println!("  env           - Lista escopo");
                    println!("  help          - Mostra essa ajuda");
                }
                other if other.starts_with("inspect ") => {
                    let var = &other[8..];
                    if let Some(val) = self.get_global(var) {
                        println!("{} = {:?}", var, val);
                    } else {
                        println!("Variable '{}' not found.", var);
                    }
                }
                _ => println!("Unknown command. Type 'help'."),
            }
        }
        Ok(false)
    }

    fn execute(&mut self, stmt: Stmt) -> Result<()> {
        if self.debug_mode || self.fast_forward_until.is_some() {
            let should_prompt = match self.fast_forward_until {
                Some(target) => self.executed_statements >= target,
                None => self.debug_mode,
            };
            if should_prompt {
                self.fast_forward_until = None;
                self.debug_mode = true;
                if self.debug_prompt(&stmt)? {
                    return Err(crate::values::RuntimeError::DebugStepBack);
                }
            }
        }
        self.executed_statements += 1;
        let result = match stmt {
            Stmt::Expression(expr) => {
                let val = self.evaluate(expr)?;
                self.last_value = Some(val.clone());
                Ok(())
            }
            Stmt::Let {
                pattern,
                ty: _,
                initializer,
            } => {
                let value = self.evaluate(initializer)?;

                // CRITICAL: If the actor was parked during evaluation (e.g. by actor_receive),
                // do NOT bind the pattern yet. The statement will be retried when unparked.
                // Reinsertion is handled by the round-robin scheduler (run_actors_round_robin).
                if self
                    .executing_actor
                    .as_ref()
                    .map(|a| a.parked)
                    .unwrap_or(false)
                {
                    return Ok(());
                }

                // Runtime check: evitar que valores alocados em arena escapem para fora do bloco performant.
                if let ArtValue::HeapComposite(h) = &value
                    && let Some(obj) = self.heap_objects.get(&h.0)
                    && let Some(aid) = obj.arena_id
                    && (self.in_performant_block || Some(aid) != self.current_arena)
                {
                    let msg = format!(
                        "Attempt to bind arena object (arena={}) into scope outside of that arena (current_arena={:?}) for variable '{}'.",
                        aid,
                        self.current_arena,
                        match &pattern {
                            core::ast::MatchPattern::Variable(name) => name.lexeme.clone(),
                            _ => "<pattern>".to_string(),
                        }
                    );
                    debug_assert!(!msg.is_empty(), "{}", msg);
                    let fn_name = if let core::ast::MatchPattern::Variable(name) = &pattern {
                        name
                    } else {
                        &core::Token::dummy("_")
                    };
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        msg,
                        Span::new(fn_name.start, fn_name.end, fn_name.line, fn_name.col),
                    ));
                }

                // Promoção se necessário
                let aid = self.environment.borrow().associated_arena;
                let mut promoted_value = value;
                self.promote_if_escaping(aid, &mut promoted_value);

                self.bind_value_to_pattern(&pattern, promoted_value)?;
                Ok(())
            }
            Stmt::Block { statements } => {
                self.execute_block(statements, Some(self.environment.clone()))
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition_value = self.evaluate(condition)?;
                if self.is_truthy(&condition_value) {
                    self.execute(*then_branch)
                } else if let Some(else_stmt) = else_branch {
                    self.execute(*else_stmt)
                } else {
                    Ok(())
                }
            }
            Stmt::IfLet {
                pattern,
                value,
                then_branch,
                else_branch,
            } => {
                let eval_value = self.evaluate(value)?;
                if let Some(bindings) = self.pattern_matches(&pattern, &eval_value) {
                    let (parent_depth, parent_arena) = {
                        let b = self.environment.borrow();
                        (b.depth, b.associated_arena)
                    };
                    let mut new_env = Environment::new(
                        Some(self.environment.clone()),
                        parent_depth + 1,
                        parent_arena,
                    );
                    for (k, mut v) in bindings {
                        let target_aid = new_env.associated_arena;
                        self.promote_if_escaping(target_aid, &mut v);
                        new_env.define(&k, v);
                    }
                    let previous = self.environment.clone();
                    self.environment = Rc::new(RefCell::new(new_env));
                    let res = self.execute(*then_branch);
                    self.environment = previous;
                    res
                } else if let Some(else_stmt) = else_branch {
                    self.execute(*else_stmt)
                } else {
                    Ok(())
                }
            }
            Stmt::StructDecl { name, fields } => {
                self.type_registry.register_struct(name, fields);
                Ok(())
            }
            Stmt::EnumDecl { name, variants } => {
                self.type_registry.register_enum(name, variants);
                Ok(())
            }
            Stmt::Match { expr, cases } => {
                let match_value = self.evaluate(expr)?;
                for (pattern, guard, stmt) in cases {
                    if let Some(bindings) = self.pattern_matches(&pattern, &match_value) {
                        let (p_depth, p_arena) = {
                            let b = self.environment.borrow();
                            (b.depth, b.associated_arena)
                        };
                        // Avaliar guard (se existir) em ambiente com bindings temporário
                        if let Some(gexpr) = guard {
                            let previous_env = self.environment.clone();
                            let temp_env = Rc::new(RefCell::new(Environment::new(
                                Some(previous_env.clone()),
                                p_depth + 1,
                                p_arena,
                            )));
                            self.environment = temp_env.clone();
                            for (name, value) in bindings.iter() {
                                self.environment.borrow_mut().define(name, value.clone());
                            }
                            let guard_passed = self
                                .evaluate(gexpr)
                                .map(|v| self.is_truthy(&v))
                                .unwrap_or(false);
                            // Garantir que handles fortes do ambiente temporário do guard sejam decrementados
                            self.drop_scope_heap_objects(&temp_env);
                            self.environment = previous_env;
                            if !guard_passed {
                                continue;
                            }
                        }
                        let (p_depth, p_arena) = {
                            let b = self.environment.borrow();
                            (b.depth, b.associated_arena)
                        };
                        let new_env_struct =
                            Environment::new(Some(self.environment.clone()), p_depth + 1, p_arena);
                        let new_env = Rc::new(RefCell::new(new_env_struct));
                        let previous = self.environment.clone();
                        self.environment = new_env.clone();
                        for (name, mut value) in bindings {
                            let target_aid = new_env.borrow().associated_arena;
                            self.promote_if_escaping(target_aid, &mut value);
                            new_env.borrow_mut().define(&name, value);
                        }
                        // Executar o corpo e garantir que mesmo em erro o escopo temporário seja limpo
                        let result = self.execute(stmt);
                        // Drop handles do env de bindings antes de restaurar
                        self.drop_scope_heap_objects(&new_env);
                        self.environment = previous;
                        return result;
                    }
                }

                // Se chegou aqui, nenhum pattern casou (Non-exhaustive pattern match no Runtime)
                self.diagnostics.push(Diagnostic::new(
                    DiagnosticKind::Runtime,
                    format!(
                        "Non-exhaustive match: no pattern matched the value '{:?}'",
                        match_value
                    ),
                    Span::new(0, 0, 0, 0), // Idealmente teríamos o span do Stmt::Match
                ));
                Ok(())
            }
            Stmt::TryCatch {
                try_branch,
                catch_name,
                catch_branch,
            } => match self.execute(*try_branch) {
                Ok(()) => Ok(()),
                Err(RuntimeError::Return(v)) => Err(RuntimeError::Return(v)),
                Err(RuntimeError::DebugStepBack) => Err(RuntimeError::DebugStepBack),
                Err(RuntimeError::TypeError(msg)) => {
                    let previous_env = self.environment.clone();
                    let (p_depth, p_arena) = {
                        let b = previous_env.borrow();
                        (b.depth, b.associated_arena)
                    };
                    let catch_env = Rc::new(RefCell::new(Environment::new(
                        Some(previous_env.clone()),
                        p_depth + 1,
                        p_arena,
                    )));
                    self.environment = catch_env.clone();

                    self.environment
                        .borrow_mut()
                        .define(&catch_name.lexeme, ArtValue::String(Arc::from(msg)));

                    let result = self.execute(*catch_branch);

                    self.drop_scope_heap_objects(&catch_env);
                    self.environment = previous_env;

                    result
                }
            },
            Stmt::Function {
                name,
                type_params,
                params,
                return_type: _,
                body,
                method_owner,
                is_async: _,
            } => {
                let fn_rc = Rc::new(Function {
                    name: Some(name.lexeme.clone()),
                    type_params: type_params.clone(),
                    params: params.clone(),
                    body: body.clone(),
                    closure: Rc::downgrade(&self.environment),
                    retained_env: None,
                });
                if let Some(owner) = method_owner {
                    if let Some(sdef) = self.type_registry.structs.get_mut(&owner) {
                        sdef.methods.insert(name.lexeme.clone(), (*fn_rc).clone());
                    } else if let Some(edef) = self.type_registry.enums.get_mut(&owner) {
                        edef.methods.insert(name.lexeme.clone(), (*fn_rc).clone());
                    } else {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("Unknown type '{}' for method.", owner),
                            Span::new(name.start, name.end, name.line, name.col),
                        ));
                    }
                } else {
                    let old_opt = {
                        self.environment
                            .borrow()
                            .values
                            .get(name.lexeme.as_str())
                            .cloned()
                    };
                    if let Some(old) = &old_opt {
                        self.dec_value_if_heap(old);
                    }
                    let mut env = self.environment.borrow_mut();
                    env.define(&name.lexeme, ArtValue::Function(fn_rc.clone()));
                }
                Ok(())
            }
            Stmt::ImplBlock { methods, .. } => {
                for method in methods {
                    self.execute(method)?;
                }
                Ok(())
            }
            Stmt::Return { value } => {
                let mut return_value = match value {
                    Some(expr) => self.evaluate(expr)?,
                    None => ArtValue::none(),
                };
                // Runtime check: impedir retorno de objetos de arena para fora do bloco performant
                if let ArtValue::HeapComposite(h) = &return_value
                    && let Some(obj) = self.heap_objects.get(&h.0)
                    && let Some(aid) = obj.arena_id
                    && (self.in_performant_block || Some(aid) != self.current_arena)
                {
                    let msg = format!(
                        "Attempt to return arena object (arena={}) outside of its arena (current_arena={:?}).",
                        aid, self.current_arena
                    );
                    debug_assert!(!msg.is_empty(), "{}", msg);
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        msg,
                        Span::new(0, 0, 0, 0),
                    ));
                }
                Err(RuntimeError::Return(return_value))
            }
            Stmt::Performant { statements } => {
                // Criar arena ID e frame léxico
                let aid = self.next_arena_id;
                self.next_arena_id += 1;
                let prev_arena = self.current_arena;
                self.current_arena = Some(aid);
                let prev_performant = self.in_performant_block;
                self.in_performant_block = true;

                let previous = self.environment.clone();
                let (p_depth, _) = {
                    let b = previous.borrow();
                    (b.depth, b.associated_arena)
                };

                self.environment = Rc::new(RefCell::new(Environment::new(
                    Some(previous.clone()),
                    p_depth + 1,
                    Some(aid),
                )));
                let scope_env = self.environment.clone();

                // Executar statements
                for s in statements {
                    if let Err(e) = self.execute(s) {
                        self.drop_scope_heap_objects(&scope_env);
                        self.finalize_arena(aid);
                        self.current_arena = prev_arena;
                        self.in_performant_block = prev_performant;
                        self.environment = previous;
                        return Err(e);
                    }
                }

                // Cleanup
                self.drop_scope_heap_objects(&scope_env);
                self.finalize_arena(aid);
                self.current_arena = prev_arena;
                self.in_performant_block = prev_performant;
                self.environment = previous;
                Ok(())
            }
            Stmt::Import { path: _ } => {
                // Import is a compile-time / resolver concern; runtime no-op for now.
                Ok(())
            }
            Stmt::ShellCommand { program, args } => {
                if !self.ensure_pure_allowed("shell") {
                    let blocked = Self::shell_result_err(
                        "Operation 'shell' is not allowed in --pure mode".to_string(),
                    );
                    self.publish_shell_result(blocked);
                    return Ok(());
                }

                match self.run_shell_stages(&program, &args) {
                    Ok(output) => {
                        if !output.stdout.is_empty() {
                            print!("{}", String::from_utf8_lossy(&output.stdout));
                        }
                        if !output.stderr.is_empty() {
                            eprint!("{}", String::from_utf8_lossy(&output.stderr));
                        }
                        let result = if output.status.success() {
                            Self::shell_result_ok(
                                String::from_utf8_lossy(&output.stdout).to_string(),
                            )
                        } else if output.stderr.is_empty() {
                            Self::shell_result_err(format!(
                                "Shell command '{}' exited with status {:?}",
                                program,
                                output.status.code()
                            ))
                        } else {
                            Self::shell_result_err(
                                String::from_utf8_lossy(&output.stderr).to_string(),
                            )
                        };
                        self.publish_shell_result(result);
                        Ok(())
                    }
                    Err(e) => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            e.clone(),
                            Span::new(0, 0, 0, 0),
                        ));
                        self.publish_shell_result(Self::shell_result_err(e));
                        Ok(())
                    }
                }
            }
            Stmt::While { condition, body } => {
                loop {
                    let cond_val = self.evaluate(condition.clone())?;
                    if !self.is_truthy(&cond_val) {
                        break;
                    }
                    let _aid = self.push_implicit_arena();
                    let res = self.execute(*body.clone());
                    self.pop_implicit_arena();
                    if let Err(e) = res {
                        return Err(e);
                    }
                }
                Ok(())
            }
            Stmt::For {
                element,
                iterator,
                body,
            } => {
                let iter_val = self.evaluate(iterator)?;

                // Support: arrays, stream pipelines, or iterator protocols (callable returning Option).
                // This allows generators to be implemented as closures returning Option.None.
                enum IterSource {
                    Array(Vec<ArtValue>),
                    Iterator,
                }

                let iter_source = match iter_val.clone() {
                    ArtValue::Array(arr) => IterSource::Array(arr),
                    ArtValue::StructInstance {
                        ref struct_name, ..
                    } if struct_name == "__Stream" => {
                        match self.decode_stream_value(iter_val.clone()) {
                            Ok((source, ops)) => {
                                IterSource::Array(self.run_stream_pipeline(source, ops)?)
                            }
                            Err(msg) => {
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    msg,
                                    Span::new(
                                        element.start,
                                        element.end,
                                        element.line,
                                        element.col,
                                    ),
                                ));
                                return Ok(());
                            }
                        }
                    }
                    ArtValue::HeapComposite(h) => {
                        match self.heap_objects.get(&h.0).map(|obj| obj.value.clone()) {
                            Some(ArtValue::Array(arr)) => IterSource::Array(arr),
                            Some(
                                ref v @ ArtValue::StructInstance {
                                    ref struct_name, ..
                                },
                            ) if struct_name == "__Stream" => {
                                match self.decode_stream_value(v.clone()) {
                                    Ok((source, ops)) => {
                                        IterSource::Array(self.run_stream_pipeline(source, ops)?)
                                    }
                                    Err(msg) => {
                                        self.diagnostics.push(Diagnostic::new(
                                            DiagnosticKind::Runtime,
                                            msg,
                                            Span::new(
                                                element.start,
                                                element.end,
                                                element.line,
                                                element.col,
                                            ),
                                        ));
                                        return Ok(());
                                    }
                                }
                            }
                            Some(_) => IterSource::Iterator,
                            None => {
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    "Cannot iterate over dangling heap handle.".to_string(),
                                    Span::new(
                                        element.start,
                                        element.end,
                                        element.line,
                                        element.col,
                                    ),
                                ));
                                return Ok(());
                            }
                        }
                    }
                    ArtValue::Function(_) | ArtValue::Builtin(_) => IterSource::Iterator,
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("Cannot iterate over unsupported type: {:?}", iter_val),
                            Span::new(element.start, element.end, element.line, element.col),
                        ));
                        return Ok(());
                    }
                };

                // Helper to call iterator `next()` and return the next value (or None).
                let call_next = |me: &mut Interpreter, iter_val: &ArtValue| -> Result<ArtValue> {
                    match iter_val {
                        ArtValue::Function(func) => {
                            me.call_function(func.clone(), None, Vec::new())
                        }
                        ArtValue::Builtin(b) => me.call_builtin(b.clone(), Vec::new()),
                        ArtValue::StructInstance {
                            struct_name,
                            fields,
                        } => {
                            let token = Token::dummy("next");
                            if let Some(callable) = crate::field_access::struct_field_or_method(
                                struct_name,
                                fields,
                                &token,
                                &me.type_registry,
                            ) {
                                match callable {
                                    ArtValue::Function(func) => {
                                        me.call_function(func, None, Vec::new())
                                    }
                                    ArtValue::Builtin(b) => me.call_builtin(b, Vec::new()),
                                    other => {
                                        me.diagnostics.push(Diagnostic::new(
                                            DiagnosticKind::Runtime,
                                            format!(
                                                "Iterator 'next' must be callable, got: {:?}",
                                                other
                                            ),
                                            Span::new(
                                                element.start,
                                                element.end,
                                                element.line,
                                                element.col,
                                            ),
                                        ));
                                        Ok(ArtValue::none())
                                    }
                                }
                            } else {
                                me.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    "Iterator object does not implement 'next()' method."
                                        .to_string(),
                                    Span::new(
                                        element.start,
                                        element.end,
                                        element.line,
                                        element.col,
                                    ),
                                ));
                                Ok(ArtValue::none())
                            }
                        }
                        ArtValue::HeapComposite(_) => {
                            let resolved = me.resolve_composite(iter_val).clone();
                            if let ArtValue::StructInstance {
                                struct_name,
                                fields,
                            } = resolved
                            {
                                let token = Token::dummy("next");
                                if let Some(callable) = crate::field_access::struct_field_or_method(
                                    &struct_name,
                                    &fields,
                                    &token,
                                    &me.type_registry,
                                ) {
                                    match callable {
                                        ArtValue::Function(func) => {
                                            me.call_function(func, None, Vec::new())
                                        }
                                        ArtValue::Builtin(b) => me.call_builtin(b, Vec::new()),
                                        other => {
                                            me.diagnostics.push(Diagnostic::new(
                                                DiagnosticKind::Runtime,
                                                format!(
                                                    "Iterator 'next' must be callable, got: {:?}",
                                                    other
                                                ),
                                                Span::new(
                                                    element.start,
                                                    element.end,
                                                    element.line,
                                                    element.col,
                                                ),
                                            ));
                                            Ok(ArtValue::none())
                                        }
                                    }
                                } else {
                                    me.diagnostics.push(Diagnostic::new(
                                        DiagnosticKind::Runtime,
                                        "Iterator object does not implement 'next()' method."
                                            .to_string(),
                                        Span::new(
                                            element.start,
                                            element.end,
                                            element.line,
                                            element.col,
                                        ),
                                    ));
                                    Ok(ArtValue::none())
                                }
                            } else {
                                me.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    format!("Cannot iterate over unsupported type: {:?}", iter_val),
                                    Span::new(
                                        element.start,
                                        element.end,
                                        element.line,
                                        element.col,
                                    ),
                                ));
                                Ok(ArtValue::none())
                            }
                        }
                        _ => {
                            me.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                format!("Cannot iterate over unsupported type: {:?}", iter_val),
                                Span::new(element.start, element.end, element.line, element.col),
                            ));
                            Ok(ArtValue::none())
                        }
                    }
                };

                match iter_source {
                    IterSource::Array(array_elements) => {
                        for mut val in array_elements {
                            let previous_env = self.environment.clone();
                            let (p_depth, p_arena) = {
                                let b = previous_env.borrow();
                                (b.depth, b.associated_arena)
                            };
                            let loop_env = Rc::new(RefCell::new(Environment::new(
                                Some(previous_env.clone()),
                                p_depth + 1,
                                p_arena,
                            )));
                            self.environment = loop_env.clone();

                            let target_aid = loop_env.borrow().associated_arena;
                            self.promote_if_escaping(target_aid, &mut val);
                            self.environment.borrow_mut().define(&element.lexeme, val);

                            let result = self.execute(*body.clone());

                            self.drop_scope_heap_objects(&loop_env);
                            self.environment = previous_env;

                            if let Err(e) = result {
                                return Err(e);
                            }
                        }
                        Ok(())
                    }
                    IterSource::Iterator => {
                        let iter_val = iter_val;
                        loop {
                            let next_val = call_next(self, &iter_val)?;
                            let next_val = self.resolve_composite(&next_val).clone();
                            let mut item = match next_val {
                                ArtValue::Optional(boxed) => match *boxed {
                                    Some(v) => v,
                                    None => break,
                                },
                                ArtValue::EnumInstance {
                                    enum_name,
                                    variant,
                                    values,
                                } if enum_name == "Option" => {
                                    if variant == "Some" {
                                        values.into_iter().next().unwrap_or(ArtValue::none())
                                    } else {
                                        break;
                                    }
                                }
                                other => {
                                    self.diagnostics.push(Diagnostic::new(
                                        DiagnosticKind::Runtime,
                                        format!(
                                            "Iterator protocol expected Optional, got: {:?}",
                                            other
                                        ),
                                        Span::new(
                                            element.start,
                                            element.end,
                                            element.line,
                                            element.col,
                                        ),
                                    ));
                                    break;
                                }
                            };

                            let previous_env = self.environment.clone();
                            let (p_depth, _p_arena) = {
                                let b = previous_env.borrow();
                                (b.depth, b.associated_arena)
                            };
                            // Iniciamos uma arena para o corpo do loop
                            let _aid = self.push_implicit_arena();
                            let loop_env = Rc::new(RefCell::new(Environment::new(
                                Some(previous_env.clone()),
                                p_depth + 1,
                                Some(_aid),
                            )));
                            self.environment = loop_env.clone();

                            let target_aid = loop_env.borrow().associated_arena;
                            self.promote_if_escaping(target_aid, &mut item);
                            self.environment.borrow_mut().define(&element.lexeme, item);

                            let result = self.execute(*body.clone());

                            self.drop_scope_heap_objects(&loop_env);
                            self.pop_implicit_arena();
                            self.environment = previous_env;

                            if let Err(e) = result {
                                return Err(e);
                            }
                        }
                        Ok(())
                    }
                }
            }
            Stmt::SpawnActor { body } => {
                let aid = self.next_actor_id;
                self.next_actor_id += 1;
                let actor_env = Rc::new(RefCell::new(Environment::new(
                    Some(self.environment.clone()),
                    0,    // Atores começam novo root de depth
                    None, // e usam ARC global por padrão (Nexus isolation)
                )));
                let actor = ActorState {
                    id: aid,
                    mailbox: Mailbox::new(),
                    body: VecDeque::from(body),
                    env: actor_env,
                    finished: false,
                    parked: false,
                    mailbox_limit: self.actor_mailbox_limit,
                };
                self.actors.insert(aid, actor);
                // Return actor handle as Actor variant (IDs still exposed as Int in tests where needed)
                self.last_value = Some(ArtValue::Actor(aid));
                Ok(())
            }
        };

        if result.is_ok() {
            self.maybe_record_checkpoint();
        }

        result
    }

    fn pattern_matches(
        &mut self,
        pattern: &MatchPattern,
        value: &ArtValue,
    ) -> Option<Vec<(String, ArtValue)>> {
        // Se valor for HeapComposite, desreferencia para o valor real subjacente antes de matching.
        let resolved_owned;
        let value_ref = if let ArtValue::HeapComposite(h) = value {
            if let Some(obj) = self.heap_objects.get(&h.0) {
                resolved_owned = obj.value.clone();
                &resolved_owned
            } else {
                value
            }
        } else {
            value
        };
        match (pattern, value_ref) {
            (MatchPattern::Literal(lit), _) if lit == value => Some(vec![]),
            (MatchPattern::Wildcard, _) => Some(vec![]),
            // Se o binding está dentro de EnumVariant, associe ao valor correto
            (MatchPattern::Binding(name) | MatchPattern::Variable(name), val) => {
                // Se val for EnumInstance com um valor, associe ao primeiro valor
                if let ArtValue::EnumInstance { values, .. } = val {
                    if values.len() == 1 {
                        Some(vec![(name.lexeme.clone(), values[0].clone())])
                    } else {
                        // Se não, associe ao próprio valor
                        Some(vec![(name.lexeme.clone(), val.clone())])
                    }
                } else {
                    Some(vec![(name.lexeme.clone(), val.clone())])
                }
            }
            (
                MatchPattern::EnumVariant {
                    enum_name,
                    variant,
                    params,
                },
                ArtValue::EnumInstance {
                    enum_name: inst_enum_name,
                    variant: v_name,
                    values,
                    ..
                },
            ) if &variant.lexeme == v_name => {
                // Verificar nome do enum se especificado
                if let Some(enum_name_tok) = enum_name
                    && &enum_name_tok.lexeme != inst_enum_name
                {
                    return None;
                }
                match params {
                    Some(param_patterns) => {
                        if param_patterns.len() != values.len() {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                format!(
                                    "Arity mismatch in pattern: expected {} found {}",
                                    values.len(),
                                    param_patterns.len()
                                ),
                                Span::new(variant.start, variant.end, variant.line, variant.col),
                            ));
                            return None;
                        }
                        let mut all_bindings = Vec::new();
                        for (i, p) in param_patterns.iter().enumerate() {
                            if let Some(bindings) = self.pattern_matches(p, &values[i]) {
                                all_bindings.extend(bindings);
                            } else {
                                return None;
                            }
                        }
                        Some(all_bindings)
                    }
                    None => {
                        if values.is_empty() {
                            Some(vec![])
                        } else {
                            None
                        }
                    }
                }
            }
            _ => None,
        }
    }

    fn execute_block(
        &mut self,
        statements: Vec<Stmt>,
        enclosing: Option<Rc<RefCell<Environment>>>,
    ) -> Result<()> {
        let (p_depth, p_arena) = if let Some(ref e) = enclosing {
            let b = e.borrow();
            (b.depth, b.associated_arena)
        } else {
            (0, None)
        };
        let previous = self.environment.clone();
        self.environment = Rc::new(RefCell::new(Environment::new(
            enclosing,
            p_depth + 1,
            p_arena,
        )));
        let scope_env = self.environment.clone();
        for statement in statements {
            if self
                .executing_actor
                .as_ref()
                .map(|a| a.parked)
                .unwrap_or(false)
            {
                break;
            }
            if let Err(e) = self.execute(statement) {
                // If this is a Return carrying values that depend on the current scope,
                // preserve them BEFORE dropping the block environment.
                let transformed = match e {
                    RuntimeError::Return(mut rv) => {
                        // Promoção Adaptativa ao retornar valores
                        let target_aid = previous.borrow().associated_arena;
                        self.promote_if_escaping(target_aid, &mut rv);
                        if let ArtValue::HeapComposite(ref h) = rv {
                            // Keep heap composite alive across scope teardown.
                            self.inc_heap_strong(h.0);
                        }
                        if let ArtValue::Function(ref f) = rv {
                            if f.retained_env.is_none() {
                                // Returned closures can capture this block env.
                                let escaped = Function {
                                    name: f.name.clone(),
                                    type_params: f.type_params.clone(),
                                    params: f.params.clone(),
                                    body: f.body.clone(),
                                    closure: f.closure.clone(),
                                    retained_env: Some(scope_env.clone()),
                                };
                                let mut rv = ArtValue::Function(Rc::new(escaped));
                                let aid = previous.borrow().associated_arena;
                                self.promote_if_escaping(aid, &mut rv);
                                return Err(RuntimeError::Return(rv));
                            }
                        }
                        RuntimeError::Return(rv)
                    }
                    other => other,
                };
                self.drop_scope_heap_objects(&scope_env);
                self.environment = previous;
                return Err(transformed);
            }
        }
        self.drop_scope_heap_objects(&scope_env);
        self.environment = previous;
        Ok(())
    }

    fn evaluate(&mut self, expr: Expr) -> Result<ArtValue> {
        const MAX_EVAL_DEPTH: usize = 128;
        self.eval_depth += 1;
        if self.eval_depth > MAX_EVAL_DEPTH {
            self.eval_depth -= 1;
            self.diagnostics.push(Diagnostic::new(
                DiagnosticKind::Runtime,
                format!("Expression evaluation nesting too deep (limit {MAX_EVAL_DEPTH}). Possible infinite recursion."),
                Span::new(0, 0, 0, 0),
            ));
            return Ok(ArtValue::none());
        }
        let result = self.evaluate_inner(expr);
        self.eval_depth -= 1;
        result
    }

    fn evaluate_inner(&mut self, expr: Expr) -> Result<ArtValue> {
        match expr {
            Expr::InterpolatedString(parts) => {
                use crate::fstring::eval_fstring;
                eval_fstring(parts, |e| self.evaluate(e))
            }
            Expr::Try(inner) => {
                // Com a introdução de weak/unowned, Try original de Result permanece como compat.
                let result_val = self.evaluate(*inner)?;
                match result_val {
                    ArtValue::EnumInstance {
                        enum_name,
                        variant,
                        mut values,
                    } if enum_name == "Result" => {
                        if variant == "Ok" {
                            Ok(values.pop().unwrap_or(ArtValue::none()))
                        } else {
                            Err(RuntimeError::Return(
                                values.pop().unwrap_or(ArtValue::none()),
                            ))
                        }
                    }
                    other => Ok(other),
                }
            }
            Expr::Literal(value) => Ok(value),
            Expr::Grouping { expression } => self.evaluate(*expression),
            Expr::Variable { name } => {
                let name_str = name.lexeme.clone();
                if (name_str == "variant" || name_str == "values")
                    && let Some(ArtValue::EnumInstance {
                        variant, values, ..
                    }) = self.environment.borrow().get("self")
                {
                    if name_str == "variant" {
                        return Ok(ArtValue::String(core::intern_arc(variant.as_str())));
                    } else {
                        return Ok(ArtValue::Array(values.clone()));
                    }
                }
                // Fallback para builtins: checamos ANTES para evitar empréstimos mutáveis desnecessários
                // do ambiente global (evita pânicos de RefCell em atores).
                // Nota: Shadowing ainda funciona se o usuário definir explicitamente a variável,
                // pois o ambiente local será checado em seguida.
                if let Some(builtin) = PRELUDE_VALUES.with(|p| p.get(name_str.as_str()).cloned()) {
                    // Mas espera! Se houver uma variável local com esse nome, ela deve ter precedência.
                    // Fazemos um borrow imutável rápido para verificar.
                    if !self.environment.borrow().has_locally(&name_str) {
                        return Ok(builtin);
                    }
                }

                let val = self.environment.borrow_mut().read_for_eval(&name_str);
                if let Some(v) = val {
                    if let ArtValue::MovedCapability = v {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!(
                                "Capability '{}' was already moved/consumed and cannot be reused",
                                name_str
                            ),
                            Span::new(name.start, name.end, name.line, name.col),
                        ));
                        return Ok(ArtValue::none());
                    }
                    return Ok(v);
                }

                // Não encontrado nem no ambiente léxico nem nos builtins
                let env_borrow = self.environment.borrow();
                let candidates = env_borrow.values.keys().copied();
                let suggestion = if let Some(best) = did_you_mean(&name_str, candidates) {
                    format!(" Did you mean '{}'?", best)
                } else {
                    String::new()
                };

                self.diagnostics.push(Diagnostic::new(
                    DiagnosticKind::Runtime,
                    format!("Undefined variable '{}'.{}", name_str, suggestion),
                    Span::new(name.start, name.end, name.line, name.col),
                ));
                Ok(ArtValue::none())
            }
            Expr::Unary { operator, right } => {
                let right_val = self.evaluate(*right)?;
                match operator.token_type {
                    core::TokenType::Minus => match right_val {
                        ArtValue::Int(n) => Ok(ArtValue::Int(-n)),
                        ArtValue::Float(f) => Ok(ArtValue::Float(-f)),
                        _ => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Type mismatch.".to_string(),
                                Span::new(
                                    operator.start,
                                    operator.end,
                                    operator.line,
                                    operator.col,
                                ),
                            ));
                            Ok(ArtValue::none())
                        }
                    },
                    core::TokenType::Bang => Ok(ArtValue::Bool(!self.is_truthy(&right_val))),
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "Invalid operator.".to_string(),
                            Span::new(operator.start, operator.end, operator.line, operator.col),
                        ));
                        Ok(ArtValue::none())
                    }
                }
            }
            Expr::Logical {
                left,
                operator,
                right,
            } => {
                let left_val = self.evaluate(*left)?;
                if operator.token_type == core::TokenType::Or {
                    if self.is_truthy(&left_val) {
                        return Ok(left_val);
                    }
                } else if !self.is_truthy(&left_val) {
                    return Ok(left_val);
                }
                self.evaluate(*right)
            }
            Expr::Binary {
                left,
                operator,
                right,
            } => {
                let left_val = self.evaluate(*left)?;
                let right_val = self.evaluate(*right)?;
                match operator.token_type {
                    core::TokenType::Plus => match (&left_val, &right_val) {
                        (ArtValue::Int(l), ArtValue::Int(r)) => Ok(ArtValue::Int(l + r)),
                        (ArtValue::Float(l), ArtValue::Float(r)) => Ok(ArtValue::Float(l + r)),
                        (ArtValue::String(l), ArtValue::String(r)) => Ok(ArtValue::String(
                            std::sync::Arc::from(format!("{}{}", l, r)),
                        )),
                        (ArtValue::Int(l), ArtValue::Float(r)) => {
                            Ok(ArtValue::Float(*l as f64 + r))
                        }
                        (ArtValue::Float(l), ArtValue::Int(r)) => {
                            Ok(ArtValue::Float(l + *r as f64))
                        }
                        _ => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Type mismatch.".to_string(),
                                Span::new(
                                    operator.start,
                                    operator.end,
                                    operator.line,
                                    operator.col,
                                ),
                            ));
                            Ok(ArtValue::none())
                        }
                    },
                    core::TokenType::Minus => self.binary_num_op(left_val, right_val, |a, b| a - b),
                    core::TokenType::Star => self.binary_num_op(left_val, right_val, |a, b| a * b),
                    core::TokenType::Slash => {
                        let div_by_zero = matches!(right_val, ArtValue::Int(0))
                            || matches!(right_val, ArtValue::Float(f) if f == 0.0);
                        if div_by_zero {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Division by zero".to_string(),
                                Span::new(
                                    operator.start,
                                    operator.end,
                                    operator.line,
                                    operator.col,
                                ),
                            ));
                            Ok(ArtValue::none())
                        } else {
                            self.binary_num_op(left_val, right_val, |a, b| a / b)
                        }
                    }
                    core::TokenType::Greater => {
                        self.binary_cmp_op(left_val, right_val, |a, b| a > b)
                    }
                    core::TokenType::GreaterEqual => {
                        self.binary_cmp_op(left_val, right_val, |a, b| a >= b)
                    }
                    core::TokenType::Less => self.binary_cmp_op(left_val, right_val, |a, b| a < b),
                    core::TokenType::LessEqual => {
                        self.binary_cmp_op(left_val, right_val, |a, b| a <= b)
                    }
                    core::TokenType::BangEqual => {
                        Ok(ArtValue::Bool(!self.is_equal(&left_val, &right_val)))
                    }
                    core::TokenType::EqualEqual => {
                        Ok(ArtValue::Bool(self.is_equal(&left_val, &right_val)))
                    }
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "Invalid operator.".to_string(),
                            Span::new(operator.start, operator.end, operator.line, operator.col),
                        ));
                        Ok(ArtValue::none())
                    }
                }
            }
            Expr::Call {
                callee,
                type_args,
                arguments,
            } => self.handle_call(*callee, type_args, arguments),
            Expr::Tuple(elements) => {
                let mut evaluated_elements = Vec::new();
                for element_expr in elements {
                    let value = self.evaluate(element_expr)?;
                    self.note_composite_child(&value);
                    evaluated_elements.push(value);
                }
                // Tuple are heap allocated like arrays for passing by reference
                Ok(self.heapify_composite(ArtValue::Tuple(evaluated_elements)))
            }
            Expr::StructInit { name, fields } => {
                let struct_def = match self.type_registry.get_struct(&name.lexeme) {
                    Some(def) => def.clone(),
                    None => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("Undefined struct '{}'.", name.lexeme),
                            Span::new(name.start, name.end, name.line, name.col),
                        ));
                        return Ok(ArtValue::none().clone());
                    }
                };
                let mut field_values = HashMap::new();
                for (field_name, field_expr) in fields {
                    let value = self.evaluate(field_expr)?;
                    self.note_composite_child(&value);
                    field_values.insert(field_name.lexeme, value);
                }
                for (field_name, _field_type) in &struct_def.fields {
                    if !field_values.contains_key(field_name) {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("Missing field '{}'.", field_name),
                            Span::new(name.start, name.end, name.line, name.col),
                        ));
                        return Ok(ArtValue::none().clone());
                    }
                }
                Ok(self.heapify_composite(ArtValue::StructInstance {
                    struct_name: name.lexeme,
                    fields: field_values,
                }))
            }
            Expr::EnumInit {
                name,
                variant,
                values,
            } => {
                let enum_name = match name {
                    Some(n) => n.lexeme,
                    None => {
                        // Inferência: procurar enum que contenha a variant de forma única
                        let mut candidate: Option<String> = None;
                        for (ename, edef) in self.type_registry.enums.iter() {
                            if edef.variants.iter().any(|(v, _)| v == &variant.lexeme) {
                                if candidate.is_some() && candidate.as_ref() != Some(ename) {
                                    self.diagnostics.push(Diagnostic::new(
                                        DiagnosticKind::Runtime,
                                        "Ambiguous enum variant shorthand.".to_string(),
                                        Span::new(
                                            variant.start,
                                            variant.end,
                                            variant.line,
                                            variant.col,
                                        ),
                                    ));
                                    return Ok(ArtValue::none());
                                }
                                candidate = Some(ename.clone());
                            }
                        }
                        match candidate {
                            Some(c) => c,
                            None => {
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    "Cannot infer enum type for shorthand initialization."
                                        .to_string(),
                                    Span::new(
                                        variant.start,
                                        variant.end,
                                        variant.line,
                                        variant.col,
                                    ),
                                ));
                                return Ok(ArtValue::none());
                            }
                        }
                    }
                };
                let enum_def = match self.type_registry.get_enum(&enum_name) {
                    Some(def) => def.clone(),
                    None => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("Undefined enum '{}'.", enum_name),
                            Span::new(variant.start, variant.end, variant.line, variant.col),
                        ));
                        return Ok(ArtValue::none());
                    }
                };
                let variant_def = match enum_def
                    .variants
                    .iter()
                    .find(|(v_name, _)| v_name == &variant.lexeme)
                {
                    Some(v) => v,
                    None => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("Invalid enum variant '{}'.", variant.lexeme),
                            Span::new(variant.start, variant.end, variant.line, variant.col),
                        ));
                        return Ok(ArtValue::none());
                    }
                };
                let mut evaluated_values = Vec::new();
                for value_expr in values {
                    let v = self.evaluate(value_expr)?;
                    self.note_composite_child(&v);
                    evaluated_values.push(v);
                }
                match &variant_def.1 {
                    Some(expected_params) => {
                        if evaluated_values.len() != expected_params.len() {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Wrong number of arguments.".to_string(),
                                Span::new(variant.start, variant.end, variant.line, variant.col),
                            ));
                            return Ok(ArtValue::none());
                        }
                    }
                    None => {
                        if !evaluated_values.is_empty() {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Wrong number of arguments.".to_string(),
                                Span::new(variant.start, variant.end, variant.line, variant.col),
                            ));
                            return Ok(ArtValue::none());
                        }
                    }
                }
                Ok(self.heapify_composite(ArtValue::EnumInstance {
                    enum_name,
                    variant: variant.lexeme,
                    values: evaluated_values,
                }))
            }
            Expr::FieldAccess { object, field } => {
                let evaluated = self.evaluate(*object)?;
                let obj_value_ref = self.resolve_composite(&evaluated).clone();
                // Normalize internal Optional representation to the language-level Option enum.
                let obj_value = match obj_value_ref {
                    ArtValue::Optional(boxed) => {
                        if let Some(v) = &*boxed {
                            ArtValue::EnumInstance {
                                enum_name: "Option".to_string(),
                                variant: "Some".to_string(),
                                values: vec![v.clone()],
                            }
                        } else {
                            ArtValue::EnumInstance {
                                enum_name: "Option".to_string(),
                                variant: "None".to_string(),
                                values: Vec::new(),
                            }
                        }
                    }
                    other => other,
                };
                use crate::field_access::{enum_method, struct_field_or_method};
                match obj_value {
                    ArtValue::Array(arr) => match field.lexeme.as_str() {
                        "sum" => {
                            let mut sum = 0;
                            for val in arr.iter() {
                                if let ArtValue::Int(n) = val {
                                    sum += n;
                                } else {
                                    self.diagnostics.push(Diagnostic::new(
                                        DiagnosticKind::Runtime,
                                        "Type mismatch in sum (expected Int)".to_string(),
                                        Span::new(field.start, field.end, field.line, field.col),
                                    ));
                                    return Ok(ArtValue::none());
                                }
                            }
                            Ok(ArtValue::Int(sum))
                        }
                        "count" => Ok(ArtValue::Int(arr.len() as i64)),
                        _ => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Type mismatch.".to_string(),
                                Span::new(field.start, field.end, field.line, field.col),
                            ));
                            Ok(ArtValue::none())
                        }
                    },
                    ArtValue::StructInstance {
                        struct_name,
                        fields,
                    } => {
                        if let Some(v) = struct_field_or_method(
                            &struct_name,
                            &fields,
                            &field,
                            &self.type_registry,
                        ) {
                            Ok(v)
                        } else {
                            let available = fields.keys().map(String::as_str);
                            let suggestion =
                                if let Some(best) = did_you_mean(&field.lexeme, available) {
                                    format!(" Did you mean '{}'?", best)
                                } else {
                                    String::new()
                                };

                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                format!(
                                    "Missing field '{}' on struct '{}'.{}",
                                    field.lexeme, struct_name, suggestion
                                ),
                                Span::new(field.start, field.end, field.line, field.col),
                            ));
                            Ok(ArtValue::none())
                        }
                    }
                    ArtValue::EnumInstance {
                        enum_name,
                        variant,
                        values,
                    } => {
                        if let Some(v) =
                            enum_method(&enum_name, &variant, &values, &field, &self.type_registry)
                        {
                            Ok(v)
                        } else {
                            // Suggest methods on the enum variant (since enum instances only have methods/values)
                            // We can check the type registry for methods on this enum type
                            let available = self
                                .type_registry
                                .get_enum(&enum_name)
                                .map(|def| {
                                    def.methods.keys().map(String::as_str).collect::<Vec<_>>()
                                })
                                .unwrap_or_default();

                            let suggestion = if let Some(best) =
                                did_you_mean(&field.lexeme, available.into_iter())
                            {
                                format!(" Did you mean '{}'?", best)
                            } else {
                                String::new()
                            };

                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                format!(
                                    "Missing field or method '{}' on enum '{}'.{}",
                                    field.lexeme, enum_name, suggestion
                                ),
                                Span::new(field.start, field.end, field.line, field.col),
                            ));
                            Ok(ArtValue::none())
                        }
                    }
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "Type mismatch.".to_string(),
                            Span::new(field.start, field.end, field.line, field.col),
                        ));
                        Ok(ArtValue::none())
                    }
                }
            }
            Expr::Weak(inner) => {
                // Açúcar: weak expr => builtin weak(expr)
                let expr = Expr::Call {
                    callee: Box::new(Expr::Variable {
                        name: Token::dummy("weak"),
                    }),
                    type_args: None,
                    arguments: vec![*inner],
                };
                self.evaluate(expr)
            }
            Expr::Unowned(inner) => {
                let expr = Expr::Call {
                    callee: Box::new(Expr::Variable {
                        name: Token::dummy("unowned"),
                    }),
                    type_args: None,
                    arguments: vec![*inner],
                };
                self.evaluate(expr)
            }
            Expr::WeakUpgrade(inner) => {
                // Açúcar: expr? -> weak_get(expr)
                let expr = Expr::Call {
                    callee: Box::new(Expr::Variable {
                        name: Token::dummy("weak_get"),
                    }),
                    type_args: None,
                    arguments: vec![*inner],
                };
                self.evaluate(expr)
            }
            Expr::UnownedAccess(inner) => {
                // Açúcar: expr! -> unowned_get(expr)
                let expr = Expr::Call {
                    callee: Box::new(Expr::Variable {
                        name: Token::dummy("unowned_get"),
                    }),
                    type_args: None,
                    arguments: vec![*inner],
                };
                self.evaluate(expr)
            }
            Expr::Cast { object, .. } => self.evaluate(*object),
            Expr::Array(elements) => {
                let mut evaluated_elements = Vec::new();
                for element in elements {
                    let v = self.evaluate(element)?;
                    self.note_composite_child(&v);
                    evaluated_elements.push(v);
                }
                Ok(self.heapify_composite(ArtValue::Array(evaluated_elements)))
            }
            Expr::SpawnActor { body } => {
                // Create a new actor from an expression context and return its handle
                let aid = self.next_actor_id;
                self.next_actor_id += 1;
                let actor_env = Rc::new(RefCell::new(Environment::new(
                    Some(self.environment.clone()),
                    0,
                    None,
                )));
                let actor = ActorState {
                    id: aid,
                    mailbox: Mailbox::new(),
                    body: VecDeque::from(body),
                    env: actor_env,
                    finished: false,
                    parked: false,
                    mailbox_limit: self.actor_mailbox_limit,
                };
                self.actors.insert(aid, actor);
                Ok(ArtValue::Actor(aid))
            }
        }
    }

    fn handle_call(
        &mut self,
        callee: Expr,
        type_args: Option<Vec<String>>,
        arguments: Vec<Expr>,
    ) -> Result<ArtValue> {
        if let Expr::Variable { name } = &callee {
            let is_defined = self.environment.borrow().get(&name.lexeme).is_some();
            if !is_defined {
                // Também checamos nos builtins antes de cair para o shell.
                // Como não estão mais no environment (otimização de cold-start),
                // precisamos verificar o registro global aqui.
                let is_builtin = PRELUDE_VALUES.with(|p| p.contains_key(name.lexeme.as_str()));
                if !is_builtin {
                    return self.run_shell_function_call(&name.lexeme, arguments);
                }
            }
        }

        let original_expr = callee.clone();
        let value = self.evaluate(callee)?;
        match value {
            ArtValue::Function(func) => self.call_function(func, type_args, arguments),
            ArtValue::Builtin(b) => self.call_builtin(b, arguments),
            ArtValue::EnumInstance {
                enum_name,
                variant,
                values,
            } if values.is_empty() => self.construct_enum_variant(enum_name, variant, arguments),
            other => self.call_fallback(original_expr, other, &arguments),
        }
    }

    fn call_function(
        &mut self,
        func: Rc<Function>,
        _type_args: Option<Vec<String>>,
        arguments: Vec<Expr>,
    ) -> Result<ArtValue> {
        // record call counter by function name (if present)
        let callee_name_opt = func.name.clone();
        if let Some(name) = &callee_name_opt {
            self.call_counters
                .entry(name.clone())
                .and_modify(|c| *c += 1)
                .or_insert(1);
        }
        // record edge from caller -> callee
        if let Some(caller) = self.fn_stack.last().and_then(|opt| opt.clone()) {
            if let Some(callee) = &callee_name_opt {
                let edge = format!("{}->{}", caller, callee);
                self.edge_counters
                    .entry(edge)
                    .and_modify(|c| *c += 1)
                    .or_insert(1);
            }
        }
        self.fn_stack.push(callee_name_opt.clone());

        let argc = arguments.len();
        if func.params.len() != argc {
            self.diagnostics.push(Diagnostic::new(
                DiagnosticKind::Runtime,
                "Wrong number of arguments.".to_string(),
                Span::new(0, 0, 0, 0),
            ));
            self.fn_stack.pop();
            return Ok(ArtValue::none());
        }

        // Avalia argumentos
        let mut evaluated_args = Vec::with_capacity(argc);
        for arg in arguments {
            evaluated_args.push(self.evaluate(arg)?);
        }

        let previous_env = self.environment.clone();
        let base_env = match func.closure.upgrade() {
            Some(env) => env,
            None => {
                self.diagnostics.push(Diagnostic::new(
                    DiagnosticKind::Runtime,
                    "Dangling closure environment".to_string(),
                    Span::new(0, 0, 0, 0),
                ));
                Rc::new(RefCell::new(Environment::new(None, 0, None)))
            }
        };

        // Arenas Implícitas para Funções (Adaptive ARC)
        let mut pushed_arena = false;
        let call_arena = if self.arena_with_active {
            self.current_arena
        } else {
            let aid = self.push_implicit_arena();
            pushed_arena = true;
            Some(aid)
        };

        let call_env = Rc::new(RefCell::new(Environment::new(
            Some(base_env.clone()),
            base_env.borrow().depth + 1,
            call_arena,
        )));
        self.environment = call_env.clone();

        // Bind parâmetros
        for (param, mut value) in func.params.iter().zip(evaluated_args.into_iter()) {
            let target_aid = call_env.borrow().associated_arena;
            self.promote_if_escaping(target_aid, &mut value);
            self.environment
                .borrow_mut()
                .define(&param.name.lexeme, value);
        }

        let result = self.execute(Rc::as_ref(&func.body).clone());

        let mut return_val = match result {
            Ok(()) => ArtValue::none(),
            Err(RuntimeError::Return(mut rv)) => {
                // Ao retornar de uma função, se estiver saindo de uma arena para uma externa, promovemos.
                let target_aid = previous_env.borrow().associated_arena;
                self.promote_if_escaping(target_aid, &mut rv);
                rv
            }
            Err(e) => {
                self.pop_implicit_arena();
                self.environment = previous_env;
                self.fn_stack.pop();
                return Err(e);
            }
        };

        // Escape analysis para closures
        if let ArtValue::Function(f) = &return_val {
            if f.retained_env.is_none() {
                if let Some(captured_env) = f.closure.upgrade() {
                    let escaped = Function {
                        name: f.name.clone(),
                        type_params: f.type_params.clone(),
                        params: f.params.clone(),
                        body: f.body.clone(),
                        closure: f.closure.clone(),
                        retained_env: Some(captured_env),
                    };
                    return_val = ArtValue::Function(Rc::new(escaped));
                }
            }
        }

        self.drop_scope_heap_objects(&call_env);
        if pushed_arena {
            self.pop_implicit_arena();
        }
        self.environment = previous_env;
        self.fn_stack.pop();

        Ok(return_val)
    }

    /// Write a simple profile JSON file to `path` containing function call counts.
    /// This implementation avoids introducing serde as a dependency by emitting
    /// a tiny JSON object manually.
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
