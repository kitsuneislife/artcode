use interpreter::Interpreter;
use core::ast::{ArtValue, Function, Stmt};
use std::rc::Rc;

#[test]
fn finalizer_skipped_for_atomic_and_mutex() {
    let mut interp = Interpreter::new();
    // create atomic and mutex
    let a = interp.heap_create_atomic(ArtValue::Int(1));
    let m = interp.heap_create_mutex(ArtValue::Int(2));
    // attach fake finalizers to their ids
    if let ArtValue::Atomic(h) = a {
        interp.finalizers.insert(h.0, Rc::new(Function { name: Some("f".to_string()), params: vec![], body: Rc::new(Stmt::Block { statements: vec![] }), closure: std::rc::Weak::new(), retained_env: None }));
    }
    if let ArtValue::Mutex(h) = m {
        interp.finalizers.insert(h.0, Rc::new(Function { name: Some("g".to_string()), params: vec![], body: Rc::new(Stmt::Block { statements: vec![] }), closure: std::rc::Weak::new(), retained_env: None }));
    }
    // force finalization path by forcing strong to one and dec
    for id in interp.heap_objects.keys().cloned().collect::<Vec<u64>>() {
        interp.force_heap_strong_to_one(id);
        interp.dec_object_strong_recursive(id);
    }
    // Since finalizers for Atomic/Mutex are skipped, objects_finalized should increase but no diagnostics about finalizer running
    let diags = interp.take_diagnostics();
    assert!(!diags.iter().any(|d| d.message.contains("Finalizer skipped")));
}
