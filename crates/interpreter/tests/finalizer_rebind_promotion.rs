use core::ast::{Expr, Stmt};
use interpreter::interpreter::Interpreter;

#[test]
fn finalizer_promotes_handles_to_root_during_execution() {
    let mut interp = Interpreter::with_prelude();

    // Define finalizer `fin_promote` which assigns `promoted = outside` when run.
    let program = vec![
        // finalizer function
        Stmt::Function {
            name: core::Token::dummy("fin_promote"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block { statements: vec![
                // body: let promoted = outside
                Stmt::Let {
                    name: core::Token::dummy("promoted"),
                    ty: None,
                    initializer: Expr::Variable { name: core::Token::dummy("outside") },
                }
            ]}),
            method_owner: None,
        },
        // define an outside root object that finalizer will promote
        Stmt::Let {
            name: core::Token::dummy("outside"),
            ty: None,
            initializer: Expr::Array(vec![ Expr::Literal(core::ast::ArtValue::Int(7)).into() ]),
        },
        // create block where x is created and finalizer registered, then rebind x
        Stmt::Block { statements: vec![
            Stmt::Let {
                name: core::Token::dummy("x"),
                ty: None,
                initializer: Expr::Array(vec![ Expr::Literal(core::ast::ArtValue::Int(1)).into() ]),
            },
            // register finalizer
            Stmt::Expression(Expr::Call {
                callee: Box::new(Expr::Variable { name: core::Token::dummy("on_finalize") }),
                arguments: vec![ Expr::Variable { name: core::Token::dummy("x") }, Expr::Variable { name: core::Token::dummy("fin_promote") } ],
            }),
            // rebind x to new value -> should drop the old and run finalizer which will promote `outside` into `promoted`
            Stmt::Let {
                name: core::Token::dummy("x"),
                ty: None,
                initializer: Expr::Array(vec![]),
            },
        ]},
    ];

    assert!(interp.interpret(program).is_ok());
    // finalizer should have created `promoted` in the global env
    let got = interp.debug_get_global("promoted");
    assert!(got.is_some(), "finalizer did not promote 'outside' into 'promoted'");
    assert!(interp.debug_check_invariants(), "invariants violated after promotion");
}
