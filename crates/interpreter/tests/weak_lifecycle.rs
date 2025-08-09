use interpreter::interpreter::Interpreter; // ajustar caminho se necessário
use core::ast::ArtValue;

#[test]
fn weak_lifecycle_transitions_to_dead_after_dec_strong() {
    let mut interp = Interpreter::new();
    // Registra objeto forte inicial
    let id = interp.debug_heap_register(ArtValue::Int(123));
    // Cria weak ref manualmente e armazena no ambiente global para ser contabilizado no cycle_report
    let weak_val = ArtValue::WeakRef(core::ast::ObjHandle(id));
    interp.debug_define_global("w", weak_val.clone());

    // Antes: upgrade deve funcionar
    let up = interp.debug_heap_upgrade_weak(id);
    assert!(up.is_some(), "Weak upgrade deveria retornar valor enquanto alive");

    // Relatório inicial: 1 weak alive
    let rep1 = interp.cycle_report();
    assert_eq!(rep1.weak_total, 1);
    assert_eq!(rep1.weak_alive, 1);
    assert_eq!(rep1.weak_dead, 0);

    // Simula queda do último strong
    interp.debug_heap_dec_strong(id);

    // Agora upgrade deve falhar
    let up2 = interp.debug_heap_upgrade_weak(id);
    assert!(up2.is_none(), "Weak upgrade deveria falhar após strong==0/alive=false");

    // Relatório atualizado: 1 weak dead
    let rep2 = interp.cycle_report();
    assert_eq!(rep2.weak_total, 1);
    assert_eq!(rep2.weak_alive + rep2.weak_dead, 1);
    assert_eq!(rep2.weak_dead, 1, "Esperava weak_dead=1 após dec_strong");
}
