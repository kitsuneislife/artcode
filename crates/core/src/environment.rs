use crate::ast::ArtValue;
use std::collections::HashMap;
use crate::interner::intern;
use std::rc::Rc;
use std::cell::RefCell;

#[derive(Clone)]
pub struct Environment {
    pub enclosing: Option<Rc<RefCell<Environment>>>,
    pub values: HashMap<&'static str, ArtValue>,
}

impl Environment {
    pub fn new(enclosing: Option<Rc<RefCell<Environment>>>) -> Self {
        Environment {
            enclosing,
            values: HashMap::new(),
        }
    }

    pub fn define(&mut self, name: &str, value: ArtValue) {
        let sym = intern(name);
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