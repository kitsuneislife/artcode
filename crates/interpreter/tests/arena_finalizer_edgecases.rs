use core::ast::{ArtValue, Expr, Stmt};
use interpreter::interpreter::Interpreter;

// Tests for arena finalization edge-cases: promotions, allocations inside finalizers,
// and weak/unowned invalidation.

#[test]
fn finalizer_promotes_handles_from_arena() {
    let mut interp = Interpreter::with_prelude();
    interp.enable_invariant_checks(true);
    // create arena and object
    let aid = interp.debug_create_arena();
    let id = interp.debug_heap_register_in_arena(ArtValue::Array(vec![ArtValue::Int(1)]), aid);
    // finalizer that creates a local var (promoted to root by finalizer semantics)
    let program = vec![
        Stmt::Function {
            name: core::Token::dummy("fp"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![Stmt::Let {
                    name: core::Token::dummy("promoted"),
                    ty: None,
                    initializer: Expr::Literal(ArtValue::Int(42)),
                }],
            }),
            method_owner: None,
        },
        Stmt::Expression(Expr::Call {
            callee: Box::new(Expr::Variable {
                name: core::Token::dummy("on_finalize"),
            }),
            arguments: vec![
                Expr::Literal(ArtValue::HeapComposite(core::ast::ObjHandle(id))),
                Expr::Variable {
                    name: core::Token::dummy("fp"),
                },
            ],
        }),
    ];
    assert!(
        interp.interpret(program).is_ok(),
        "interpret program in arena_finalizer_edgecases.rs failed"
    );
    // finalize arena: finalizer should run and promote local handles to root
    interp.debug_finalize_arena(aid);
    assert!(
        interp.get_finalizer_promotions() > 0 || interp.debug_get_global("promoted").is_some(),
        "finalizer promotion did not occur"
    );
    assert!(
        interp.debug_check_invariants(),
        "invariants failed after finalizer promotions"
    );
}

#[test]
fn finalizer_allocation_inside_arena_does_not_break_finalize() {
    let mut interp = Interpreter::with_prelude();
    interp.enable_invariant_checks(true);
    let aid = interp.debug_create_arena();
    let id = interp.debug_heap_register_in_arena(ArtValue::Array(vec![ArtValue::Int(2)]), aid);
    // finalizer that allocates a new array (which will be promoted to root by finalizer semantics)
    let program = vec![
        Stmt::Function {
            name: core::Token::dummy("allocf"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![Stmt::Let {
                    name: core::Token::dummy("tmp"),
                    ty: None,
                    initializer: Expr::Array(vec![Expr::Literal(ArtValue::Int(7))]),
                }],
            }),
            method_owner: None,
        },
        Stmt::Expression(Expr::Call {
            callee: Box::new(Expr::Variable {
                name: core::Token::dummy("on_finalize"),
            }),
            arguments: vec![
                Expr::Literal(ArtValue::HeapComposite(core::ast::ObjHandle(id))),
                Expr::Variable {
                    name: core::Token::dummy("allocf"),
                },
            ],
        }),
    ];
    assert!(
        interp.interpret(program).is_ok(),
        "interpret program in arena_finalizer_edgecases.rs failed"
    );
    // Should not panic or leave invalid invariants
    interp.debug_finalize_arena(aid);
    assert!(
        interp.debug_check_invariants(),
        "invariants failed after finalizer allocation in arena"
    );
}

#[test]
fn weak_and_unowned_inside_arena_invalidated_on_finalize() {
    let mut interp = Interpreter::with_prelude();
    let aid = interp.debug_create_arena();
    let id = interp.debug_heap_register_in_arena(ArtValue::Array(vec![ArtValue::Int(3)]), aid);
    // create weak and unowned wrappers
    let w =
        interp.debug_heap_register_in_arena(ArtValue::HeapComposite(core::ast::ObjHandle(id)), aid);
    // simulate weak builtin: use test helper to inc weak
    interp.debug_heap_inc_weak(w);
    // finalize arena
    interp.debug_finalize_arena(aid);
    // weak/unowned to arena objects should be dangling (weak_upgrade returns None)
    // Use debug_heap_upgrade_weak via the original id
    assert!(
        interp.debug_heap_upgrade_weak(id).is_none(),
        "weak upgrade unexpectedly succeeded after finalize"
    );
}
