use core::ast::{ArtValue, ObjHandle};
use interpreter::interpreter::Interpreter;
use lexer::lexer::Lexer;
use parser::parser::Parser;

fn run(code: &str) -> (Interpreter, String) {
    let mut lexer = Lexer::new(code.to_string());
    let tokens = match lexer.scan_tokens() {
        Ok(t) => t,
        Err(e) => {
            assert!(false, "lexer scan_tokens in weak_unowned.rs failed: {:?}", e);
            Vec::new()
        }
    };
    let mut parser = Parser::new(tokens);
    let (program, diags) = parser.parse();
    assert!(diags.is_empty(), "parse diags: {:?}", diags);
    let mut interp = Interpreter::with_prelude();
    assert!(interp.interpret(program).is_ok(), "interpret program in weak_unowned.rs failed");
    (interp, code.to_string())
}

#[test]
fn weak_new_and_get_alive() {
    let (mut interp, _) = run("let a = 42; let w = weak(a); let v = weak_get(w);");
    // não deve produzir diagnósticos
    assert!(interp.take_diagnostics().is_empty());
}

#[test]
fn weak_sugar_postfix_question() {
    let (mut interp, _) = run("let a = 7; let w = weak a; let v = w?;");
    assert!(interp.take_diagnostics().is_empty());
}

#[test]
fn unowned_sugar_postfix_bang() {
    let (mut interp, _) = run("let a = 5; let u = unowned(a); let v = u!;");
    // protótipo: unowned_get não deve gerar diag se alvo vivo
    assert!(interp.take_diagnostics().is_empty());
}

#[test]
fn weak_get_dead_returns_none() {
    // criar weak e depois remover manualmente do heap
    let mut interp = Interpreter::with_prelude();
    let id = interp.debug_heap_register(ArtValue::Int(10));
    interp.debug_heap_remove(id);
    assert!(interp.debug_heap_upgrade_weak(id).is_none());
}

#[test]
fn weak_becomes_none_after_drop_run() {
    let mut interp = Interpreter::with_prelude();
    let id = interp.debug_heap_register(ArtValue::Int(123));
    let _w = ArtValue::WeakRef(ObjHandle(id));
    interp.debug_heap_remove(id);
    assert!(interp.debug_heap_upgrade_weak(id).is_none());
}

#[test]
fn rebind_decrements_strong_and_invalidates_weak_unowned() {
    // Simular rebind: criar objeto, expor weak/unowned via debug_define_global, então remover o strong
    let mut interp = Interpreter::with_prelude();
    let id = interp.debug_heap_register(ArtValue::Int(7));
    interp.debug_define_global("w", ArtValue::WeakRef(ObjHandle(id)));
    interp.debug_define_global("u", ArtValue::UnownedRef(ObjHandle(id)));
    // Simular rebind/remover a referência forte
    interp.debug_heap_remove(id);
    assert!(
        interp.debug_heap_upgrade_weak(id).is_none(),
        "weak deveria ser None após rebind simulated"
    );
    assert!(
        interp.debug_heap_get_unowned(id).is_none(),
        "unowned_get deveria ser None após rebind simulated"
    );
}

#[test]
fn drop_scope_decrements_handles() {
    // Simular drop de escopo removendo explicitamente o strong handle
    let mut interp = Interpreter::with_prelude();
    let id = interp.debug_heap_register(ArtValue::Int(11));
    interp.debug_define_global("w", ArtValue::WeakRef(ObjHandle(id)));
    // remover o strong (simula saída de escopo)
    interp.debug_heap_remove(id);
    assert!(
        interp.debug_heap_upgrade_weak(id).is_none(),
        "weak deveria ser None após scope drop simulated"
    );
}

#[test]
fn arena_finalization_decrements_and_invalidates() {
    // Simular finalização de arena removendo manualmente o objeto heap
    let mut interp = Interpreter::with_prelude();
    let id = interp.debug_heap_register(ArtValue::Int(99));
    interp.debug_define_global("w", ArtValue::WeakRef(ObjHandle(id)));
    // Simular finalização
    interp.debug_heap_remove(id);
    assert!(
        interp.debug_heap_upgrade_weak(id).is_none(),
        "weak deveria ser None após finalize arena simulated"
    );
}
