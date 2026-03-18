use core::ast::{Expr, Stmt};
use interpreter::interpreter::Interpreter;

#[test]
fn rebind_decrements_strong_and_runs_finalizer() {
    let mut interp = Interpreter::with_prelude();
    // Define uma finalizer que cria flag; cria x, associa finalizer e rebinds x para outro valor
    let program = vec![
        Stmt::Function {
            type_params: None,
            is_async: false,
            name: core::Token::dummy("fin"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![Stmt::Let {
                    pattern: core::ast::MatchPattern::Variable(core::Token::dummy("flag2")),
                    ty: None,
                    initializer: Expr::Literal(core::ast::ArtValue::Int(42)),
                }],
            }),
            method_owner: None,
        },
        Stmt::Block {
            statements: vec![
                Stmt::Let {
                    pattern: core::ast::MatchPattern::Variable(core::Token::dummy("x")),
                    ty: None,
                    initializer: Expr::Array(vec![
                        Expr::Literal(core::ast::ArtValue::Int(1)).into(),
                    ]),
                },
                // registrar finalizer
                Stmt::Expression(Expr::Call {
                    type_args: None,
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
                    pattern: core::ast::MatchPattern::Variable(core::Token::dummy("x")),
                    ty: None,
                    initializer: Expr::Array(vec![]),
                },
            ],
        },
    ];
    assert!(
        interp.interpret(program).is_ok(),
        "interpret program in rebind_and_finalizer.rs failed"
    );
    let got = interp.debug_get_global("flag2");
    assert!(got.is_some(), "finalizer não executou no rebind");
}
