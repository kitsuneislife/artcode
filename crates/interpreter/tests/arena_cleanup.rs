use core::ast::ArtValue;
use interpreter::interpreter::Interpreter;

#[test]
fn arena_cleanup_removes_dead_objects_without_weaks() {
    let mut interp = Interpreter::with_prelude();
    // Criar arena id simulada
    let aid = interp.debug_create_arena();
    // Registrar objeto na arena
    let id = interp.debug_heap_register_in_arena(ArtValue::Int(123), aid);
    // Confirmar que existe
    assert!(interp.debug_heap_contains(id));
    // Remover o strong (simula saída de escopo)
    interp.debug_heap_remove(id);
    // Finalizar arena explicitamente
    interp.debug_finalize_arena(aid);
    // Como não há weaks, o objeto deve ser removido do heap
    assert!(
        !interp.debug_heap_contains(id),
        "objeto de arena deveria ser removido"
    );
}
