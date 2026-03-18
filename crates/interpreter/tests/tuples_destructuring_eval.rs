use core::ast::{ArtValue, Expr, MatchPattern, Stmt};
use interpreter::interpreter::Interpreter;

#[test]
fn tuple_destructuring_assignment() {
    let mut interp = Interpreter::with_prelude();
    
    // Test: let (a, b) = (1, 2);
    // Evaluates to verify a=1 and b=2 are present in environment
    
    let program = vec![
        Stmt::Let {
            pattern: MatchPattern::Tuple(vec![
                MatchPattern::Variable(core::Token::dummy("a")),
                MatchPattern::Variable(core::Token::dummy("b")),
            ]),
            ty: None,
            initializer: Expr::Tuple(vec![
                Expr::Literal(ArtValue::Int(1)).into(),
                Expr::Literal(ArtValue::Int(2)).into(),
            ]),
        },
    ];

    assert!(
        interp.interpret(program).is_ok(),
        "interpret failed for tuple destructuring"
    );

    let val_a = interp.debug_get_global("a");
    let val_b = interp.debug_get_global("b");

    assert!(val_a.is_some(), "variable a not bound");
    assert!(val_b.is_some(), "variable b not bound");

    assert_eq!(val_a.unwrap(), ArtValue::Int(1));
    assert_eq!(val_b.unwrap(), ArtValue::Int(2));
}

#[test]
fn nested_tuple_destructuring_assignment() {
    let mut interp = Interpreter::with_prelude();
    
    // Test: let (a, (b, c)) = (1, (2, 3));
    
    let program = vec![
        Stmt::Let {
            pattern: MatchPattern::Tuple(vec![
                MatchPattern::Variable(core::Token::dummy("a")),
                MatchPattern::Tuple(vec![
                    MatchPattern::Variable(core::Token::dummy("b")),
                    MatchPattern::Variable(core::Token::dummy("c"))
                ]),
            ]),
            ty: None,
            initializer: Expr::Tuple(vec![
                Expr::Literal(ArtValue::Int(1)).into(),
                Expr::Tuple(vec![
                    Expr::Literal(ArtValue::Int(2)).into(),
                    Expr::Literal(ArtValue::Int(3)).into(),
                ]).into(),
            ]),
        },
    ];

    assert!(
        interp.interpret(program).is_ok(),
        "interpret failed for nested tuple destructuring"
    );

    let val_b = interp.debug_get_global("b");
    let val_c = interp.debug_get_global("c");

    assert_eq!(val_b.unwrap(), ArtValue::Int(2));
    assert_eq!(val_c.unwrap(), ArtValue::Int(3));
}
