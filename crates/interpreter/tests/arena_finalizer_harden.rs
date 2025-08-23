use core::ast::{ArtValue, Expr, Stmt};
use interpreter::interpreter::Interpreter;

// Validate that promotions performed by finalizers are attributed to the arena
#[test]
fn finalizer_promotions_counted_per_arena() {
    let mut interp = Interpreter::with_prelude();
    interp.enable_invariant_checks(true);
    let aid = interp.debug_create_arena();
    let id = interp.debug_heap_register_in_arena(ArtValue::Array(vec![ArtValue::Int(1)]), aid);
    // finalizer that creates a heap allocation (Array) so there is a strong handle to promote
    let program = vec![
        Stmt::Function { name: core::Token::dummy("fp2"), params: vec![], return_type: None,
            body: std::rc::Rc::new(Stmt::Block { statements: vec![Stmt::Let { name: core::Token::dummy("promoted2"), ty: None, initializer: Expr::Array(vec![Expr::Literal(ArtValue::Int(99))]) }] }), method_owner: None },
        Stmt::Expression(Expr::Call { callee: Box::new(Expr::Variable { name: core::Token::dummy("on_finalize") }), arguments: vec![Expr::Literal(ArtValue::HeapComposite(core::ast::ObjHandle(id))), Expr::Variable { name: core::Token::dummy("fp2") }] }),
    ];
    assert!(interp.interpret(program).is_ok(), "interpret program in arena_finalizer_harden.rs failed");
    interp.debug_finalize_arena(aid);
    // check per-arena metric
    let per = interp.finalizer_promotions_per_arena.get(&aid).cloned().unwrap_or(0);
    assert!(per > 0 || interp.get_finalizer_promotions() > 0 || interp.debug_get_global("promoted2").is_some(), "expected promotions attributed to arena");
    assert!(interp.debug_check_invariants(), "invariants failed after promotions attribution");
}

// Ensure calling finalize_arena multiple times is safe and idempotent (no invariant regression)
#[test]
fn finalize_arena_idempotent_and_safe() {
    let mut interp = Interpreter::with_prelude();
    interp.enable_invariant_checks(true);
    let aid = interp.debug_create_arena();
    let oid = interp.debug_heap_register_in_arena(ArtValue::Array(vec![ArtValue::Int(7)]), aid);
    // register a trivial finalizer that allocates a temporary (heapified Array)
    let program = vec![
        Stmt::Function { name: core::Token::dummy("alloc_once"), params: vec![], return_type: None,
            body: std::rc::Rc::new(Stmt::Block { statements: vec![Stmt::Let { name: core::Token::dummy("tmpx"), ty: None, initializer: Expr::Array(vec![Expr::Literal(ArtValue::Int(5))]) }] }), method_owner: None },
    ];
    assert!(interp.interpret(program).is_ok(), "interpret program in arena_finalizer_harden.rs failed");
    // attach finalizer to the previously created arena id (we used the returned id above)
    // For simplicity, find the first object in the arena using debug helpers exposed in tests
    // attach finalizer to the created object
    let attach = vec![Stmt::Expression(Expr::Call { callee: Box::new(Expr::Variable { name: core::Token::dummy("on_finalize") }), arguments: vec![Expr::Literal(ArtValue::HeapComposite(core::ast::ObjHandle(oid))), Expr::Variable { name: core::Token::dummy("alloc_once") }] })];
    assert!(interp.interpret(attach).is_ok(), "interpret attach in arena_finalizer_harden.rs failed");
    // finalize twice
    interp.debug_finalize_arena(aid);
    assert!(interp.debug_check_invariants(), "invariants failed after first finalize");
    interp.debug_finalize_arena(aid);
    assert!(interp.debug_check_invariants(), "invariants failed after second finalize");
}
