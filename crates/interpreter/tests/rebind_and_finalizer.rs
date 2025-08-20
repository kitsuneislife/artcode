use core::ast::{Expr, Stmt};
use interpreter::interpreter::Interpreter;

#[test]
fn rebind_decrements_strong_and_runs_finalizer() {
    let mut interp = Interpreter::with_prelude();
    // Define uma finalizer que cria flag; cria x, associa finalizer e rebinds x para outro valor
    let program = vec![
        Stmt::Function {
            name: core::Token::dummy("fin"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![Stmt::Let {
                    name: core::Token::dummy("flag2"),
                    ty: None,
                    initializer: Expr::Literal(core::ast::ArtValue::Int(42)),
                }],
            }),
            method_owner: None,
        },
        Stmt::Block {
            statements: vec![
                Stmt::Let {
                    name: core::Token::dummy("x"),
                    ty: None,
                    initializer: Expr::Array(vec![
                        Expr::Literal(core::ast::ArtValue::Int(1)).into(),
                    ]),
                },
                // registrar finalizer
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
                // Rebind x -> outro valor; antigo deve ser dropado e finalizer executado
                Stmt::Let {
                    name: core::Token::dummy("x"),
                    ty: None,
                    initializer: Expr::Array(vec![]),
                },
            ],
        },
    ];
    interp.interpret(program).unwrap();
    let got = interp.debug_get_global("flag2");
    assert!(got.is_some(), "finalizer não executou no rebind");
}
