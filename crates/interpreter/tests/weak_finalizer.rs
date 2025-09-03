use core::ast::ArtValue;
use core::ast::{Expr, Stmt};
use interpreter::interpreter::Interpreter;

#[test]
fn finalizer_runs_and_object_stays_while_weak() {
    let mut interp = Interpreter::with_prelude();
    interp.enable_invariant_checks(true);
    // registrar objeto via helper e expor como global 'a'
    let id = interp.debug_heap_register(ArtValue::Array(vec![ArtValue::Int(1)]));
    interp.debug_define_global("a", ArtValue::HeapComposite(core::ast::ObjHandle(id)));
    // incrementar weak para simular existência de weak antes do drop
    interp.debug_heap_inc_weak(id);

    // Criar função finalizer 'fin' que define 'created' global e registrar on_finalize(a, fin)
    let program = vec![
        Stmt::Function {
            name: core::Token::dummy("fin"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![Stmt::Let {
                    name: core::Token::dummy("created"),
                    ty: None,
                    initializer: Expr::Array(vec![Expr::Literal(ArtValue::Int(2))]),
                }],
            }),
            method_owner: None,
        },
        Stmt::Expression(Expr::Call {
            callee: Box::new(Expr::Variable {
                name: core::Token::dummy("on_finalize"),
            }),
            arguments: vec![
                Expr::Variable {
                    name: core::Token::dummy("a"),
                },
                Expr::Variable {
                    name: core::Token::dummy("fin"),
                },
            ],
        }),
    ];
    assert!(
        interp.interpret(program).is_ok(),
        "interpret program in weak_finalizer.rs failed"
    );

    // simular remoção do strong
    interp.debug_heap_remove(id);

    // enquanto houver weak, o objeto não deve ser removido
    assert!(
        interp.debug_heap_contains(id),
        "objeto removido mesmo com weak>0"
    );

    // finalizer deveria ter sido executado e criado a variável 'created' no root
    // forçar execução do finalizer e limpeza para garantir que qualquer ação definida seja aplicada
    interp.debug_run_finalizer(id);
    let created = interp.debug_get_global("created");
    assert!(created.is_some(), "finalizer não criou 'created' global'");

    // agora decrementar o weak e forçar sweep
    interp.debug_heap_dec_weak(id);
    interp.debug_sweep_dead();
    assert!(
        !interp.debug_heap_contains(id),
        "objeto deveria ser removido apos weak==0"
    );
}

#[test]
fn multiple_weaks_only_remove_when_all_gone() {
    let mut interp = Interpreter::with_prelude();
    interp.enable_invariant_checks(true);
    // registrar objeto manualmente e criar 3 weaks
    let id = interp.debug_heap_register(ArtValue::Int(999));
    interp.debug_heap_inc_weak(id);
    interp.debug_heap_inc_weak(id);
    interp.debug_heap_inc_weak(id);
    // remover strong
    interp.debug_heap_remove(id);
    // objeto deve continuar existindo
    assert!(
        interp.debug_heap_contains(id),
        "objeto removido mesmo com weaks > 0"
    );
    // decrementar weaks um a um
    interp.debug_heap_dec_weak(id);
    assert!(
        interp.debug_heap_contains(id),
        "removido antes de todos weaks decrementados (1)"
    );
    interp.debug_heap_dec_weak(id);
    assert!(
        interp.debug_heap_contains(id),
        "removido antes de todos weaks decrementados (2)"
    );
    interp.debug_heap_dec_weak(id);
    // forçar sweep
    interp.debug_sweep_dead();
    assert!(
        !interp.debug_heap_contains(id),
        "não removido após todos weaks decrementados"
    );
}

#[test]
fn finalizer_creates_handle_preserved() {
    let mut interp = Interpreter::with_prelude();
    interp.enable_invariant_checks(true);
    // registrar um objeto que o finalizer irá salvar (owner)
    let owner_id = interp.debug_heap_register(ArtValue::Array(vec![ArtValue::Int(7)]));
    interp.debug_define_global(
        "owner",
        ArtValue::HeapComposite(core::ast::ObjHandle(owner_id)),
    );

    // registrar objeto alvo que receberá o finalizer
    let target_id = interp.debug_heap_register(ArtValue::Array(vec![]));
    interp.debug_define_global(
        "target",
        ArtValue::HeapComposite(core::ast::ObjHandle(target_id)),
    );

    // incrementar weak no alvo para simular weak existente
    interp.debug_heap_inc_weak(target_id);

    // Criar finalizer que faz: let saved = owner
    let program = vec![
        Stmt::Function {
            name: core::Token::dummy("fin"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![Stmt::Let {
                    name: core::Token::dummy("saved"),
                    ty: None,
                    initializer: Expr::Variable {
                        name: core::Token::dummy("owner"),
                    },
                }],
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
    assert!(
        interp.interpret(program).is_ok(),
        "interpret program in weak_finalizer.rs failed"
    );

    // remover strong do alvo e forçar execução do finalizer
    interp.debug_heap_remove(target_id);
    interp.debug_run_finalizer(target_id);

    // o finalizer deve ter criado a global 'saved' e preservado o objeto 'owner'
    let saved = interp.debug_get_global("saved");
    assert!(saved.is_some(), "finalizer não criou 'saved'");
    assert!(
        interp.debug_heap_contains(owner_id),
        "owner deveria ser preservado pelo handle criado no finalizer"
    );
}

#[test]
fn finalizer_promotes_handles_across_arenas() {
    let mut interp = Interpreter::with_prelude();
    interp.enable_invariant_checks(true);
    // criar objeto fora de arena que será referenciado pelo finalizer
    let outside_id = interp.debug_heap_register(ArtValue::Int(42));
    interp.debug_define_global(
        "outside",
        ArtValue::HeapComposite(core::ast::ObjHandle(outside_id)),
    );

    // criar uma arena id (valor sintético para inserir objetos nela)
    let aid = interp.debug_create_arena();

    // registrar objeto dentro da arena
    let arena_obj = interp.debug_heap_register_in_arena(ArtValue::Array(vec![]), aid);
    interp.debug_define_global(
        "arena_obj",
        ArtValue::HeapComposite(core::ast::ObjHandle(arena_obj)),
    );

    // criar e registrar finalizer que salva a referência a `outside` em 'promoted'
    let program = vec![
        Stmt::Function {
            name: core::Token::dummy("fin"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![Stmt::Let {
                    name: core::Token::dummy("promoted"),
                    ty: None,
                    initializer: Expr::Variable {
                        name: core::Token::dummy("outside"),
                    },
                }],
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
    assert!(
        interp.interpret(program).is_ok(),
        "interpret program in weak_finalizer.rs failed"
    );

    // finalizar explicitamente a arena
    interp.debug_finalize_arena(aid);

    // Após finalização, o objeto da arena deve ter sido removido
    assert!(
        !interp.debug_heap_contains(arena_obj),
        "objeto da arena não foi removido"
    );

    // O finalizer deve ter promovido a referência ao objeto externo e criado global 'promoted'
    let promoted = interp.debug_get_global("promoted");
    assert!(
        promoted.is_some(),
        "finalizer não promoveu/registrou 'promoted'"
    );
    // e o objeto externo deve continuar presente
    assert!(
        interp.debug_heap_contains(outside_id),
        "objeto externo deveria ser preservado"
    );
    // Métrica: alguma promoção deve ter sido contabilizada
    assert!(
        interp.get_finalizer_promotions() > 0,
        "expected finalizer_promotions > 0 after cross-arena finalizer"
    );
}
