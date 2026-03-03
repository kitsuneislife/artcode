use core::ast::{Expr, Stmt};
use interpreter::interpreter::Interpreter;

#[test]
fn finalizer_runs_on_drop() {
    let mut interp = Interpreter::with_prelude();
    // Função cria objeto e registra finalizer que cria sinal global
    // finalizer definido globalmente; objeto vive só dentro do bloco abaixo
    let program = vec![
        Stmt::Function {
            name: core::Token::dummy("fin"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![
                    // finalizer cria flag global
                    Stmt::Let {
                        name: core::Token::dummy("flag"),
                        ty: None,
                        initializer: Expr::Literal(core::ast::ArtValue::Int(1)),
                    },
                ],
            }),
            method_owner: None,
        },
        // bloco cria 'x' e registra finalizer; ao sair dele strong ref cai a zero
        Stmt::Block {
            statements: vec![
                Stmt::Let {
                    name: core::Token::dummy("x"),
                    ty: None,
                    initializer: Expr::Array(vec![]),
                },
                Stmt::Expression(Expr::Call {
                    callee: Box::new(Expr::Variable {
                        name: core::Token::dummy("on_finalize"),
                    }),
                    arguments: vec![
                        Expr::Variable {
                            name: core::Token::dummy("x"),
                        },
                        Expr::Variable {
                            name: core::Token::dummy("fin"),
                        },
                    ],
                }),
            ],
        },
    ];
    assert!(
        interp.interpret(program).is_ok(),
        "interpret program in finalizer.rs failed"
    );
    // Após execução, finalizer deve ter rodado criando variável global 'flag'
    let report = interp.cycle_report();
    eprintln!(
        "cycle report objects_finalized={} weak_total={} unowned_total={}",
        report.objects_finalized, report.weak_total, report.unowned_total
    );
    let got = interp.debug_get_global("flag");
    assert!(
        report.objects_finalized > 0,
        "nenhum objeto finalizado (report)"
    );
    assert!(got.is_some(), "finalizer não executou");
}
