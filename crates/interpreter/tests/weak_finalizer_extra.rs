use core::ast::ArtValue;
use core::ast::{Expr, Stmt};
use interpreter::interpreter::Interpreter;

#[test]
fn finalizer_creates_multiple_handles() {
    let mut interp = Interpreter::with_prelude();

    // Criar dois owners separados
    let owner1 = interp.debug_heap_register(ArtValue::Array(vec![ArtValue::Int(1)]));
    interp.debug_define_global(
        "owner1",
        ArtValue::HeapComposite(core::ast::ObjHandle(owner1)),
    );
    let owner2 = interp.debug_heap_register(ArtValue::Array(vec![ArtValue::Int(2)]));
    interp.debug_define_global(
        "owner2",
        ArtValue::HeapComposite(core::ast::ObjHandle(owner2)),
    );

    // Criar target que terá finalizer que salva ambos owners
    let target = interp.debug_heap_register(ArtValue::Array(vec![]));
    interp.debug_define_global(
        "target",
        ArtValue::HeapComposite(core::ast::ObjHandle(target)),
    );
    interp.debug_heap_inc_weak(target);

    // Program: define finalizer that assigns saved1 = owner1; saved2 = owner2
    let program = vec![
        Stmt::Function {
            name: core::Token::dummy("fin"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![
                    Stmt::Let {
                        name: core::Token::dummy("saved1"),
                        ty: None,
                        initializer: Expr::Variable {
                            name: core::Token::dummy("owner1"),
                        },
                    },
                    Stmt::Let {
                        name: core::Token::dummy("saved2"),
                        ty: None,
                        initializer: Expr::Variable {
                            name: core::Token::dummy("owner2"),
                        },
                    },
                ],
            }),
            method_owner: None,
        },
        Stmt::Expression(Expr::Call {
            callee: Box::new(Expr::Variable {
                name: core::Token::dummy("on_finalize"),
            }),
            arguments: vec![
                Expr::Variable {
                    name: core::Token::dummy("target"),
                },
                Expr::Variable {
                    name: core::Token::dummy("fin"),
                },
            ],
        }),
    ];
    assert!(interp.interpret(program).is_ok(), "interpret program in weak_finalizer_extra.rs failed");

    // Remove strong do target e executa finalizer
    interp.debug_heap_remove(target);
    interp.debug_run_finalizer(target);

    // Saved1 e saved2 devem existir e owners preservados
    assert!(
        interp.debug_get_global("saved1").is_some(),
        "saved1 não criado"
    );
    assert!(
        interp.debug_get_global("saved2").is_some(),
        "saved2 não criado"
    );
    assert!(
        interp.debug_heap_contains(owner1),
        "owner1 deveria ser preservado"
    );
    assert!(
        interp.debug_heap_contains(owner2),
        "owner2 deveria ser preservado"
    );
}

#[test]
fn finalizer_promotes_multiple_to_root_from_arena() {
    let mut interp = Interpreter::with_prelude();

    // objeto fora da arena
    let outside = interp.debug_heap_register(ArtValue::Int(123));
    interp.debug_define_global(
        "outside",
        ArtValue::HeapComposite(core::ast::ObjHandle(outside)),
    );

    // criar arena e objeto dentro
    let aid = interp.debug_create_arena();
    let arena_obj = interp.debug_heap_register_in_arena(ArtValue::Array(vec![]), aid);
    interp.debug_define_global(
        "arena_obj",
        ArtValue::HeapComposite(core::ast::ObjHandle(arena_obj)),
    );

    // registrar finalizer que cria duas variáveis apontando para `outside`
    let program = vec![
        Stmt::Function {
            name: core::Token::dummy("fin"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![
                    Stmt::Let {
                        name: core::Token::dummy("p1"),
                        ty: None,
                        initializer: Expr::Variable {
                            name: core::Token::dummy("outside"),
                        },
                    },
                    Stmt::Let {
                        name: core::Token::dummy("p2"),
                        ty: None,
                        initializer: Expr::Variable {
                            name: core::Token::dummy("outside"),
                        },
                    },
                ],
            }),
            method_owner: None,
        },
        Stmt::Expression(Expr::Call {
            callee: Box::new(Expr::Variable {
                name: core::Token::dummy("on_finalize"),
            }),
            arguments: vec![
                Expr::Variable {
                    name: core::Token::dummy("arena_obj"),
                },
                Expr::Variable {
                    name: core::Token::dummy("fin"),
                },
            ],
        }),
    ];
    assert!(interp.interpret(program).is_ok(), "interpret program in weak_finalizer_extra.rs failed");

    // finalizar arena explicitamente
    interp.debug_finalize_arena(aid);

    // arena obj removido
    assert!(
        !interp.debug_heap_contains(arena_obj),
        "objeto de arena ainda presente"
    );

    // finalizer deve ter promovido referências e criado p1/p2
    assert!(interp.debug_get_global("p1").is_some(), "p1 não criado");
    assert!(interp.debug_get_global("p2").is_some(), "p2 não criado");
    assert!(
        interp.debug_heap_contains(outside),
        "objeto externo deveria ser preservado"
    );
}
