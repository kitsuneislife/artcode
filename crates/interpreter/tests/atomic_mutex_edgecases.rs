use core::ast::{ArtValue, Expr, Stmt};
use interpreter::interpreter::Interpreter;

#[test]
fn atomic_add_type_error_emits_diag() {
    let mut interp = Interpreter::with_prelude();
    // Create an atomic holding a non-int (e.g., a string) via heap_create path
    let call_atomic_new = Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable {
            name: core::Token::dummy("atomic_new"),
        }),
        arguments: vec![Expr::Literal(ArtValue::String(std::sync::Arc::from("x")))],
    });
    interp.interpret(vec![call_atomic_new]).unwrap();
    let atomic_handle = interp.last_value.clone().expect("expected last value");
    // atomic_add(handle, 5) -> should emit runtime diagnostic and return none
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
    // Expect at least one runtime diagnostic mentioning atomic_add
    let diags = interp.take_diagnostics();
    assert!(diags.iter().any(|d| d.message.contains("atomic_add")));
}

#[test]
fn mutex_double_unlock_emits_diag_and_returns_false() {
    let mut interp = Interpreter::with_prelude();
    // mutex_new(1)
    let call_mutex_new = Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable {
            name: core::Token::dummy("mutex_new"),
        }),
        arguments: vec![Expr::Literal(ArtValue::Int(1))],
    });
    interp.interpret(vec![call_mutex_new]).unwrap();
    let mutex_handle = interp.last_value.clone().expect("expected last value");

    // First unlock (without lock) should emit diagnostic and return false
    let call_unlock = Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable {
            name: core::Token::dummy("mutex_unlock"),
        }),
        arguments: vec![Expr::Literal(mutex_handle.clone())],
    });
    interp.interpret(vec![call_unlock]).unwrap();
    match interp.last_value.clone().unwrap() {
        ArtValue::Bool(b) => assert!(!b),
        _ => panic!("expected Bool"),
    }
    let diags = interp.take_diagnostics();
    assert!(diags.iter().any(|d| d.message.contains("mutex_unlock")));
}

#[test]
fn atomic_and_mutex_heap_kind_set() {
    let mut interp = Interpreter::with_prelude();
    // create atomic
    let call_atomic_new = Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable {
            name: core::Token::dummy("atomic_new"),
        }),
        arguments: vec![Expr::Literal(ArtValue::Int(5))],
    });
    interp.interpret(vec![call_atomic_new]).unwrap();
    let atomic_handle = interp.last_value.clone().unwrap();
    // create mutex
    let call_mutex_new = Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable {
            name: core::Token::dummy("mutex_new"),
        }),
        arguments: vec![Expr::Literal(ArtValue::Int(7))],
    });
    interp.interpret(vec![call_mutex_new]).unwrap();
    let mutex_handle = interp.last_value.clone().unwrap();

    // Inspect heap objects and assert kind
    use interpreter::heap::HeapKind;
    if let ArtValue::Atomic(core::ast::ObjHandle(id)) = atomic_handle {
        let kind = interp.debug_heap_kind(id).expect("expected kind");
        assert!(matches!(kind, HeapKind::Atomic));
    } else {
        panic!("expected atomic handle");
    }
    if let ArtValue::Mutex(core::ast::ObjHandle(id)) = mutex_handle {
        let kind = interp.debug_heap_kind(id).expect("expected kind");
        assert!(matches!(kind, HeapKind::Mutex));
    } else {
        panic!("expected mutex handle");
    }
}
