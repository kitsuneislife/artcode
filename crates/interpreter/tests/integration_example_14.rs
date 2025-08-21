use interpreter::interpreter::Interpreter;
use core::ast::ArtValue;

#[test]
fn integration_example_14_finalizer_examples() {
    let mut interp = Interpreter::with_prelude();

    // Cenário 1: finalizer salva 'owner' em 'saved'
    let owner = interp.debug_heap_register(ArtValue::Array(vec![ArtValue::Int(1)]));
    interp.debug_define_global("owner", ArtValue::HeapComposite(core::ast::ObjHandle(owner)));
    let target = interp.debug_heap_register(ArtValue::Array(vec![]));
    interp.debug_define_global("target", ArtValue::HeapComposite(core::ast::ObjHandle(target)));
    // registrar finalizer via programa minimal
    interp.interpret(vec![
        core::ast::Stmt::Function {
            name: core::Token::dummy("fin_save_owner"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(core::ast::Stmt::Block { statements: vec![core::ast::Stmt::Let {
                name: core::Token::dummy("saved"),
                ty: None,
                initializer: core::ast::Expr::Variable { name: core::Token::dummy("owner") },
            }] }),
            method_owner: None,
        },
        core::ast::Stmt::Expression(core::ast::Expr::Call {
            callee: Box::new(core::ast::Expr::Variable { name: core::Token::dummy("on_finalize") }),
            arguments: vec![core::ast::Expr::Variable { name: core::Token::dummy("target") }, core::ast::Expr::Variable { name: core::Token::dummy("fin_save_owner") }],
        }),
    ]).unwrap();

    // remover strong do target e forçar execução
    interp.debug_heap_remove(target);
    interp.debug_run_finalizer(target);
    assert!(interp.debug_get_global("saved").is_some(), "finalizer não criou 'saved'");

    // Cenário 2: finalizer em arena promove 'outside' para 'promoted'
    let outside = interp.debug_heap_register(ArtValue::Int(999));
    interp.debug_define_global("outside", ArtValue::HeapComposite(core::ast::ObjHandle(outside)));
    let aid = interp.debug_create_arena();
    let a = interp.debug_heap_register_in_arena(ArtValue::Array(vec![]), aid);
    interp.debug_define_global("arena_obj", ArtValue::HeapComposite(core::ast::ObjHandle(a)));
    // registrar finalizer que define promoted = outside
    interp.interpret(vec![
        core::ast::Stmt::Function {
            name: core::Token::dummy("fin_promote"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(core::ast::Stmt::Block { statements: vec![core::ast::Stmt::Let {
                name: core::Token::dummy("promoted"),
                ty: None,
                initializer: core::ast::Expr::Variable { name: core::Token::dummy("outside") },
            }] }),
            method_owner: None,
        },
        core::ast::Stmt::Expression(core::ast::Expr::Call {
            callee: Box::new(core::ast::Expr::Variable { name: core::Token::dummy("on_finalize") }),
            arguments: vec![core::ast::Expr::Variable { name: core::Token::dummy("arena_obj") }, core::ast::Expr::Variable { name: core::Token::dummy("fin_promote") }],
        }),
    ]).unwrap();
    // finalizar arena explicitamente
    interp.debug_finalize_arena(aid);
    // promoted deve existir e outside preservado
    assert!(interp.debug_get_global("promoted").is_some(), "promoted não criado");
    assert!(interp.debug_heap_contains(outside), "outside deveria ser preservado");
}
