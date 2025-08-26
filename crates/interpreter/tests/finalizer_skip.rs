use interpreter::Interpreter;
use core::ast::{ArtValue, Function, Stmt};
use std::rc::Rc;
use interpreter::test_helpers::test_helpers as th;

#[test]
fn finalizer_skipped_for_atomic_and_mutex() {
    let mut interp = Interpreter::new();
    // create atomic and mutex
    let a = th::heap_create_atomic(&mut interp, ArtValue::Int(1));
    let m = th::heap_create_mutex(&mut interp, ArtValue::Int(2));
    // attach fake finalizers to their ids
    if let ArtValue::Atomic(h) = a {
    th::insert_finalizer(&mut interp, h.0, Function { name: Some("f".to_string()), params: vec![], body: Rc::new(Stmt::Block { statements: vec![] }), closure: std::rc::Weak::new(), retained_env: None });
    }
    if let ArtValue::Mutex(h) = m {
    th::insert_finalizer(&mut interp, h.0, Function { name: Some("g".to_string()), params: vec![], body: Rc::new(Stmt::Block { statements: vec![] }), closure: std::rc::Weak::new(), retained_env: None });
    }
    // force finalization path by forcing strong to one and dec
    for id in interp.heap_objects.keys().cloned().collect::<Vec<u64>>() {
    th::force_heap_strong_to_one(&mut interp, id);
    th::dec_object_strong_recursive(&mut interp, id);
    }
    // Since finalizers for Atomic/Mutex are skipped, objects_finalized should increase but no diagnostics about finalizer running
    let diags = interp.take_diagnostics();
    assert!(!diags.iter().any(|d| d.message.contains("Finalizer skipped")));
}
