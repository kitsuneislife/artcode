use core::ast::{ArtValue, Expr, Stmt};
use interpreter::interpreter::Interpreter;

// If finalizer of A promotes B, B must survive finalization regardless of execution order.
#[test]
fn finalizer_promote_other_survives() {
    let mut interp = Interpreter::with_prelude();
    interp.enable_invariant_checks(true);
    let aid = interp.debug_create_arena();
    let a = interp.debug_heap_register_in_arena(ArtValue::Array(vec![ArtValue::Int(1)]), aid);
    let b = interp.debug_heap_register_in_arena(ArtValue::Array(vec![ArtValue::Int(2)]), aid);
    // finalizer for A promotes B to a global named "survivor"
    let program = vec![
        // function promote_b() { let survivor = <heapified b>; }
        Stmt::Function {
            name: core::Token::dummy("promote_b"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![Stmt::Let {
                    name: core::Token::dummy("survivor"),
                    ty: None,
                    initializer: Expr::Literal(ArtValue::HeapComposite(core::ast::ObjHandle(b))),
                }],
            }),
            method_owner: None,
        },
        // attach finalize to a
        Stmt::Expression(Expr::Call {
            callee: Box::new(Expr::Variable {
                name: core::Token::dummy("on_finalize"),
            }),
            arguments: vec![
                Expr::Literal(ArtValue::HeapComposite(core::ast::ObjHandle(a))),
                Expr::Variable {
                    name: core::Token::dummy("promote_b"),
                },
            ],
        }),
    ];
    assert!(
        interp.interpret(program).is_ok(),
        "interpret program in arena_finalizer_ordering.rs failed"
    );
    interp.debug_finalize_arena(aid);
    // survivor should be present or promotion metric show promotions
    assert!(
        interp.debug_get_global("survivor").is_some()
            || interp.get_finalizer_promotions() > 0
            || interp
                .finalizer_promotions_per_arena
                .get(&aid)
                .cloned()
                .unwrap_or(0)
                > 0,
        "B was not promoted by A's finalizer"
    );
    assert!(
        interp.debug_check_invariants(),
        "invariants broken after finalize ordering test"
    );
}

// Mutual finalizers: A promotes B; B promotes A. Ensure no invariant failures and at least one survives.
#[test]
fn mutual_finalizers_promote_each_other_safe() {
    let mut interp = Interpreter::with_prelude();
    interp.enable_invariant_checks(true);
    let aid = interp.debug_create_arena();
    let a = interp.debug_heap_register_in_arena(ArtValue::Array(vec![ArtValue::Int(10)]), aid);
    let b = interp.debug_heap_register_in_arena(ArtValue::Array(vec![ArtValue::Int(11)]), aid);
    // func promote_a() { let x = <heap a>; }
    let program = vec![
        Stmt::Function {
            name: core::Token::dummy("prom_a"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![Stmt::Let {
                    name: core::Token::dummy("x"),
                    ty: None,
                    initializer: Expr::Literal(ArtValue::HeapComposite(core::ast::ObjHandle(a))),
                }],
            }),
            method_owner: None,
        },
        Stmt::Function {
            name: core::Token::dummy("prom_b"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![Stmt::Let {
                    name: core::Token::dummy("y"),
                    ty: None,
                    initializer: Expr::Literal(ArtValue::HeapComposite(core::ast::ObjHandle(b))),
                }],
            }),
            method_owner: None,
        },
        // attach promote_b to a and promote_a to b
        Stmt::Expression(Expr::Call {
            callee: Box::new(Expr::Variable {
                name: core::Token::dummy("on_finalize"),
            }),
            arguments: vec![
                Expr::Literal(ArtValue::HeapComposite(core::ast::ObjHandle(a))),
                Expr::Variable {
                    name: core::Token::dummy("prom_b"),
                },
            ],
        }),
        Stmt::Expression(Expr::Call {
            callee: Box::new(Expr::Variable {
                name: core::Token::dummy("on_finalize"),
            }),
            arguments: vec![
                Expr::Literal(ArtValue::HeapComposite(core::ast::ObjHandle(b))),
                Expr::Variable {
                    name: core::Token::dummy("prom_a"),
                },
            ],
        }),
    ];
    assert!(
        interp.interpret(program).is_ok(),
        "interpret program in arena_finalizer_ordering.rs failed"
    );
    interp.debug_finalize_arena(aid);
    // At least one of the globals should exist or promotions counted
    let ga = interp.debug_get_global("x");
    let gb = interp.debug_get_global("y");
    let promoted_any = interp.get_finalizer_promotions() > 0
        || interp
            .finalizer_promotions_per_arena
            .get(&aid)
            .cloned()
            .unwrap_or(0)
            > 0;
    assert!(
        ga.is_some() || gb.is_some() || promoted_any,
        "mutual finalizers failed to promote any handle"
    );
    assert!(
        interp.debug_check_invariants(),
        "invariants failed after mutual finalizers test"
    );
}
