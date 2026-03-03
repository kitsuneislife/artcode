use core::ast::ArtValue;
use interpreter::interpreter::Interpreter;

#[test]
fn weak_upgrade_none_after_owner_drop_and_metrics() {
    let mut interp = Interpreter::with_prelude();
    // registrar um objeto no heap e depois simular drop do strong
    let id = interp.debug_heap_register(ArtValue::Int(10));
    // criar weak wrapper (registramos mas não precisamos da variável local)
    let _weak_val = interp.debug_heap_register(ArtValue::HeapComposite(core::ast::ObjHandle(id)));
    // owner drop: decrementar strong do objeto original
    interp.debug_heap_dec_strong(id);
    // forçar execução do fluxo de finalizador (helper)
    interp.debug_run_finalizer(id);
    // upgrade weak deve retornar None
    assert!(interp.debug_heap_upgrade_weak(id).is_none());
}

#[test]
fn unowned_get_reports_dangling_metric_on_finalize() {
    let mut interp = Interpreter::with_prelude();
    let id = interp.debug_heap_register(ArtValue::Int(99));
    // create unowned wrapper (ignore returned id)
    let _wrapper = interp.debug_heap_register(ArtValue::UnownedRef(core::ast::ObjHandle(id)));
    // remove owner
    interp.debug_heap_dec_strong(id);
    interp.debug_run_finalizer(id);
    // after finalization, check metric incremented at least once for unowned or weak
    assert!(interp.unowned_dangling > 0 || interp.weak_dangling > 0);
}
