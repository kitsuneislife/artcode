use crate::ast::{ArtValue, ObjHandle};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

#[derive(Clone)]
pub struct Environment {
    pub enclosing: Option<Rc<RefCell<Environment>>>,
    /// Bindings in this scope.
    ///
    /// The key is `Arc<str>` rather than `&'static str` so that names are
    /// released when the scope is dropped. The previous type forced every
    /// binding name through a global interner that `Box::leak`ed it, which made
    /// memory grow with the number of programs executed — unbounded in
    /// `art lsp` and in the fuzzers, both of which run many programs in one
    /// process.
    ///
    /// `Arc<str>: Borrow<str>`, so lookups still take a plain `&str` and never
    /// allocate.
    pub values: HashMap<Arc<str>, ArtValue>,
    pub strong_handles: Vec<ObjHandle>, // rastreia HeapComposite definidos neste escopo
    pub depth: usize,
    pub associated_arena: Option<u32>,
}

impl Environment {
    pub fn new(
        enclosing: Option<Rc<RefCell<Environment>>>,
        depth: usize,
        associated_arena: Option<u32>,
    ) -> Self {
        Environment {
            enclosing,
            values: HashMap::new(),
            strong_handles: Vec::new(),
            depth,
            associated_arena,
        }
    }

    pub fn with_values(
        enclosing: Option<Rc<RefCell<Environment>>>,
        values: HashMap<Arc<str>, ArtValue>,
        depth: usize,
        associated_arena: Option<u32>,
    ) -> Self {
        Environment {
            enclosing,
            values,
            strong_handles: Vec::new(),
            depth,
            associated_arena,
        }
    }

    pub fn define(&mut self, name: &str, value: ArtValue) {
        // Se já existia um valor neste escopo, e esse valor era um HeapComposite,
        // removemos uma ocorrência do handle registrado em `strong_handles`.
        // Isso evita que o mesmo objeto seja decrementado duas vezes: uma no
        // momento do rebind (Interpreter costuma chamar `dec_value_if_heap`)
        // e outra no `drop_scope_heap_objects` ao sair do escopo.
        if let Some(ArtValue::HeapComposite(h)) = self.values.get(name)
            && let Some(pos) = self.strong_handles.iter().position(|hh| hh.0 == h.0)
        {
            self.strong_handles.remove(pos);
        }
        // Se o novo valor for um HeapComposite, rastreá-lo como um strong handle neste escopo.
        if let ArtValue::HeapComposite(h) = &value {
            self.strong_handles.push(*h)
        }
        // Rebind reaproveita a chave existente: só um binding novo aloca.
        if let Some(slot) = self.values.get_mut(name) {
            *slot = value;
        } else {
            self.values.insert(Arc::from(name), value);
        }
    }

    pub fn has_locally(&self, name: &str) -> bool {
        self.values.contains_key(name)
    }

    pub fn get(&self, name: &str) -> Option<ArtValue> {
        if let Some(value) = self.values.get(name) {
            return Some(value.clone());
        }
        if let Some(enclosing) = &self.enclosing {
            return enclosing.borrow().get(name);
        }
        None
    }

    pub fn read_for_eval(&mut self, name: &str) -> Option<ArtValue> {
        if let Some(value) = self.values.get_mut(name) {
            return match value {
                ArtValue::Capability { .. } => {
                    let moved = value.clone();
                    *value = ArtValue::MovedCapability;
                    Some(moved)
                }
                ArtValue::MovedCapability => Some(ArtValue::MovedCapability),
                _ => Some(value.clone()),
            };
        }
        if let Some(enclosing) = &self.enclosing {
            return enclosing.borrow_mut().read_for_eval(name);
        }
        None
    }
}
