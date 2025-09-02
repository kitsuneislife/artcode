use crate::type_registry::TypeRegistry;
use crate::values::{Result, RuntimeError};
use core::Token;
use core::ast::{ArtValue, Expr, Function, MatchPattern, ObjHandle, Program, Stmt};
use core::environment::Environment;
use diagnostics::{Diagnostic, DiagnosticKind, Span};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::rc::Rc;
use crate::heap_utils::dec_strong_obj;

use std::collections::BTreeMap;

#[cfg(test)]
pub mod test_helpers;

pub struct Interpreter {
    environment: Rc<RefCell<Environment>>,
    type_registry: TypeRegistry,
    pub diagnostics: Vec<Diagnostic>,
    pub last_value: Option<ArtValue>,
    pub handled_errors: usize,
    pub executed_statements: usize,
    heap_objects: HashMap<u64, crate::heap::HeapObject>,
    next_heap_id: u64,
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
    pub current_finalizer_promotion_target: Option<u32>,
    pub invariant_checks: bool,
    finalizers: HashMap<u64, Rc<Function>>, // finalizers por objeto composto
    // Arena support
    pub current_arena: Option<u32>,
    pub next_arena_id: u32,
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
            interp.finalizers.insert(h.0, Rc::new(Function { name: Some("f".to_string()), params: vec![], body: Rc::new(Stmt::Block { statements: vec![] }), closure: std::rc::Weak::new(), retained_env: None }));
        }
        if let ArtValue::Mutex(h) = m {
            interp.finalizers.insert(h.0, Rc::new(Function { name: Some("g".to_string()), params: vec![], body: Rc::new(Stmt::Block { statements: vec![] }), closure: std::rc::Weak::new(), retained_env: None }));
        }
        for id in interp.heap_objects.keys().cloned().collect::<Vec<u64>>() {
            interp.force_heap_strong_to_one(id);
            interp.dec_object_strong_recursive(id);
        }
        let diags = interp.take_diagnostics();
        // ensure we did not add a runtime diag complaining about finalizer execution (skip is allowed)
        assert!(!diags.iter().any(|d| d.message.contains("Finalizer skipped")));
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

#[derive(Clone)]
pub struct ActorState {
    pub id: u32,
    pub mailbox: Mailbox,
    pub body: VecDeque<Stmt>,
    pub env: Rc<RefCell<Environment>>,
    pub finished: bool,
    pub parked: bool,
    pub mailbox_limit: usize,
}

/// Mailbox with small-size linear insert and large-size BTreeMap per-priority buckets.
pub struct Mailbox {
    inner: MailboxImpl,
}

impl Clone for Mailbox {
    fn clone(&self) -> Self {
        Mailbox { inner: match &self.inner {
            MailboxImpl::Linear(v) => MailboxImpl::Linear(v.clone()),
            MailboxImpl::Map(m) => MailboxImpl::Map(m.clone()),
        }}
    }
}

enum MailboxImpl {
    Linear(VecDeque<core::ast::ValueEnvelope>),
    Map(BTreeMap<i32, VecDeque<core::ast::ValueEnvelope>>), // key = priority (ascending)
}

impl Mailbox {
    const MIGRATE_THRESHOLD: usize = 64; // simple heuristic

    pub fn new() -> Self {
        Mailbox { inner: MailboxImpl::Linear(VecDeque::new()) }
    }

    pub fn len(&self) -> usize {
        match &self.inner {
            MailboxImpl::Linear(v) => v.len(),
            MailboxImpl::Map(m) => m.values().map(|q| q.len()).sum(),
        }
    }

    pub fn is_empty(&self) -> bool { self.len() == 0 }

    pub fn front(&self) -> Option<&core::ast::ValueEnvelope> {
        match &self.inner {
            MailboxImpl::Linear(v) => v.front(),
            MailboxImpl::Map(m) => {
                // highest priority -> last key in BTreeMap
                m.keys().rev().next().and_then(|k| m.get(k)).and_then(|q| q.front())
            }
        }
    }

    pub fn to_vec(&self) -> Vec<core::ast::ValueEnvelope> {
        match &self.inner {
            MailboxImpl::Linear(v) => v.iter().cloned().collect(),
            MailboxImpl::Map(m) => {
                let mut out = Vec::new();
                for (&_pri, q) in m.iter().rev() { // descending priority
                    for e in q {
                        out.push(e.clone());
                    }
                }
                out
            }
        }
    }

    pub fn pop_front(&mut self) -> Option<core::ast::ValueEnvelope> {
        match &mut self.inner {
            MailboxImpl::Linear(v) => v.pop_front(),
            MailboxImpl::Map(m) => {
                if let Some((&pri, _)) = m.iter().rev().next() {
                    if let Some(q) = m.get_mut(&pri) {
                        let res = q.pop_front();
                        if q.is_empty() {
                            m.remove(&pri);
                        }
                        return res;
                    }
                }
                None
            }
        }
    }

    pub fn insert(&mut self, env: core::ast::ValueEnvelope) {
        match &mut self.inner {
            MailboxImpl::Linear(v) => {
                // linear insert by priority with FIFO among equals
                let mut insert_pos = 0usize;
                while insert_pos < v.len() {
                    if v[insert_pos].priority < env.priority {
                        break;
                    }
                    insert_pos += 1;
                }
                while insert_pos < v.len() && v[insert_pos].priority == env.priority {
                    insert_pos += 1;
                }
                v.insert(insert_pos, env);
                if v.len() > Mailbox::MIGRATE_THRESHOLD {
                    // migrate to map
                    let mut map: BTreeMap<i32, VecDeque<core::ast::ValueEnvelope>> = BTreeMap::new();
                    for e in v.drain(..) {
                        map.entry(e.priority).or_default().push_back(e);
                    }
                    self.inner = MailboxImpl::Map(map);
                }
            }
            MailboxImpl::Map(m) => {
                m.entry(env.priority).or_default().push_back(env);
            }
        }
    }

    pub fn iter(&self) -> Vec<core::ast::ValueEnvelope> { self.to_vec() }
}

impl Interpreter {
    pub fn new() -> Self {
        let global_env = Rc::new(RefCell::new(Environment::new(None)));

        global_env
            .borrow_mut()
            .define("println", ArtValue::Builtin(core::ast::BuiltinFn::Println));
        global_env
            .borrow_mut()
            .define("len", ArtValue::Builtin(core::ast::BuiltinFn::Len));
        global_env
            .borrow_mut()
            .define("type_of", ArtValue::Builtin(core::ast::BuiltinFn::TypeOf));
        global_env
            .borrow_mut()
            .define("weak", ArtValue::Builtin(core::ast::BuiltinFn::WeakNew));
        global_env
            .borrow_mut()
            .define("weak_get", ArtValue::Builtin(core::ast::BuiltinFn::WeakGet));
        global_env.borrow_mut().define(
            "unowned",
            ArtValue::Builtin(core::ast::BuiltinFn::UnownedNew),
        );
        global_env.borrow_mut().define(
            "unowned_get",
            ArtValue::Builtin(core::ast::BuiltinFn::UnownedGet),
        );
        global_env.borrow_mut().define(
            "on_finalize",
            ArtValue::Builtin(core::ast::BuiltinFn::OnFinalize),
        );
        global_env.borrow_mut().define(
            "actor_send",
            ArtValue::Builtin(core::ast::BuiltinFn::ActorSend),
        );
        global_env.borrow_mut().define(
            "actor_receive",
            ArtValue::Builtin(core::ast::BuiltinFn::ActorReceive),
        );
        global_env.borrow_mut().define(
            "actor_receive_envelope",
            ArtValue::Builtin(core::ast::BuiltinFn::ActorReceiveEnvelope),
        );
        global_env.borrow_mut().define(
            "actor_yield",
            ArtValue::Builtin(core::ast::BuiltinFn::ActorYield),
        );
        global_env.borrow_mut().define(
            "actor_set_mailbox_limit",
            ArtValue::Builtin(core::ast::BuiltinFn::ActorSetMailboxLimit),
        );
        global_env.borrow_mut().define(
            "envelope",
            ArtValue::Builtin(core::ast::BuiltinFn::EnvelopeNew),
        );
        global_env.borrow_mut().define(
            "make_envelope",
            ArtValue::Builtin(core::ast::BuiltinFn::MakeEnvelope),
        );
        global_env.borrow_mut().define(
            "run_actors",
            ArtValue::Builtin(core::ast::BuiltinFn::RunActors),
        );
        // Concurrency primitive prototypes
        global_env.borrow_mut().define(
            "atomic_new",
            ArtValue::Builtin(core::ast::BuiltinFn::AtomicNew),
        );
        global_env.borrow_mut().define(
            "atomic_load",
            ArtValue::Builtin(core::ast::BuiltinFn::AtomicLoad),
        );
        global_env.borrow_mut().define(
            "atomic_store",
            ArtValue::Builtin(core::ast::BuiltinFn::AtomicStore),
        );
        global_env.borrow_mut().define(
            "atomic_add",
            ArtValue::Builtin(core::ast::BuiltinFn::AtomicAdd),
        );
        global_env.borrow_mut().define(
            "mutex_new",
            ArtValue::Builtin(core::ast::BuiltinFn::MutexNew),
        );
        global_env.borrow_mut().define(
            "mutex_lock",
            ArtValue::Builtin(core::ast::BuiltinFn::MutexLock),
        );
        global_env.borrow_mut().define(
            "mutex_unlock",
            ArtValue::Builtin(core::ast::BuiltinFn::MutexUnlock),
        );

        Interpreter {
            environment: global_env,
            type_registry: TypeRegistry::new(),
            diagnostics: Vec::new(),
            last_value: None,
            handled_errors: 0,
            executed_statements: 0,
            heap_objects: HashMap::new(),
            next_heap_id: 1,
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
            finalizer_promotions: 0,
            finalizer_promotions_per_arena: std::collections::HashMap::new(),
            current_finalizer_promotion_target: None,
            arena_alloc_count: std::collections::HashMap::new(),
            objects_finalized_per_arena: std::collections::HashMap::new(),
            invariant_checks: false,
            finalizers: HashMap::new(),
            current_arena: None,
            next_arena_id: 1,
            actors: HashMap::new(),
            next_actor_id: 1,
            current_actor: None,
            actor_mailbox_limit: 1024,
            executing_actor: None,
            call_counters: std::collections::HashMap::new(),
            edge_counters: std::collections::HashMap::new(),
            fn_stack: Vec::new(),
        }
    }

    pub fn with_prelude() -> Self {
        let mut interp = Self::new();
        // Registrar enum Result simples (não genérica) com Ok, Err aceitando 1 valor
        use core::Token;
        let name = Token::dummy("Result");
        let variants = vec![
            (Token::dummy("Ok"), Some(vec!["T".to_string()])),
            (Token::dummy("Err"), Some(vec!["E".to_string()])),
        ];
        interp.type_registry.register_enum(name, variants);
        // Register Envelope struct type for actor messages (sender: Optional<Int>, payload: Any, priority: Int)
        interp.type_registry.register_struct(
            Token::dummy("Envelope"),
            vec![
                (Token::dummy("sender"), "Optional<Int>".to_string()),
                (Token::dummy("payload"), "Any".to_string()),
                (Token::dummy("priority"), "Int".to_string()),
            ],
        );
        interp
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
        self.heap_objects
            .insert(id, crate::heap::HeapObject::new_in_arena(id, val.clone(), arena_id));
        // Mirror heap_register behavior for arena-allocated objects as well.
        self.inc_children_strong(&val);
    // record arena allocation
    *self.arena_alloc_count.entry(arena_id).or_insert(0) += 1;
        id
    }
    pub fn debug_create_arena(&mut self) -> u32 {
        (self.next_heap_id as u32).wrapping_add(1)
    }

    fn heap_upgrade_weak(&self, id: u64) -> Option<ArtValue> {
        self.heap_objects
            .get(&id)
            .and_then(|o| if o.alive { Some(o.value.clone()) } else { None })
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
        fields.insert("kind".to_string(), ArtValue::String(std::sync::Arc::from("atomic")));
        fields.insert("value".to_string(), initial);
        let id = if let Some(aid) = self.current_arena {
            self.heap_register_in_arena(ArtValue::StructInstance { struct_name: "Atomic".to_string(), fields }, aid)
        } else {
            self.heap_register(ArtValue::StructInstance { struct_name: "Atomic".to_string(), fields })
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
                                Span::new(0,0,0,0),
                            ));
                            return None;
                        }
                    }
                    Some(other) => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("atomic_add: underlying atomic value is not an Int: {:?}", other),
                            Span::new(0,0,0,0),
                        ));
                        return None;
                    }
                    None => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "atomic_add: atomic has no 'value' field".to_string(),
                            Span::new(0,0,0,0),
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
        fields.insert("kind".to_string(), ArtValue::String(std::sync::Arc::from("mutex")));
        fields.insert("locked".to_string(), ArtValue::Bool(false));
        fields.insert("value".to_string(), initial);
        let id = if let Some(aid) = self.current_arena {
            self.heap_register_in_arena(ArtValue::StructInstance { struct_name: "Mutex".to_string(), fields }, aid)
        } else {
            self.heap_register(ArtValue::StructInstance { struct_name: "Mutex".to_string(), fields })
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
                            Span::new(0,0,0,0),
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
                            Span::new(0,0,0,0),
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
    let prev_promo_target = self.current_finalizer_promotion_target;
    self.current_finalizer_promotion_target = Some(arena_id);
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
    self.current_finalizer_promotion_target = prev_promo_target;
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
    fn resolve_composite<'a>(&'a self, v: &'a ArtValue) -> &'a ArtValue {
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

    #[inline]
    fn dec_children_strong(&mut self, v: &ArtValue) {
        match v {
            ArtValue::Array(a) => {
                for child in a {
                    if let ArtValue::HeapComposite(h) = child {
                        self.dec_object_strong_recursive(h.0);
                    }
                }
            }
            ArtValue::StructInstance { fields, .. } => {
                for child in fields.values() {
                    if let ArtValue::HeapComposite(h) = child {
                        self.dec_object_strong_recursive(h.0);
                    }
                }
            }
            ArtValue::EnumInstance { values, .. } => {
                for child in values {
                    if let ArtValue::HeapComposite(h) = child {
                        self.dec_object_strong_recursive(h.0);
                    }
                }
            }
            _ => {}
        }
    }

    fn dec_object_strong_recursive(&mut self, id: u64) {
        // Prepare debug info before taking mutable borrow to avoid borrow conflicts
        let debug_keys_opt: Option<Vec<u64>> = if self.finalizers.contains_key(&id) {
            Some(self.heap_objects.keys().cloned().collect())
        } else {
            None
        };
        // Limitar o escopo do borrow mutável para não conflitar com chamadas recursivas
        if let Some(obj) = self.heap_objects.get_mut(&id) {
            if obj.strong > 0 {
                // centralize the mutation so further changes live in one place
                if dec_strong_obj(obj) {
                    self.strong_decrements += 1;
                }
            }
            let should_recurse = !obj.alive; // caiu a zero agora
            if should_recurse {
                self.objects_finalized += 1;
                // attribute finalized object to its arena if present
                if let Some(aid) = obj.arena_id {
                    *self.objects_finalized_per_arena.entry(aid).or_insert(0) += 1;
                }
            }
            if should_recurse {
                // Executar finalizer se existir (snapshot para usar depois do borrow)
                if let Some(keys) = debug_keys_opt.as_ref() {
                    // debug info collected earlier (no-op in release)
                    let _ = keys;
                }
                let finalizer = self.finalizers.remove(&id);
                // Skip running finalizers for special heap-backed kinds (Atomic/Mutex).
                let skip_finalizer_due_to_kind = match obj.kind {
                    Some(crate::heap::HeapKind::Atomic) | Some(crate::heap::HeapKind::Mutex) => true,
                    _ => false,
                };
                // liberar filhos fortes
                let snapshot = obj.value.clone(); // clone para evitar emprestimo duplo
                // debug info omitted in release
                let _ = snapshot; // snapshot used later in logic
                // encerra o borrow mutável aqui
                let _ = obj;
                // agora podemos recursivamente decrementar filhos sem conflito de borrow
                self.dec_children_strong(&snapshot);
                // Invalidate weak/unowned wrappers that reference this object: mark as dangling
                // We record dead weak ids for metrics and later removal.
                // Note: heap_objects may have changed during finalizer execution; operate on snapshot of keys.
                let mut to_mark_dead: Vec<u64> = Vec::new();
                for (other_id, other_obj) in self.heap_objects.iter_mut() {
                    match &mut other_obj.value {
                        ArtValue::WeakRef(h) => {
                            if h.0 == id {
                                // Mark weak wrapper as dangling for metrics; upgrade will return None thereafter
                                self.weak_dangling += 1;
                                to_mark_dead.push(*other_id);
                            }
                        }
                        ArtValue::UnownedRef(h) => {
                            if h.0 == id {
                                // mark the unowned wrapper as dangling by recording metric
                                self.unowned_dangling += 1;
                                to_mark_dead.push(*other_id);
                            }
                        }
                        _ => {}
                    }
                }
                // For wrappers that we decided to mark, we don't remove heap entries; we record ids
                // no-op: metrics already updated above for unowned dangling wrappers
                if let Some(func) = finalizer {
                    if skip_finalizer_due_to_kind {
                        if self.invariant_checks {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Finalizer skipped for special heap-backed object (Atomic/Mutex)".to_string(),
                                Span::new(0,0,0,0),
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
                    self.environment = Rc::new(RefCell::new(Environment::new(Some(root.clone()))));
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
                        if let Some(aid) = self.current_finalizer_promotion_target {
                            *self.finalizer_promotions_per_arena.entry(aid).or_insert(0) += promoted;
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
            }
        }

        // Segunda fase: se o objeto foi finalizado e não tem weaks, remover do heap somente se
        // nenhum outro objeto vivo referencia este id (evita dangling handles).
        if let Some(obj2) = self.heap_objects.get(&id)
            && !obj2.alive
            && obj2.weak == 0
        {
            // verificar se algum objeto vivo referencia este id
            fn referenced_in(value: &ArtValue, target: u64) -> bool {
                match value {
                    ArtValue::HeapComposite(h) => h.0 == target,
                    ArtValue::Array(a) => a.iter().any(|e| referenced_in(e, target)),
                    ArtValue::StructInstance { fields, .. } =>
                        fields.values().any(|e| referenced_in(e, target)),
                    ArtValue::EnumInstance { values, .. } =>
                        values.iter().any(|e| referenced_in(e, target)),
                    _ => false,
                }
            }
            let mut referenced = false;
            for (_other_id, other_obj) in self.heap_objects.iter() {
                if other_obj.alive && referenced_in(&other_obj.value, id) {
                    referenced = true;
                    break;
                }
            }
            if !referenced {
                // safe to remove
                self.heap_objects.remove(&id);
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
                ArtValue::StructInstance { fields, .. } =>
                    fields.values().any(|e| referenced_in(e, target)),
                ArtValue::EnumInstance { values, .. } =>
                    values.iter().any(|e| referenced_in(e, target)),
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
                msgs.push(format!("object {} has absurd refcounts strong={} weak={}", id, obj.strong, obj.weak));
            }
            // scan children for dangling handles
            fn scan(v: &ArtValue, heap: &std::collections::HashMap<u64, crate::heap::HeapObject>, msgs: &mut Vec<String>, parent: u64) {
                match v {
                    ArtValue::HeapComposite(h) => {
                        if !heap.contains_key(&h.0) {
                            msgs.push(format!("parent {} references missing child {}", parent, h.0));
                        }
                    }
                    ArtValue::Array(a) => {
                        for e in a { scan(e, heap, msgs, parent); }
                    }
                    ArtValue::StructInstance { fields, .. } => {
                        for val in fields.values() { scan(val, heap, msgs, parent); }
                    }
                    ArtValue::EnumInstance { values, .. } => {
                        for val in values { scan(val, heap, msgs, parent); }
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

    fn execute(&mut self, stmt: Stmt) -> Result<()> {
        self.executed_statements += 1;
        match stmt {
            Stmt::Expression(expr) => {
                let val = self.evaluate(expr)?;
                self.last_value = Some(val.clone());
                Ok(())
            }
            Stmt::Let {
                name,
                ty: _,
                initializer,
            } => {
                let value = self.evaluate(initializer)?;
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
                    // Em debug, usar debug_assert para ajudar no diagnóstico sem abortar logicamente
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
                if let ArtValue::HeapComposite(h) = value {
                    env.strong_handles.push(h);
                }
                env.define(&name.lexeme, value);
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
                        // Avaliar guard (se existir) em ambiente com bindings temporário
                        if let Some(gexpr) = guard {
                            let previous_env = self.environment.clone();
                            let temp_env =
                                Rc::new(RefCell::new(Environment::new(Some(previous_env.clone()))));
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
                        let previous_env = self.environment.clone();
                        let new_env =
                            Rc::new(RefCell::new(Environment::new(Some(previous_env.clone()))));
                        self.environment = new_env.clone();
                        for (name, value) in bindings {
                            self.environment.borrow_mut().define(&name, value);
                        }
                        // Executar o corpo e garantir que mesmo em erro o escopo temporário seja limpo
                        let result = self.execute(stmt);
                        // Drop handles do env de bindings antes de restaurar
                        self.drop_scope_heap_objects(&new_env);
                        self.environment = previous_env;
                        return result;
                    }
                }
                Ok(())
            }
            Stmt::Function {
                name,
                params,
                body,
                method_owner,
                ..
            } => {
                let fn_rc = Rc::new(Function {
                    name: Some(name.lexeme.clone()),
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
            Stmt::Return { value } => {
                let return_value = match value {
                    Some(expr) => self.evaluate(expr)?,
                    None => ArtValue::none(),
                };
                // Runtime check: impedir retorno de objetos de arena para fora do bloco performant
                if let ArtValue::HeapComposite(h) = &return_value
                    && let Some(obj) = self.heap_objects.get(&h.0)
                    && let Some(aid) = obj.arena_id
                    && Some(aid) != self.current_arena
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
                // criar arena id
                let aid = self.next_arena_id;
                self.next_arena_id += 1;
                let prev_arena = self.current_arena;
                self.current_arena = Some(aid);
                // Criar frame léxico para o bloco
                let previous = self.environment.clone();
                self.environment = Rc::new(RefCell::new(Environment::new(Some(previous.clone()))));
                let scope_env = self.environment.clone();
                // Executar statements
                for s in statements {
                    if let Err(e) = self.execute(s) {
                        self.drop_scope_heap_objects(&scope_env);
                        // finalize arena (libera objetos da arena)
                        self.finalize_arena(aid);
                        self.current_arena = prev_arena;
                        self.environment = previous;
                        return Err(e);
                    }
                }
                // Limpar handles do escopo e finalizar arena
                self.drop_scope_heap_objects(&scope_env);
                self.finalize_arena(aid);
                self.current_arena = prev_arena;
                self.environment = previous;
                Ok(())
            }
            Stmt::Import { path: _ } => {
                // Import is a compile-time / resolver concern; runtime no-op for now.
                Ok(())
            }
            Stmt::SpawnActor { body } => {
                // Create a new actor with its own lexical environment snapshot
                let aid = self.next_actor_id;
                self.next_actor_id += 1;
                let actor_env = Rc::new(RefCell::new(Environment::new(Some(self.environment.clone()))));
                let actor = ActorState { id: aid, mailbox: Mailbox::new(), body: VecDeque::from(body), env: actor_env, finished: false, parked: false, mailbox_limit: self.actor_mailbox_limit };
                self.actors.insert(aid, actor);
                // Return actor handle as Actor variant (IDs still exposed as Int in tests where needed)
                self.last_value = Some(ArtValue::Actor(aid));
                Ok(())
            }
        }
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
            (MatchPattern::Binding(name), val) => {
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
        let previous = self.environment.clone();
        self.environment = Rc::new(RefCell::new(Environment::new(enclosing)));
        let scope_env = self.environment.clone();
        for statement in statements {
            if let Err(e) = self.execute(statement) {
                self.drop_scope_heap_objects(&scope_env);
                self.environment = previous;
                return Err(e);
            }
        }
        self.drop_scope_heap_objects(&scope_env);
        self.environment = previous;
        Ok(())
    }

    fn evaluate(&mut self, expr: Expr) -> Result<ArtValue> {
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
                        return Ok(ArtValue::String(std::sync::Arc::from(variant.clone())));
                    } else {
                        return Ok(ArtValue::Array(values.clone()));
                    }
                }
                match self.environment.borrow().get(&name_str) {
                    Some(v) => Ok(v.clone()),
                    None => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("Undefined variable '{}'.", name_str),
                            Span::new(name.start, name.end, name.line, name.col),
                        ));
                        Ok(ArtValue::none())
                    }
                }
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
            Expr::Call { callee, arguments } => self.handle_call(*callee, arguments),
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
                let obj_value = obj_value_ref; // owned for match
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
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                format!("Missing field '{}'.", field.lexeme),
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
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                format!("Missing field '{}'.", field.lexeme),
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
                    arguments: vec![*inner],
                };
                self.evaluate(expr)
            }
            Expr::Unowned(inner) => {
                let expr = Expr::Call {
                    callee: Box::new(Expr::Variable {
                        name: Token::dummy("unowned"),
                    }),
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
                let actor_env = Rc::new(RefCell::new(Environment::new(Some(self.environment.clone()))));
                let actor = ActorState { id: aid, mailbox: Mailbox::new(), body: VecDeque::from(body), env: actor_env, finished: false, parked: false, mailbox_limit: self.actor_mailbox_limit };
                self.actors.insert(aid, actor);
                Ok(ArtValue::Actor(aid))
            }
        }
    }

    fn handle_call(&mut self, callee: Expr, arguments: Vec<Expr>) -> Result<ArtValue> {
        let original_expr = callee.clone();
        let value = self.evaluate(callee)?;
        match value {
            ArtValue::Function(func) => self.call_function(func, arguments),
            ArtValue::Builtin(b) => self.call_builtin(b, arguments),
            ArtValue::EnumInstance {
                enum_name,
                variant,
                values,
            } if values.is_empty() => self.construct_enum_variant(enum_name, variant, arguments),
            other => self.call_fallback(original_expr, other, &arguments),
        }
    }

    fn call_function(&mut self, func: Rc<Function>, arguments: Vec<Expr>) -> Result<ArtValue> {
        // record call counter by function name (if present)
        let callee_name_opt = func.name.clone();
        if let Some(name) = &callee_name_opt {
            *self.call_counters.entry(name.clone()).or_insert(0) += 1;
        }
        // record edge from caller -> callee using fn_stack top as caller if present
        let caller_name_opt = match self.fn_stack.last() {
            Some(opt) => opt.clone(),
            None => None,
        };
        if let Some(callee) = &callee_name_opt {
            let key = match &caller_name_opt {
                Some(caller) => format!("{}->{}", caller, callee),
                None => format!("<root>->{}", callee),
            };
            *self.edge_counters.entry(key).or_insert(0) += 1;
        }
        // push callee onto stack for nested call attribution
        self.fn_stack.push(callee_name_opt.clone());
        let argc = arguments.len();
        if func.params.len() != argc {
            self.diagnostics.push(Diagnostic::new(
                DiagnosticKind::Runtime,
                "Wrong number of arguments.".to_string(),
                Span::new(0, 0, 0, 0),
            ));
            return Ok(ArtValue::none());
        }
        // Avalia argumentos uma vez
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
                Rc::new(RefCell::new(Environment::new(None)))
            }
        };
        self.environment = Rc::new(RefCell::new(Environment::new(Some(base_env))));
        // Inserir valores movendo (sem clone) consumindo o vetor
        for (param, value) in func.params.iter().zip(evaluated_args.into_iter()) {
            self.environment
                .borrow_mut()
                .define(&param.name.lexeme, value);
        }
        let result = self.execute(Rc::as_ref(&func.body).clone());
        // Garantir que handles fortes dos parâmetros (env criado acima) sejam decrementados.
        // Usamos `mem::replace` para extrair o ambiente da função sem provocar um borrow
        // imutável de `self.environment` durante a chamada de método que requer `&mut self`.
        let func_env = std::mem::replace(&mut self.environment, previous_env.clone());
        self.drop_scope_heap_objects(&func_env);
        // Restaurar ambiente anterior (usamos `previous_env` original)
        self.environment = previous_env;
    // pop fn stack
    let _ = self.fn_stack.pop();
        match result {
            Ok(()) => Ok(ArtValue::none()),
            Err(RuntimeError::Return(val)) => Ok(val),
        }
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

    fn call_builtin(&mut self, b: core::ast::BuiltinFn, arguments: Vec<Expr>) -> Result<ArtValue> {
        match b {
            core::ast::BuiltinFn::Println => {
                if let Some(first) = arguments.into_iter().next() {
                    let val = self.evaluate(first)?;
                    println!("{}", val);
                } else {
                    println!();
                }
                Ok(ArtValue::none())
            }
            core::ast::BuiltinFn::Len => {
                if let Some(first) = arguments.into_iter().next() {
                    let val = self.evaluate(first)?;
                    let n = match val {
                        ArtValue::String(ref s) => s.len() as i64,
                        ArtValue::Array(ref a) => a.len() as i64,
                        _ => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "len: unsupported type".to_string(),
                                Span::new(0, 0, 0, 0),
                            ));
                            return Ok(ArtValue::none());
                        }
                    };
                    Ok(ArtValue::Int(n))
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "len: missing argument".to_string(),
                        Span::new(0, 0, 0, 0),
                    ));
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::TypeOf => {
                if let Some(first) = arguments.into_iter().next() {
                    let val = self.evaluate(first)?;
                    let resolved = if let ArtValue::HeapComposite(h) = &val {
                        self.heap_objects
                            .get(&h.0)
                            .map(|o| &o.value)
                            .unwrap_or(&val)
                    } else {
                        &val
                    };
                    let t = match resolved {
                        ArtValue::Int(_) => "Int",
                        ArtValue::Float(_) => "Float",
                        ArtValue::String(_) => "String",
                        ArtValue::Bool(_) => "Bool",
                        ArtValue::Optional(_) => "Optional",
                        ArtValue::Array(_) => "Array",
                        ArtValue::StructInstance { .. } => "Struct",
                        ArtValue::EnumInstance { .. } => "Enum",
                        ArtValue::Function(_) => "Function",
                        ArtValue::Builtin(_) => "Builtin",
                        ArtValue::WeakRef(_) => "WeakRef",
                        ArtValue::UnownedRef(_) => "UnownedRef",
                        ArtValue::HeapComposite(_) => "Composite",
                        ArtValue::Atomic(_) => "Atomic",
                        ArtValue::Mutex(_) => "Mutex",
                        ArtValue::Actor(_) => "Actor",
                    };
                    Ok(ArtValue::String(std::sync::Arc::from(t)))
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "type_of: missing argument".to_string(),
                        Span::new(0, 0, 0, 0),
                    ));
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::WeakNew => {
                if let Some(first) = arguments.into_iter().next() {
                    // Avalia e registra objeto
                    let val = self.evaluate(first)?;
                    let (_id, handle) = match val {
                        ArtValue::HeapComposite(h) => {
                            self.inc_heap_weak(h.0);
                            (h.0, h)
                        }
                        _other => {
                            // Para tipos escalares ainda criar wrapper heap para permitir weak.
                            let id = self.heap_register(_other);
                            self.inc_heap_weak(id);
                            (id, ObjHandle(id))
                        }
                    };
                    self.weak_created += 1;
                    Ok(ArtValue::WeakRef(handle))
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "__weak: missing arg",
                        Span::new(0, 0, 0, 0),
                    ));
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::WeakGet => {
                if let Some(first) = arguments.into_iter().next() {
                    match self.evaluate(first)? {
                        ArtValue::WeakRef(h) => match self.heap_upgrade_weak(h.0) {
                            Some(v) => {
                                self.weak_upgrades += 1;
                                Ok(ArtValue::Optional(Box::new(Some(v))))
                            }
                            None => {
                                self.weak_dangling += 1;
                                Ok(ArtValue::Optional(Box::new(None)))
                            }
                        },
                        _ => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "__weak_get: expected WeakRef",
                                Span::new(0, 0, 0, 0),
                            ));
                            Ok(ArtValue::none())
                        }
                    }
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "__weak_get: missing arg",
                        Span::new(0, 0, 0, 0),
                    ));
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::UnownedNew => {
                if let Some(first) = arguments.into_iter().next() {
                    let val = self.evaluate(first)?;
                    let handle = match val {
                        ArtValue::HeapComposite(h) => h,
                        _other => {
                            let id = self.heap_register(_other);
                            ObjHandle(id)
                        }
                    };
                    self.unowned_created += 1;
                    Ok(ArtValue::UnownedRef(handle))
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "__unowned: missing arg",
                        Span::new(0, 0, 0, 0),
                    ));
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::UnownedGet => {
                if let Some(first) = arguments.into_iter().next() {
                    match self.evaluate(first)? {
                        ArtValue::UnownedRef(h) => match self.heap_get_unowned(h.0) {
                            Some(v) => Ok(v),
                            None => {
                                self.unowned_dangling += 1;
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    "dangling unowned reference",
                                    Span::new(0, 0, 0, 0),
                                ));
                                Ok(ArtValue::none())
                            }
                        },
                        _ => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "__unowned_get: expected UnownedRef",
                                Span::new(0, 0, 0, 0),
                            ));
                            Ok(ArtValue::none())
                        }
                    }
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "__unowned_get: missing arg",
                        Span::new(0, 0, 0, 0),
                    ));
                    Ok(ArtValue::none())
                }
            }
            core::ast::BuiltinFn::OnFinalize => {
                if arguments.len() != 2 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "on_finalize espera 2 args",
                        Span::new(0, 0, 0, 0),
                    ));
                    return Ok(ArtValue::none());
                }
                let obj_val = self.evaluate(arguments[0].clone())?;
                let fn_val = self.evaluate(arguments[1].clone())?;
                let handle_opt = match obj_val {
                    ArtValue::HeapComposite(h) => Some(h),
                    _ => None,
                };
                let func_rc = match fn_val {
                    ArtValue::Function(f) => Some(f),
                    _ => None,
                };
                if let (Some(h), Some(frc)) = (handle_opt, func_rc) {
                    if let Some(o) = self.heap_objects.get(&h.0)
                        && o.alive
                    {
                        self.finalizers.insert(h.0, frc.clone());
                    }
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "on_finalize tipos inválidos",
                        Span::new(0, 0, 0, 0),
                    ));
                }
                Ok(ArtValue::none())
            }
            core::ast::BuiltinFn::ActorSend => {
                // Accepts actor_send(actor_id, value [, priority])
                if arguments.len() < 2 || arguments.len() > 3 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "actor_send expects 2 or 3 args".to_string(),
                        Span::new(0, 0, 0, 0),
                    ));
                    return Ok(ArtValue::none());
                }
                let aid_val = self.evaluate(arguments[0].clone())?;
                let msg_val = self.evaluate(arguments[1].clone())?;
                let priority = if arguments.len() == 3 {
                    match self.evaluate(arguments[2].clone())? {
                        ArtValue::Int(n) => n as i32,
                        _ => 0,
                    }
                } else {
                    0
                };
                // accept Actor handle variant or Int for backward compatibility
                let aid_opt = match aid_val {
                    ArtValue::Actor(id) => Some(id),
                    ArtValue::Int(n) => Some(n as u32),
                    _ => None,
                };
                if let Some(aid) = aid_opt {
                    if let Some(actor) = self.actors.get_mut(&aid) {
                        let limit = actor.mailbox_limit;
                        if actor.mailbox.len() >= limit {
                            // mailbox full: signal backpressure (return false)
                            return Ok(ArtValue::Bool(false));
                        }
                        let env = core::ast::ValueEnvelope { sender: self.current_actor, payload: msg_val, priority };
                        actor.mailbox.insert(env);
                        // If actor was parked waiting for messages, unpark it
                        if actor.parked {
                            actor.parked = false;
                        }
                        return Ok(ArtValue::Bool(true));
                    } else if let Some(exec) = &mut self.executing_actor {
                        if exec.id == aid {
                            let limit = exec.mailbox_limit;
                            if exec.mailbox.len() >= limit {
                                return Ok(ArtValue::Bool(false));
                            }
                            let env = core::ast::ValueEnvelope { sender: self.current_actor, payload: msg_val, priority };
                            exec.mailbox.insert(env);
                            if exec.parked {
                                exec.parked = false;
                            }
                            return Ok(ArtValue::Bool(true));
                        }
                    } else {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("actor_send: unknown actor id {}", aid),
                            Span::new(0, 0, 0, 0),
                        ));
                    }
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "actor_send: actor id must be Int".to_string(),
                        Span::new(0, 0, 0, 0),
                    ));
                }
                Ok(ArtValue::none())
            }
            core::ast::BuiltinFn::ActorReceive => {
                // actor_receive reads from the current actor's mailbox
                if let Some(aid) = self.current_actor {
                    // First try to get the actor from actors map
                    if let Some(actor) = self.actors.get_mut(&aid) {
                        if let Some(env) = actor.mailbox.pop_front() {
                            return Ok(env.payload);
                        } else {
                            // Park the actor: scheduler should skip it until a message arrives
                            actor.parked = true;
                            return Ok(ArtValue::Optional(Box::new(None)));
                        }
                    }
                    // If actor not found because it's currently executing and removed from map,
                    // try executing_actor
                    if let Some(exec) = &mut self.executing_actor {
                        if exec.id == aid {
                            if let Some(env) = exec.mailbox.pop_front() {
                                return Ok(env.payload);
                            } else {
                                exec.parked = true;
                                return Ok(ArtValue::Optional(Box::new(None)));
                            }
                        }
                    }
                }
                self.diagnostics.push(Diagnostic::new(
                    DiagnosticKind::Runtime,
                    "actor_receive: no current actor context".to_string(),
                    Span::new(0, 0, 0, 0),
                ));
                Ok(ArtValue::Optional(Box::new(None)))
            }
            core::ast::BuiltinFn::ActorReceiveEnvelope => {
                // Return the full envelope (sender, payload, priority) as a StructInstance
                if let Some(aid) = self.current_actor {
                    if let Some(actor) = self.actors.get_mut(&aid) {
                        if let Some(env) = actor.mailbox.pop_front() {
                            // Build a StructInstance with fields: sender, payload, priority
                            let mut fields = std::collections::HashMap::new();
                            let sender_val = match env.sender {
                                Some(s) => ArtValue::Int(s as i64),
                                None => ArtValue::Optional(Box::new(None)),
                            };
                            fields.insert("sender".to_string(), sender_val);
                            fields.insert("payload".to_string(), env.payload);
                            fields.insert("priority".to_string(), ArtValue::Int(env.priority as i64));
                            let struct_val = ArtValue::StructInstance { struct_name: "Envelope".to_string(), fields };
                            return Ok(self.heapify_composite(struct_val));
                        } else {
                            actor.parked = true;
                            return Ok(ArtValue::Optional(Box::new(None)));
                        }
                    }
                    if let Some(exec) = &mut self.executing_actor {
                        if exec.id == aid {
                            if let Some(env) = exec.mailbox.pop_front() {
                                let mut fields = std::collections::HashMap::new();
                                let sender_val = match env.sender {
                                    Some(s) => ArtValue::Int(s as i64),
                                    None => ArtValue::Optional(Box::new(None)),
                                };
                                fields.insert("sender".to_string(), sender_val);
                                fields.insert("payload".to_string(), env.payload);
                                fields.insert("priority".to_string(), ArtValue::Int(env.priority as i64));
                                let struct_val = ArtValue::StructInstance { struct_name: "Envelope".to_string(), fields };
                                return Ok(self.heapify_composite(struct_val));
                            } else {
                                exec.parked = true;
                                return Ok(ArtValue::Optional(Box::new(None)));
                            }
                        }
                    }
                }
                self.diagnostics.push(Diagnostic::new(
                    DiagnosticKind::Runtime,
                    "actor_receive_envelope: no current actor context".to_string(),
                    Span::new(0, 0, 0, 0),
                ));
                Ok(ArtValue::Optional(Box::new(None)))
            }
            core::ast::BuiltinFn::ActorSetMailboxLimit => {
                if arguments.len() != 2 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "actor_set_mailbox_limit expects 2 args".to_string(),
                        Span::new(0, 0, 0, 0),
                    ));
                    return Ok(ArtValue::none());
                }
                let aid_val = self.evaluate(arguments[0].clone())?;
                let limit_val = self.evaluate(arguments[1].clone())?;
                let aid_opt = match aid_val {
                    core::ast::ArtValue::Actor(id) => Some(id),
                    core::ast::ArtValue::Int(n) => Some(n as u32),
                    _ => None,
                };
                if let (Some(aid), core::ast::ArtValue::Int(l)) = (aid_opt, limit_val) {
                    let lim = if l < 0 { 0 } else { l as usize };
                    if let Some(actor) = self.actors.get_mut(&aid) {
                        actor.mailbox_limit = lim;
                        return Ok(ArtValue::Bool(true));
                    } else {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("actor_set_mailbox_limit: unknown actor id {}", aid),
                            Span::new(0, 0, 0, 0),
                        ));
                    }
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "actor_set_mailbox_limit: invalid args".to_string(),
                        Span::new(0, 0, 0, 0),
                    ));
                }
                Ok(ArtValue::none())
            }
            core::ast::BuiltinFn::ActorYield => {
                // actor_yield is a cooperative hint; scheduler will rotate after statement
                // For runtime, just return None; scheduler sees it's a normal statement boundary.
                Ok(ArtValue::none())
            }
            core::ast::BuiltinFn::EnvelopeNew => {
                // envelope(sender, payload, priority)
                if arguments.len() != 3 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "envelope expects 3 args".to_string(),
                        Span::new(0, 0, 0, 0),
                    ));
                    return Ok(ArtValue::none());
                }
                let sender_val = self.evaluate(arguments[0].clone())?;
                let payload_val = self.evaluate(arguments[1].clone())?;
                let priority_val = self.evaluate(arguments[2].clone())?;
                let sender_field = match sender_val {
                    ArtValue::Optional(boxed) => match *boxed {
                        Some(ArtValue::Int(n)) => ArtValue::Int(n),
                        _ => ArtValue::Optional(Box::new(None)),
                    },
                    ArtValue::Int(n) => ArtValue::Int(n),
                    other => other,
                };
                let priority = if let ArtValue::Int(n) = priority_val { n as i32 } else { 0 };
                let mut fields = std::collections::HashMap::new();
                fields.insert("sender".to_string(), sender_field);
                fields.insert("payload".to_string(), payload_val);
                fields.insert("priority".to_string(), ArtValue::Int(priority as i64));
                let struct_val = ArtValue::StructInstance { struct_name: "Envelope".to_string(), fields };
                Ok(self.heapify_composite(struct_val))
            }
            core::ast::BuiltinFn::MakeEnvelope => {
                // make_envelope(payload [, priority]) -> Envelope with sender=current_actor
                if arguments.is_empty() || arguments.len() > 2 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "make_envelope expects 1 or 2 args".to_string(),
                        Span::new(0, 0, 0, 0),
                    ));
                    return Ok(ArtValue::none());
                }
                let payload_val = self.evaluate(arguments[0].clone())?;
                let priority = if arguments.len() == 2 {
                    match self.evaluate(arguments[1].clone())? {
                        ArtValue::Int(n) => n as i32,
                        _ => 0,
                    }
                } else {
                    0
                };
                let sender_field = if let Some(sid) = self.current_actor {
                    ArtValue::Int(sid as i64)
                } else {
                    ArtValue::Optional(Box::new(None))
                };
                let mut fields = std::collections::HashMap::new();
                fields.insert("sender".to_string(), sender_field);
                fields.insert("payload".to_string(), payload_val);
                fields.insert("priority".to_string(), ArtValue::Int(priority as i64));
                let struct_val = ArtValue::StructInstance { struct_name: "Envelope".to_string(), fields };
                Ok(self.heapify_composite(struct_val))
            }
            core::ast::BuiltinFn::RunActors => {
                // run_actors([max_steps]) -> drive scheduler until idle or up to max_steps
                let max_steps = if arguments.len() == 1 {
                    match self.evaluate(arguments[0].clone())? {
                        ArtValue::Int(n) if n >= 0 => n as usize,
                        _other => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "run_actors: invalid max_steps argument".to_string(),
                                Span::new(0, 0, 0, 0),
                            ));
                            return Ok(ArtValue::none());
                        }
                    }
                } else {
                    usize::MAX
                };
                self.run_actors_round_robin(max_steps);
                Ok(ArtValue::none())
            }
            // Prototype atomic/mutex builtins for performant blocks (single-threaded semantics)
            core::ast::BuiltinFn::AtomicNew => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "atomic_new expects 1 arg".to_string(),
                        Span::new(0, 0, 0, 0),
                    ));
                    return Ok(ArtValue::none());
                }
                let val = self.evaluate(arguments[0].clone())?;
                Ok(self.heap_create_atomic(val))
            }
            core::ast::BuiltinFn::AtomicLoad => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "atomic_load expects 1 arg".to_string(),
                        Span::new(0, 0, 0, 0),
                    ));
                    return Ok(ArtValue::none());
                }
                let a = self.evaluate(arguments[0].clone())?;
                if let ArtValue::Atomic(h) = a {
                    return Ok(self.heap_atomic_load(h).unwrap_or(ArtValue::none()));
                }
                Ok(ArtValue::none())
            }
            core::ast::BuiltinFn::AtomicStore => {
                if arguments.len() != 2 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "atomic_store expects 2 args".to_string(),
                        Span::new(0, 0, 0, 0),
                    ));
                    return Ok(ArtValue::none());
                }
                let a = self.evaluate(arguments[0].clone())?;
                let v = self.evaluate(arguments[1].clone())?;
                if let ArtValue::Atomic(h) = a {
                    return Ok(ArtValue::Bool(self.heap_atomic_store(h, v)));
                }
                Ok(ArtValue::Bool(false))
            }
            core::ast::BuiltinFn::AtomicAdd => {
                if arguments.len() != 2 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "atomic_add expects 2 args".to_string(),
                        Span::new(0, 0, 0, 0),
                    ));
                    return Ok(ArtValue::none());
                }
                let a = self.evaluate(arguments[0].clone())?;
                let delta = self.evaluate(arguments[1].clone())?;
                if let (ArtValue::Atomic(h), ArtValue::Int(d)) = (a, delta) {
                    if let Some(new) = self.heap_atomic_add(h, d) {
                        return Ok(ArtValue::Int(new));
                    }
                }
                Ok(ArtValue::none())
            }
            core::ast::BuiltinFn::MutexNew => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "mutex_new expects 1 arg".to_string(),
                        Span::new(0, 0, 0, 0),
                    ));
                    return Ok(ArtValue::none());
                }
                let v = self.evaluate(arguments[0].clone())?;
                Ok(self.heap_create_mutex(v))
            }
            core::ast::BuiltinFn::MutexLock => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "mutex_lock expects 1 arg".to_string(),
                        Span::new(0, 0, 0, 0),
                    ));
                    return Ok(ArtValue::none());
                }
                let a = self.evaluate(arguments[0].clone())?;
                if let ArtValue::Mutex(h) = a {
                    return Ok(ArtValue::Bool(self.heap_mutex_lock(h)));
                }
                Ok(ArtValue::Bool(false))
            }
            core::ast::BuiltinFn::MutexUnlock => {
                if arguments.len() != 1 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Runtime,
                        "mutex_unlock expects 1 arg".to_string(),
                        Span::new(0, 0, 0, 0),
                    ));
                    return Ok(ArtValue::none());
                }
                let a = self.evaluate(arguments[0].clone())?;
                if let ArtValue::Mutex(h) = a {
                    return Ok(ArtValue::Bool(self.heap_mutex_unlock(h)));
                }
                Ok(ArtValue::Bool(false))
            }
        }
    }

    /// Run actors in a simple round-robin scheduler. Each actor executes at most one
    /// statement per turn. Actors with empty body but non-empty mailbox will be considered runnable
    /// (so user code can `actor_receive()` in the body to consume messages). max_steps limits total turns.
    pub fn run_actors_round_robin(&mut self, max_steps: usize) {
        let mut steps = 0usize;
        let mut actor_ids: Vec<u32> = self.actors.keys().cloned().collect();
        actor_ids.sort_unstable();
        let mut idx = 0usize;
        // rotation_progress = whether any actor made progress during the current full pass
        let mut rotation_progress = false;
        while steps < max_steps && !actor_ids.is_empty() {
            if idx >= actor_ids.len() {
                // completed a full pass
                if !rotation_progress {
                    // no actor made progress during the full rotation -> quiescent
                    break;
                }
                rotation_progress = false;
                idx = 0;
            }
            let aid = actor_ids[idx];
            // If actor was removed or finished, skip
            let should_remove = if let Some(actor) = self.actors.get(&aid) {
                if actor.finished {
                    true
                } else {
                    false
                }
            } else { true };
            if should_remove {
                // remove from list
                actor_ids.remove(idx);
                continue;
            }

            // Execute one statement of the actor if available
                if let Some(actor_entry) = self.actors.remove(&aid) {
                let mut actor = actor_entry;
                // Store in executing_actor during execution to allow builtins to access
                // the actor state even though it's temporarily removed from the map.
                self.executing_actor = Some(actor.clone());
                // If parked (waiting for message) skip until unparked (actor_send will unpark)
                if actor.parked {
                    self.executing_actor = None;
                    self.actors.insert(aid, actor);
                    idx += 1;
                    continue;
                }
                // Determine if actor is runnable: has body statements or mailbox with content
                if actor.body.is_empty() && actor.mailbox.is_empty() {
                    // nothing to do for this actor; reinsert and skip
                    self.actors.insert(aid, actor);
                    idx += 1;
                    continue;
                }
                // set current actor context
                self.current_actor = Some(aid);
                if let Some(stmt) = actor.body.pop_front() {
                    // Swap environment
                    let previous_env = self.environment.clone();
                    self.environment = actor.env.clone();
                    // Execute statement; ignore return errors for now
                    let _ = self.execute(stmt);
                    // Mark that we made progress this rotation (executed a statement)
                    rotation_progress = true;
                    // Drop handles created in actor frame to avoid leaking into global
                    let env_for_drop = self.environment.clone();
                    self.drop_scope_heap_objects(&env_for_drop);
                    // restore env
                    actor.env = self.environment.clone();
                    self.environment = previous_env;
                } else {
                    // No statements; actor may be waiting for mailbox messages handled by actor_receive
                    // nothing to step here
                }
                // Clear current actor and executing_actor
                self.current_actor = None;
                self.executing_actor = None;
                // If actor has no body and mailbox empty, mark finished
                if actor.body.is_empty() && actor.mailbox.is_empty() {
                    actor.finished = true;
                }
                // reinsert actor state
                self.actors.insert(aid, actor);
            }

            steps += 1;
            idx += 1;
        }
        // Cleanup finished actors
        let finished_ids: Vec<u32> = self
            .actors
            .iter()
            .filter_map(|(id, a)| if a.finished { Some(*id) } else { None })
            .collect();
        for id in finished_ids {
            self.actors.remove(&id);
        }
    }

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

#[derive(Debug, Clone, PartialEq)]
pub struct CycleReport {
    pub weak_total: usize,
    pub weak_alive: usize,
    pub weak_dead: usize,
    pub unowned_total: usize,
    pub unowned_dangling: usize,
    pub objects_finalized: usize,
    pub heap_alive: usize,
    pub avg_out_degree: f32,
    pub avg_in_degree: f32,
    pub candidate_owner_edges: Vec<(u64, u64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CycleDetectionResult {
    pub cycles: Vec<CycleInfo>, // info detalhada sobre cada SCC >1
    pub weak_dead: Vec<u64>,    // ids de weak mortos
    pub unowned_dangling: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CycleInfo {
    pub nodes: Vec<u64>, // endereços (placeholder de id de objeto composto)
    pub isolated: bool,  // nenhum edge forte de fora do ciclo -> potencial vazamento
    pub suggested_break_edges: Vec<(u64, u64)>, // pares (from,to) sugeridos para enfraquecer
    pub reachable_from_root: bool, // algum nó alcançável de root global
    pub leak_candidate: bool, // isolated && !reachable_from_root
    pub ranked_suggestions: Vec<(u64, u64, u32)>, // (from,to,score)
}

impl Interpreter {
    // Protótipo: coleta ids de weak/unowned mortos; sem grafo real ainda.
    pub fn detect_cycles(&mut self) -> CycleDetectionResult {
        use std::collections::{HashMap, HashSet};
        // 1. Coletar weak/unowned mortos (ids) via varredura ambiente
        let mut weak_dead: Vec<u64> = Vec::new();
        let mut unowned_dangling: Vec<u64> = Vec::new();
        fn scan_ids(
            v: &ArtValue,
            this: &Interpreter,
            weak_dead: &mut Vec<u64>,
            unowned_dangling: &mut Vec<u64>,
        ) {
            match v {
                ArtValue::WeakRef(h) => {
                    if !this.is_object_alive(h.0) {
                        weak_dead.push(h.0);
                    }
                }
                ArtValue::UnownedRef(h) => {
                    if !this.is_object_alive(h.0) {
                        unowned_dangling.push(h.0);
                    }
                }
                ArtValue::HeapComposite(h) => {
                    if let Some(obj) = this.heap_objects.get(&h.0) {
                        scan_ids(&obj.value, this, weak_dead, unowned_dangling);
                    }
                }
                ArtValue::Array(a) => {
                    for e in a {
                        scan_ids(e, this, weak_dead, unowned_dangling)
                    }
                }
                ArtValue::StructInstance { fields, .. } => {
                    for val in fields.values() {
                        scan_ids(val, this, weak_dead, unowned_dangling)
                    }
                }
                ArtValue::EnumInstance { values, .. } => {
                    for val in values {
                        scan_ids(val, this, weak_dead, unowned_dangling)
                    }
                }
                _ => {}
            }
        }
        for (_k, v) in self.environment.borrow().values.iter() {
            scan_ids(v, self, &mut weak_dead, &mut unowned_dangling);
        }
        // 2. Construir grafo usando heap ids (apenas objetos vivos)
        let mut edges: HashMap<u64, Vec<u64>> = HashMap::new();
        let mut incoming: HashMap<u64, Vec<u64>> = HashMap::new();
        for (id, obj) in self.heap_objects.iter() {
            if !obj.alive {
                continue;
            }
            match &obj.value {
                ArtValue::Array(a) => {
                    for child in a {
                        if let ArtValue::HeapComposite(h) = child
                            && let Some(c) = self.heap_objects.get(&h.0)
                            && c.alive
                        {
                            edges.entry(*id).or_default().push(h.0);
                            incoming.entry(h.0).or_default().push(*id);
                        }
                    }
                }
                ArtValue::StructInstance { fields, .. } => {
                    for child in fields.values() {
                        if let ArtValue::HeapComposite(h) = child
                            && let Some(c) = self.heap_objects.get(&h.0)
                            && c.alive
                        {
                            edges.entry(*id).or_default().push(h.0);
                            incoming.entry(h.0).or_default().push(*id);
                        }
                    }
                }
                ArtValue::EnumInstance { values, .. } => {
                    for child in values {
                        if let ArtValue::HeapComposite(h) = child
                            && let Some(c) = self.heap_objects.get(&h.0)
                            && c.alive
                        {
                            edges.entry(*id).or_default().push(h.0);
                            incoming.entry(h.0).or_default().push(*id);
                        }
                    }
                }
                _ => {}
            }
        }
        // 3. Raízes: objetos vivos que não aparecem como target em incoming
        let mut all_ids: HashSet<u64> = self
            .heap_objects
            .iter()
            .filter(|(_, o)| o.alive)
            .map(|(id, _)| *id)
            .collect();
        for tgt in incoming.keys() {
            all_ids.remove(tgt);
        }
        let roots: Vec<u64> = all_ids.into_iter().collect();
        // 4. Tarjan SCC sobre ids vivos
        // Mapear id -> idx
        let mut id_vec: Vec<u64> = self
            .heap_objects
            .iter()
            .filter(|(_, o)| o.alive)
            .map(|(id, _)| *id)
            .collect();
        id_vec.sort_unstable();
        let mut pos: HashMap<u64, usize> = HashMap::new();
        for (i, id) in id_vec.iter().enumerate() {
            pos.insert(*id, i);
        }
        let n = id_vec.len();
        let mut index = 0usize;
        let mut indices = vec![usize::MAX; n];
        let mut lowlink = vec![0usize; n];
        let mut on_stack = vec![false; n];
        let mut stack: Vec<usize> = Vec::new();
        let mut sccs: Vec<Vec<usize>> = Vec::new();
        #[allow(clippy::too_many_arguments)]
        fn strongconnect(
            u: usize,
            index: &mut usize,
            indices: &mut [usize],
            low: &mut [usize],
            stack: &mut Vec<usize>,
            on: &mut [bool],
            edges: &HashMap<u64, Vec<u64>>,
            id_vec: &[u64],
            pos: &HashMap<u64, usize>,
            sccs: &mut Vec<Vec<usize>>,
        ) {
            indices[u] = *index;
            low[u] = *index;
            *index += 1;
            stack.push(u);
            on[u] = true;
            if let Some(neigh_ids) = edges.get(&id_vec[u]) {
                for vid in neigh_ids {
                    if let Some(&v) = pos.get(vid) {
                        if indices[v] == usize::MAX {
                            strongconnect(
                                v, index, indices, low, stack, on, edges, id_vec, pos, sccs,
                            );
                            low[u] = low[u].min(low[v]);
                        } else if on[v] {
                            low[u] = low[u].min(indices[v]);
                        }
                    }
                }
            }
            if low[u] == indices[u] {
                let mut comp = Vec::new();
                while let Some(w) = stack.pop() {
                    on[w] = false;
                    comp.push(w);
                    if w == u {
                        break;
                    }
                }
                if comp.len() > 1 {
                    sccs.push(comp);
                }
            }
        }
        for u in 0..n {
            if indices[u] == usize::MAX {
                strongconnect(
                    u,
                    &mut index,
                    &mut indices,
                    &mut lowlink,
                    &mut stack,
                    &mut on_stack,
                    &edges,
                    &id_vec,
                    &pos,
                    &mut sccs,
                );
            }
        }
        // 5. Alcançabilidade a partir de roots
        let mut reachable = vec![false; n];
        fn dfs(
            u: usize,
            edges: &HashMap<u64, Vec<u64>>,
            id_vec: &[u64],
            pos: &HashMap<u64, usize>,
            seen: &mut [bool],
        ) {
            if seen[u] {
                return;
            }
            seen[u] = true;
            if let Some(neigh) = edges.get(&id_vec[u]) {
                for vid in neigh {
                    if let Some(&v) = pos.get(vid) {
                        dfs(v, edges, id_vec, pos, seen);
                    }
                }
            }
        }
        for r in &roots {
            if let Some(&u) = pos.get(r) {
                dfs(u, &edges, &id_vec, &pos, &mut reachable);
            }
        }
        // 6. Classificar ciclos
        let mut cycles_info = Vec::new();
        let mut leaks = 0usize;
        for comp in sccs {
            let set: HashSet<usize> = comp.iter().cloned().collect();
            let mut isolated = true;
            for &node in &comp {
                if let Some(ins) = incoming.get(&id_vec[node])
                    && ins
                        .iter()
                        .any(|p| pos.get(p).map(|&pi| !set.contains(&pi)).unwrap_or(true))
                {
                    isolated = false;
                    break;
                }
            }
            let reachable_from_root = comp.iter().any(|n| reachable[*n]);
            let leak_candidate = isolated && !reachable_from_root;
            if leak_candidate {
                leaks += 1;
            }
            // sugestões simples: arestas internas saindo do primeiro
            let suggestions = comp
                .first()
                .and_then(|first| {
                    edges.get(&id_vec[*first]).map(|outs| {
                        outs.iter()
                            .filter_map(|cid| {
                                if let Some(&ci) = pos.get(cid) {
                                    if set.contains(&ci) {
                                        Some((id_vec[*first], *cid))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .unwrap_or_default();
            // ranking
            let mut in_counts: HashMap<usize, u32> = HashMap::new();
            for &nidx in &comp {
                if let Some(ins) = incoming.get(&id_vec[nidx]) {
                    for pid in ins {
                        if let Some(&pi) = pos.get(pid)
                            && set.contains(&pi)
                        {
                            *in_counts.entry(nidx).or_insert(0) += 1;
                        }
                    }
                }
            }
            let mut ranked = Vec::new();
            for &nidx in &comp {
                if let Some(outs) = edges.get(&id_vec[nidx]) {
                    let internal: Vec<u64> = outs
                        .iter()
                        .copied()
                        .filter(|cid| pos.get(cid).map(|ci| set.contains(ci)).unwrap_or(false))
                        .collect();
                    let out_deg = internal.len() as u32;
                    for tgt in internal {
                        if let Some(&ti) = pos.get(&tgt) {
                            let score = out_deg + *in_counts.get(&ti).unwrap_or(&0);
                            ranked.push((id_vec[nidx], tgt, score));
                        }
                    }
                }
            }
            ranked.sort_by(|a, b| b.2.cmp(&a.2));
            ranked.truncate(3);
            cycles_info.push(CycleInfo {
                nodes: comp.iter().map(|n| id_vec[*n]).collect(),
                isolated,
                suggested_break_edges: suggestions,
                reachable_from_root,
                leak_candidate,
                ranked_suggestions: ranked,
            });
        }
        self.cycle_leaks_detected += leaks;
        CycleDetectionResult {
            cycles: cycles_info,
            weak_dead,
            unowned_dangling,
        }
    }

    /// Serializa resumo + resultado em JSON simples (sem escapagem avançada; valores numéricos e arrays apenas)
    pub fn detect_cycles_json(&mut self) -> String {
        let summary = self.cycle_report();
        let result = self.detect_cycles();
        let mut s = String::from("{");
        use std::fmt::Write;
        let owner_edges = summary
            .candidate_owner_edges
            .iter()
            .map(|(a, b)| format!("[{},{}]", a, b))
            .collect::<Vec<_>>()
            .join(",");
        let _ = write!(
            s,
            "\"summary\":{{\"weak_total\":{},\"weak_alive\":{},\"weak_dead\":{},\"unowned_total\":{},\"unowned_dangling\":{},\"objects_finalized\":{},\"heap_alive\":{},\"avg_out_degree\":{:.2},\"avg_in_degree\":{:.2},\"candidate_owner_edges\":[{}],\"cycle_leaks_detected\":{}}}",
            summary.weak_total,
            summary.weak_alive,
            summary.weak_dead,
            summary.unowned_total,
            summary.unowned_dangling,
            summary.objects_finalized,
            summary.heap_alive,
            summary.avg_out_degree,
            summary.avg_in_degree,
            owner_edges,
            self.cycle_leaks_detected
        );
        s.push(',');
        // weak_dead / unowned_dangling
        let _ = write!(
            s,
            "\"weak_dead_ids\":[{}]",
            result
                .weak_dead
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        s.push(',');
        let _ = write!(
            s,
            "\"unowned_dangling_ids\":[{}]",
            result
                .unowned_dangling
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        s.push(',');
        // cycles
        s.push_str("\"cycles\":[");
        for (i, c) in result.cycles.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let nodes = c
                .nodes
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let sugg = c
                .suggested_break_edges
                .iter()
                .map(|(a, b)| format!("[{},{}]", a, b))
                .collect::<Vec<_>>()
                .join(",");
            let ranked = c
                .ranked_suggestions
                .iter()
                .map(|(a, b, sc)| format!("[{},{} ,{}]", a, b, sc))
                .collect::<Vec<_>>()
                .join(",");
            let _ = write!(
                s,
                "{{\"nodes\":[{}],\"isolated\":{},\"reachable_from_root\":{},\"leak_candidate\":{},\"suggested_break_edges\":[{}],\"ranked_suggestions\":[{}]}}",
                nodes, c.isolated, c.reachable_from_root, c.leak_candidate, sugg, ranked
            );
        }
        s.push_str("]}");
        s
    }

    /// Versão prettificada (indentação 2 espaços)
    pub fn detect_cycles_json_pretty(&mut self) -> String {
        let mut raw = self.detect_cycles_json();
        // Simples pretty printer para nosso JSON restrito (sem strings com braces dentro)
        let mut out = String::new();
        let mut indent = 0usize;
        let bytes: Vec<char> = raw.drain(..).collect();
        let mut i = 0;
        let len = bytes.len();
        while i < len {
            let c = bytes[i];
            match c {
                '{' | '[' => {
                    out.push(c);
                    indent += 1;
                    out.push('\n');
                    out.push_str(&"  ".repeat(indent));
                }
                '}' | ']' => {
                    indent = indent.saturating_sub(1);
                    out.push('\n');
                    out.push_str(&"  ".repeat(indent));
                    out.push(c);
                }
                ',' => {
                    out.push(c);
                    out.push('\n');
                    out.push_str(&"  ".repeat(indent));
                }
                ':' => {
                    out.push(':');
                    out.push(' ');
                }
                _ => out.push(c),
            }
            i += 1;
        }
        out
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

// (Removed unused infer_type helper; now handled in dedicated type_infer module)
