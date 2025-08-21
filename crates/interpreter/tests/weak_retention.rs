use core::ast::{ArtValue, ObjHandle};
use interpreter::interpreter::Interpreter;

#[test]
fn weak_retains_object_until_weak_zero() {
    let mut interp = Interpreter::with_prelude();
    // registrar objeto e expor um weak globalmente
    let id = interp.debug_heap_register(ArtValue::Int(200));
    // definir um weak global que evita que o runtime remova imediatamente o registro
    interp.debug_define_global("w", ArtValue::WeakRef(ObjHandle(id)));
    // remover handle forte (simula saída de escopo / rebind)
    interp.debug_heap_remove(id);
    // objeto deve ainda não ter sido removido enquanto houver weak > 0
    assert!(
        interp.debug_heap_contains(id),
        "objeto foi removido mesmo com weak>0"
    );

    // agora remover o weak (simulando decremento de weak) e forçar sweep
    interp.debug_heap_dec_weak(id);
    // forçar remoção de objetos mortos sem weaks
    interp.debug_sweep_dead();
    // agora o objeto deve ter sido removido
    assert!(
        !interp.debug_heap_contains(id),
        "objeto deveria ter sido removido apos weak==0"
    );
}

#[test]
fn unowned_diag_after_drop() {
    let mut interp = Interpreter::with_prelude();
    let id = interp.debug_heap_register(ArtValue::Int(77));
    interp.debug_define_global("u", ArtValue::UnownedRef(ObjHandle(id)));
    // remover o strong
    interp.debug_heap_remove(id);
    // unowned_get deve retornar None (API debug)
    assert!(
        interp.debug_heap_get_unowned(id).is_none(),
        "unowned_get deveria ser None depois do drop"
    );
    // Agora executar uma expressão que acessa o unowned via interpret para gerar o diagnóstico
    let code = "unowned_get(u);";
    use lexer::lexer::Lexer;
    use parser::parser::Parser;
    let mut lexer = Lexer::new(code.to_string());
    let tokens = lexer.scan_tokens().unwrap();
    let mut parser = Parser::new(tokens);
    let (program, diags) = parser.parse();
    assert!(diags.is_empty(), "parse falhou: {:?}", diags);
    interp.interpret(program).unwrap();
    let diags = interp.take_diagnostics();
    assert!(
        !diags.is_empty(),
        "esperado ao menos um diagnóstico após acesso unowned inválido"
    );
}
