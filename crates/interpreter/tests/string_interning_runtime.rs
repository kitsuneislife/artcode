use core::ast::{ArtValue, Expr, MatchPattern, Stmt};
use interpreter::interpreter::Interpreter;

#[test]
fn type_of_reuses_interned_runtime_string() {
    let mut interp = Interpreter::with_prelude();

    let program = vec![
        Stmt::Let {
            pattern: MatchPattern::Variable(core::Token::dummy("a")),
            ty: None,
            initializer: Expr::Call {
                type_args: None,
                callee: Box::new(Expr::Variable {
                    name: core::Token::dummy("type_of"),
                }),
                arguments: vec![Expr::Literal(ArtValue::Int(1))],
            },
        },
        Stmt::Let {
            pattern: MatchPattern::Variable(core::Token::dummy("b")),
            ty: None,
            initializer: Expr::Call {
                type_args: None,
                callee: Box::new(Expr::Variable {
                    name: core::Token::dummy("type_of"),
                }),
                arguments: vec![Expr::Literal(ArtValue::Int(2))],
            },
        },
    ];

    assert!(interp.interpret(program).is_ok(), "interpret should succeed");

    let a = match interp.debug_get_global("a") {
        Some(ArtValue::String(s)) => s,
        other => panic!("expected string in 'a', got {:?}", other),
    };
    let b = match interp.debug_get_global("b") {
        Some(ArtValue::String(s)) => s,
        other => panic!("expected string in 'b', got {:?}", other),
    };

    assert!(
        std::sync::Arc::ptr_eq(&a, &b),
        "type_of results for same type should reuse interned Arc"
    );
}
