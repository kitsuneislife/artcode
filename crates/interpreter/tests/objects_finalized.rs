use core::ast::{Expr, Stmt};
use interpreter::interpreter::Interpreter;

#[test]
fn objects_finalized_increment_on_scope_exit_chain() {
    let mut interp = Interpreter::with_prelude();
    // Registrar struct vazia Point para permitir instância
    interp.register_struct_for_test("Point", vec![]);

    // Programa: bloco escopo cria array com struct dentro
    let program = vec![Stmt::Block {
        statements: vec![Stmt::Let {
            name: core::Token::dummy("a"),
            ty: None,
            initializer: Expr::Array(vec![Expr::StructInit {
                name: core::Token::dummy("Point"),
                fields: vec![],
            }]),
        }],
    }];

    let before = interp.objects_finalized;
    assert!(interp.interpret(program).is_ok(), "interpret program in objects_finalized.rs failed");
    let after = interp.objects_finalized;
    // Pelo menos 1 objeto (array ou struct) deve ser finalizado ao sair do escopo.
    assert!(
        after - before >= 1,
        "esperava >=1 finalizado, got before={}, after={}",
        before,
        after
    );
}
