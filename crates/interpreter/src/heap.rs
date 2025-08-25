use core::ast::ArtValue;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjHandle(pub u64);

/// Representa um objeto gerenciado com contadores strong/weak.
/// Protótipo: não faz coleta automática ainda; apenas mantém contagens e estado alive.
#[derive(Debug, Clone)]
pub enum HeapKind {
    Atomic,
    Mutex,
}

pub struct HeapObject {
    pub id: u64,
    pub value: ArtValue,
    pub strong: u32,
    pub weak: u32,
    pub alive: bool,
    pub arena_id: Option<u32>, // se alocado dentro de uma arena (bloco performant)
    pub kind: Option<HeapKind>,
}

impl HeapObject {
    pub fn new(id: u64, value: ArtValue) -> Self {
        Self {
            id,
            value,
            strong: 1,
            weak: 0,
            alive: true,
            arena_id: None,
            kind: None,
        }
    }
    pub fn new_in_arena(id: u64, value: ArtValue, arena: u32) -> Self {
        Self {
            id,
            value,
            strong: 1,
            weak: 0,
            alive: true,
            arena_id: Some(arena),
            kind: None,
        }
    }
    pub fn inc_strong(&mut self) {
        if self.alive {
            self.strong += 1;
        }
    }
    pub fn dec_strong(&mut self) {
        if self.strong > 0 {
            self.strong -= 1;
            if self.strong == 0 {
                self.alive = false;
            }
        }
    }
    pub fn inc_weak(&mut self) {
        if self.alive {
            self.weak += 1;
        }
    }
    pub fn dec_weak(&mut self) {
        if self.weak > 0 {
            self.weak -= 1;
        }
    }
    pub fn upgrade(&self) -> Option<&ArtValue> {
        if self.alive { Some(&self.value) } else { None }
    }
}

// Estrutura futura: substituir HashMap<u64, ArtValue> por HashMap<u64, HeapObject> em etapas.
