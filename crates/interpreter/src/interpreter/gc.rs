use super::Interpreter;
use core::ast::{ArtValue, Stmt};
use core::environment::Environment;
use diagnostics::{Diagnostic, DiagnosticKind, Span};
use std::cell::RefCell;
use std::rc::Rc;

impl Interpreter {
    pub(super) fn drop_scope_heap_objects(&mut self, env: &Rc<RefCell<Environment>>) {
        let handles = env.borrow().strong_handles.clone();
        for h in handles {
            self.dec_object_strong_recursive(h.0);
        }
    }

    pub(super) fn dec_value_if_heap(&mut self, v: &ArtValue) {
        if let ArtValue::HeapComposite(h) = v {
            self.dec_object_strong_recursive(h.0);
        }
    }

    #[inline]
    pub(super) fn inc_children_strong(&mut self, v: &ArtValue) {
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
    pub(super) fn enqueue_children_strong(&self, v: &ArtValue, queue: &mut Vec<u64>) {
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

    pub(super) fn dec_object_strong_recursive(&mut self, start_id: u64) {
        let mut work_queue: Vec<u64> = vec![start_id];
        let mut visited: std::collections::HashSet<u64> = std::collections::HashSet::new();
        visited.insert(start_id);

        while let Some(id) = work_queue.pop() {
            let mut snapshot_to_enqueue: Option<ArtValue> = None;
            let mut finalizer_opt = None;
            let mut skip_finalizer_due_to_kind = false;

            if let Some(obj) = self.heap_objects.get_mut(&id) {
                if obj.strong > 0
                    && crate::heap_utils::dec_strong_obj(obj) {
                        self.strong_decrements += 1;
                    }

                let should_recurse = !obj.alive; // caiu a zero agora
                if should_recurse {
                    self.objects_finalized += 1;
                    if let Some(aid) = obj.arena_id {
                        *self.objects_finalized_per_arena.entry(aid).or_insert(0) += 1;
                    }

                    snapshot_to_enqueue = Some(obj.value.clone());

                    skip_finalizer_due_to_kind = matches!(
                        obj.kind,
                        Some(crate::heap::HeapKind::Atomic) | Some(crate::heap::HeapKind::Mutex)
                    );
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
                        ArtValue::WeakRef(h)
                            if h.0 == id => {
                                self.weak_dangling += 1;
                                to_mark_dead.push(*other_id);
                            }
                        ArtValue::UnownedRef(h)
                            if h.0 == id => {
                                self.unowned_dangling += 1;
                                to_mark_dead.push(*other_id);
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
        if let Some(obj) = self.heap_objects.get_mut(&id)
            && dec_weak_obj(obj) {
                // metric kept at interpreter level if callers want to track
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
        if let Some(obj) = self.heap_objects.get_mut(&id)
            && dec_strong_obj(obj) {
                self.strong_decrements += 1;
            }
    }

    // Inner helper that performs the decrement on an existing mutable reference
    // to a `HeapObject`. This avoids performing multiple `get_mut` borrows when
    // the caller already holds a mutable reference (used by finalizer flow).
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
    pub(super) fn force_heap_strong_to_one(&mut self, id: u64) {
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
                    ArtValue::HeapComposite(h)
                        if !heap.contains_key(&h.0) => {
                            msgs.push(format!(
                                "parent {} references missing child {}",
                                parent, h.0
                            ));
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
}
