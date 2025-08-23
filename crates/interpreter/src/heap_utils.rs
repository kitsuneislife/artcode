use crate::heap::HeapObject;

/// Central helper to mutate the strong counter of a `HeapObject`.
/// Returns true if a decrement actually occurred (strong was > 0).
pub fn dec_strong_obj(obj: &mut HeapObject) -> bool {
    let had = obj.strong > 0;
    obj.dec_strong();
    had
}
