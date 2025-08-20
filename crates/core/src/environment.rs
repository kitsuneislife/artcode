use crate::ast::{ArtValue, ObjHandle};
use crate::interner::intern;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone)]
pub struct Environment {
    pub enclosing: Option<Rc<RefCell<Environment>>>,
    pub values: HashMap<&'static str, ArtValue>,
    pub strong_handles: Vec<ObjHandle>, // rastreia HeapComposite definidos neste escopo
}

impl Environment {
    pub fn new(enclosing: Option<Rc<RefCell<Environment>>>) -> Self {
        Environment {
            enclosing,
            values: HashMap::new(),
            strong_handles: Vec::new(),
        }
    }

    pub fn define(&mut self, name: &str, value: ArtValue) {
        let sym = intern(name);
        // Se já existia um valor neste escopo, e esse valor era um HeapComposite,
        // removemos uma ocorrência do handle registrado em `strong_handles`.
        // Isso evita que o mesmo objeto seja decrementado duas vezes: uma no
        // momento do rebind (Interpreter costuma chamar `dec_value_if_heap`)
        // e outra no `drop_scope_heap_objects` ao sair do escopo.
        if let Some(prev) = self.values.get(sym) {
            if let ArtValue::HeapComposite(h) = prev {
                if let Some(pos) = self.strong_handles.iter().position(|hh| hh.0 == h.0) {
                    self.strong_handles.remove(pos);
                }
            }
        }
        // Se o novo valor for um HeapComposite, rastreá-lo como um strong handle neste escopo.
        match &value {
            ArtValue::HeapComposite(h) => self.strong_handles.push(*h),
            _ => {}
        }
        // Inserir/atualizar o valor no mapa (retorno antigo será tratado acima)
        self.values.insert(sym, value);
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
}
