use core::ast::ArtValue;
use interpreter::interpreter::Interpreter;

#[test]
fn heap_object_mirror_created() {
    let mut interp = Interpreter::with_prelude();
    let id = interp.debug_heap_register(ArtValue::Int(42));
    // Acesso interno via debug (não exposto ainda) — usamos detect_cycles JSON para inspecionar indireto
    // Como workaround: detectar upgrade de weak recém criado para garantir presença
    let weak_expr_id = interp.debug_heap_register(ArtValue::Int(7));
    assert!(interp.debug_heap_upgrade_weak(id).is_some());
    assert!(interp.debug_heap_upgrade_weak(weak_expr_id).is_some());
    // Não falha -> espelho funcional (validação superficial).
}
