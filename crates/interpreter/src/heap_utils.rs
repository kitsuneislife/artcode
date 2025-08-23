use crate::heap::HeapObject;

/// Utilities that mutate a HeapObject directly. These helpers take only
/// &mut HeapObject to avoid borrowing Interpreter while callers may already
/// hold a mutable reference into the heap map. They do not update Interpreter
/// metrics; callers should update metrics when appropriate.
pub fn inc_strong_obj(obj: &mut HeapObject) {
    obj.inc_strong();
}

pub fn dec_strong_obj(obj: &mut HeapObject) -> bool {
    let before = obj.strong;
    obj.dec_strong();
    before != obj.strong
}

pub fn inc_weak_obj(obj: &mut HeapObject) {
    obj.inc_weak();
}

pub fn dec_weak_obj(obj: &mut HeapObject) -> bool {
    let before = obj.weak;
    obj.dec_weak();
    before != obj.weak
}

pub fn force_strong_to_one_obj(obj: &mut HeapObject) {
    if obj.strong > 0 {
        obj.strong = 1;
    }
}

