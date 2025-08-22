use core::ast::{ArtValue, Expr, Stmt};
use interpreter::interpreter::Interpreter;

#[test]
fn return_arena_escape_diagnostic() {
    let mut interp = Interpreter::with_prelude();
    // Registrar diretamente um objeto na arena e então tentar ligar (let) no escopo global.
    let aid = interp.debug_create_arena();
    let id = interp.debug_heap_register_in_arena(ArtValue::Array(vec![ArtValue::Int(1)]), aid);

    // Program: let b = <heap composite id>
    let program = vec![Stmt::Let {
        name: core::Token::dummy("b"),
        ty: None,
        initializer: Expr::Literal(ArtValue::HeapComposite(core::ast::ObjHandle(id))),
    }];

    assert!(interp.interpret(program).is_ok(), "interpret program in finalizer_decrement_paths.rs failed");
    let diags = interp.take_diagnostics();
    let found = diags
        .iter()
        .any(|d| d.message.contains("Attempt to bind arena object"));
    assert!(
        found,
        "esperado diagnóstico de escape ao bindar objeto de arena para escopo externo"
    );
}

#[test]
fn finalizer_allocs_temporary_promoted() {
    let mut interp = Interpreter::with_prelude();

    // Registrar owner e target
    let owner = interp.debug_heap_register(ArtValue::Array(vec![ArtValue::Int(2)]));
    interp.debug_define_global(
        "owner",
        ArtValue::HeapComposite(core::ast::ObjHandle(owner)),
    );
    let target = interp.debug_heap_register(ArtValue::Array(vec![]));
    interp.debug_define_global(
        "target",
        ArtValue::HeapComposite(core::ast::ObjHandle(target)),
    );
    interp.debug_heap_inc_weak(target);

    // Criar finalizer que faz: let temp = [42]; let kept = temp
    let program = vec![
        Stmt::Function {
            name: core::Token::dummy("fin_alloc"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![
                    Stmt::Let {
                        name: core::Token::dummy("temp"),
                        ty: None,
                        initializer: Expr::Array(vec![Expr::Literal(ArtValue::Int(42))]),
                    },
                    Stmt::Let {
                        name: core::Token::dummy("kept"),
                        ty: None,
                        initializer: Expr::Variable {
                            name: core::Token::dummy("temp"),
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
                    name: core::Token::dummy("fin_alloc"),
                },
            ],
        }),
    ];

    assert!(interp.interpret(program).is_ok(), "interpret program in finalizer_decrement_paths.rs failed");

    // remover strong do target e executar finalizer
    interp.debug_heap_remove(target);
    interp.debug_run_finalizer(target);

    // finalizer deve ter criado 'kept' e preservado o objeto temporário
    assert!(
        interp.debug_get_global("kept").is_some(),
        "finalizer não criou 'kept'"
    );
    // checagem direta via leitura da global 'kept' é suficiente
}
