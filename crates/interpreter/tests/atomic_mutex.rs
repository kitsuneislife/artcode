use core::ast::{ArtValue, Expr, Stmt};
use interpreter::interpreter::Interpreter;

#[test]
fn atomic_and_mutex_basics() {
    let mut interp = Interpreter::with_prelude();
    // atomic_new(10)
    let call_atomic_new = Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable {
            name: core::Token::dummy("atomic_new"),
        }),
        arguments: vec![Expr::Literal(ArtValue::Int(10))],
    });
    interp.interpret(vec![call_atomic_new]).unwrap();
    let atomic_handle = interp.last_value.clone().expect("expected last value");
    // atomic_load(handle) -> 10
    let call_load = Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable {
            name: core::Token::dummy("atomic_load"),
        }),
        arguments: vec![Expr::Literal(atomic_handle.clone())],
    });
    interp.interpret(vec![call_load]).unwrap();
    match interp.last_value.clone().unwrap() {
        ArtValue::Int(n) => assert_eq!(n, 10),
        other => panic!("expected Int from atomic_load, got {:?}", other),
    }
    // atomic_add(handle, 5) -> returns new value 15
    let call_add = Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable {
            name: core::Token::dummy("atomic_add"),
        }),
        arguments: vec![
            Expr::Literal(atomic_handle.clone()),
            Expr::Literal(ArtValue::Int(5)),
        ],
    });
    interp.interpret(vec![call_add]).unwrap();
    match interp.last_value.clone().unwrap() {
        ArtValue::Int(n) => assert_eq!(n, 15),
        other => panic!("expected Int from atomic_add, got {:?}", other),
    }

    // mutex_new(42)
    let call_mutex_new = Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable {
            name: core::Token::dummy("mutex_new"),
        }),
        arguments: vec![Expr::Literal(ArtValue::Int(42))],
    });
    interp.interpret(vec![call_mutex_new]).unwrap();
    let mutex_handle = interp.last_value.clone().expect("expected last value");

    // mutex_lock(handle) -> true
    let call_lock = Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable {
            name: core::Token::dummy("mutex_lock"),
        }),
        arguments: vec![Expr::Literal(mutex_handle.clone())],
    });
    interp.interpret(vec![call_lock]).unwrap();
    match interp.last_value.clone().unwrap() {
        ArtValue::Bool(b) => assert!(b),
        other => panic!("expected Bool from mutex_lock, got {:?}", other),
    }
    // mutex_unlock(handle) -> true
    let call_unlock = Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable {
            name: core::Token::dummy("mutex_unlock"),
        }),
        arguments: vec![Expr::Literal(mutex_handle.clone())],
    });
    interp.interpret(vec![call_unlock]).unwrap();
    match interp.last_value.clone().unwrap() {
        ArtValue::Bool(b) => assert!(b),
        other => panic!("expected Bool from mutex_unlock, got {:?}", other),
    }
}
