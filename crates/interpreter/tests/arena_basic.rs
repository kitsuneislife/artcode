use core::ast::{Expr, Stmt};
use interpreter::interpreter::Interpreter;

#[test]
fn performant_block_allocates_in_arena_and_finalizes() {
    let mut interp = Interpreter::with_prelude();
    let program = vec![Stmt::Performant {
        statements: vec![
            Stmt::Let {
                name: core::Token::dummy("a"),
                ty: None,
                initializer: Expr::Array(vec![Expr::Literal(core::ast::ArtValue::Int(1)).into()]),
            },
            Stmt::Let {
                name: core::Token::dummy("b"),
                ty: None,
                initializer: Expr::Array(vec![Expr::Literal(core::ast::ArtValue::Int(2)).into()]),
            },
        ],
    }];
    interp.interpret(program).unwrap();
    // Após sair do bloco, objetos na arena devem ter sido finalizados (objects_finalized>0)
    let report = interp.cycle_report();
    assert!(
        report.objects_finalized > 0,
        "nenhum objeto finalizado por arena"
    );
}
