use interpreter::interpreter::Interpreter;
use lexer::lexer::Lexer;
use parser::parser::Parser;

fn run(code: &str) -> (Interpreter, String) {
    let mut lexer = Lexer::new(code.to_string());
    let tokens = lexer.scan_tokens().unwrap();
    let mut parser = Parser::new(tokens);
    let (program, diags) = parser.parse();
    assert!(diags.is_empty(), "parse diags: {:?}", diags);
    let mut interp = Interpreter::with_prelude();
    interp.interpret(program).unwrap();
    (interp, code.to_string())
}

#[test]
fn weak_new_and_get_alive() {
    let (mut interp, _) = run("let a = 42; let w = weak(a); let v = weak_get(w);");
    // Procurar que não haja diagnósticos de erro
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
    // protótipo: unowned_get não gera diag se alvo vivo
    assert!(interp.take_diagnostics().is_empty());
}

#[test]
fn weak_get_dead_returns_none() {
    // Simular perda: ainda não temos coleta real; usamos id inválido manualmente
    let mut interp = Interpreter::with_prelude();
    // criar weak e depois remover manualmente do heap
    let id = interp.debug_heap_register(core::ast::ArtValue::Int(10));
    let _weak = core::ast::ArtValue::WeakRef(core::ast::ObjHandle(id));
    interp.debug_heap_remove(id); // invalida
    // tentar upgrade
    assert!(interp.debug_heap_upgrade_weak(id).is_none());
}

#[test]
fn unowned_get_dangling_reports_diag() {
    let mut interp = Interpreter::with_prelude();
    let id = interp.debug_heap_register(core::ast::ArtValue::Int(99));
    let _u = core::ast::ArtValue::UnownedRef(core::ast::ObjHandle(id));
    interp.debug_heap_remove(id);
    // Força caminho de diagnóstico chamando helper interno
    assert!(interp.debug_heap_get_unowned(id).is_none());
}
