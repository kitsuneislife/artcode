use crate::type_registry::TypeRegistry;
use crate::values::{Result, RuntimeError};
use core::Token;
use core::ast::{ArtValue, Expr, Function, MatchPattern, ObjHandle, Program, Stmt};
use core::environment::Environment;
use diagnostics::{Diagnostic, DiagnosticKind, Span};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

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
    // New metrics / debug helpers
    pub finalizer_promotions: usize,
    pub invariant_checks: bool,
    finalizers: HashMap<u64, Rc<Function>>, // finalizers por objeto composto
    // Arena support
    pub current_arena: Option<u32>,
    pub next_arena_id: u32,
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
            invariant_checks: false,
            finalizers: HashMap::new(),
            current_arena: None,
            next_arena_id: 1,
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
    /// Finaliza (libera) todos objetos alocados na arena especificada.
    fn finalize_arena(&mut self, arena_id: u32) {
        // Coletar ids vivos pertencentes à arena
        let ids: Vec<u64> = self
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
        for id in ids {
            // Forçar queda de strong para 0 e disparar finalização recursiva
            // limitar o escopo do borrow mutável para evitar conflitos durante a recursão
            if let Some(obj) = self.heap_objects.get_mut(&id) {
                // garantir que pelo menos um dec fará com que alive=false
                if obj.strong > 0 {
                    obj.strong = 1;
                }
            }
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
                        && let Some(c) = self.heap_objects.get_mut(&h.0)
                    {
                        c.inc_strong();
                        self.strong_increments += 1;
                    }
                }
            }
            ArtValue::StructInstance { fields, .. } => {
                for child in fields.values() {
                    if let ArtValue::HeapComposite(h) = child
                        && let Some(c) = self.heap_objects.get_mut(&h.0)
                    {
                        c.inc_strong();
                        self.strong_increments += 1;
                    }
                }
            }
            ArtValue::EnumInstance { values, .. } => {
                for child in values {
                    if let ArtValue::HeapComposite(h) = child
                        && let Some(c) = self.heap_objects.get_mut(&h.0)
                    {
                        c.inc_strong();
                        self.strong_increments += 1;
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
                obj.dec_strong();
                self.strong_decrements += 1;
            }
            let should_recurse = !obj.alive; // caiu a zero agora
            if should_recurse {
                self.objects_finalized += 1;
            }
            if should_recurse {
                // Executar finalizer se existir (snapshot para usar depois do borrow)
                if let Some(keys) = debug_keys_opt.as_ref() {
                    // debug info collected earlier (no-op in release)
                    let _ = keys;
                }
                let finalizer = self.finalizers.remove(&id);
                // liberar filhos fortes
                let snapshot = obj.value.clone(); // clone para evitar emprestimo duplo
                let _ = snapshot; // snapshot used later in logic
                // encerra o borrow mutável aqui
                let _ = obj;
                // agora podemos recursivamente decrementar filhos sem conflito de borrow
                self.dec_children_strong(&snapshot);
                if let Some(func) = finalizer {
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

        // Segunda fase: se o objeto foi finalizado e não tem weaks, removê-lo do heap para liberar memória
        if let Some(obj2) = self.heap_objects.get(&id)
            && !obj2.alive
            && obj2.weak == 0
        {
            // Removing dead object from heap
            self.heap_objects.remove(&id);
        }
    }

    /// Debug/testing: registra valor e retorna id (não otimizado; sem coleta real ainda)
    pub fn debug_heap_register(&mut self, v: ArtValue) -> u64 {
        self.heap_register(v)
    }
    /// Debug/testing: remove id simulando queda de último strong ref
    pub fn debug_heap_remove(&mut self, id: u64) {
        if let Some(obj) = self.heap_objects.get_mut(&id) {
            obj.dec_strong();
        }
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
    pub fn debug_heap_dec_strong(&mut self, id: u64) {
        if let Some(obj) = self.heap_objects.get_mut(&id) {
            obj.dec_strong();
        }
    }
    pub fn debug_heap_inc_weak(&mut self, id: u64) {
        if let Some(obj) = self.heap_objects.get_mut(&id) {
            obj.inc_weak();
        }
    }

    /// Test helper: decrementa contador weak (para simulação em testes)
    pub fn debug_heap_dec_weak(&mut self, id: u64) {
        if let Some(obj) = self.heap_objects.get_mut(&id) {
            obj.dec_weak();
        }
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
        for id in dead_ids {
            self.heap_objects.remove(&id);
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
    // define silently in debug helper
        self.environment.borrow_mut().define(name, val);
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
        match result {
            Ok(()) => Ok(ArtValue::none()),
            Err(RuntimeError::Return(val)) => Ok(val),
        }
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
                            if let Some(obj) = self.heap_objects.get_mut(&h.0) {
                                obj.inc_weak();
                            }
                            (h.0, h)
                        }
                        other => {
                            // Para tipos escalares ainda criar wrapper heap para permitir weak.
                            let id = self.heap_register(other);
                            if let Some(obj) = self.heap_objects.get_mut(&id) {
                                obj.inc_weak();
                            }
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
                        other => {
                            let id = self.heap_register(other);
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

    /// Versão prettificada (indentação 2 espaços) para debug humano.
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
