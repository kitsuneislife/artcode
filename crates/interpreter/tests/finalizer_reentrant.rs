use core::ast::{Expr, Stmt};
use interpreter::interpreter::Interpreter;

#[test]
fn finalizer_reentrant_alloc_and_release_is_stable() {
    let mut interp = Interpreter::with_prelude();

    // finalizer that allocates a transient object and doesn't leak it
    let program = vec![
        // finalizer function: creates a local temp and drops it
        Stmt::Function {
            type_params: None,
            is_async: false,
            name: core::Token::dummy("fin_reentrant"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![
                    // let _tmp = []
                    Stmt::Let {
                        pattern: core::ast::MatchPattern::Variable(core::Token::dummy("_tmp")),
                        ty: None,
                        initializer: Expr::Array(vec![
                            Expr::Literal(core::ast::ArtValue::Int(99)).into(),
                        ]),
                    },
                ],
            }),
            method_owner: None,
        },
        // block creating x and registering finalizer, then removing root to trigger finalizer
        Stmt::Block {
            statements: vec![
                Stmt::Let {
                    pattern: core::ast::MatchPattern::Variable(core::Token::dummy("x")),
                    ty: None,
                    initializer: Expr::Array(vec![
                        Expr::Literal(core::ast::ArtValue::Int(1)).into(),
                    ]),
                },
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
                            name: core::Token::dummy("fin_reentrant"),
                        },
                    ],
                }),
            ],
        },
    ];

    assert!(interp.interpret(program).is_ok());
    // After block, finalizer should run and transient allocation in finalizer should be dropped
    assert!(
        interp.debug_check_invariants(),
        "invariants violated after reentrant finalizer"
    );
}
