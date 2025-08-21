use core::ast::{ArtValue, Expr, Stmt};
use interpreter::interpreter::Interpreter;

// Teste A: Finalizer pode alocar múltiplos temporários e promovê-los para root
#[test]
fn finalizer_promotes_multiple_temporaries_to_root() {
    let mut interp = Interpreter::with_prelude();
    interp.enable_invariant_checks(true);
    let aid = interp.debug_create_arena();
    let id = interp.debug_heap_register_in_arena(ArtValue::Array(vec![]), aid);

    // Função finalizer que cria dois objetos e os define como globals (kept1 e kept2)
    let program = vec![
        Stmt::Function {
            name: core::Token::dummy("fin_multi"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![
                    Stmt::Let {
                        name: core::Token::dummy("kept1"),
                        ty: None,
                        initializer: Expr::Array(vec![Expr::Literal(ArtValue::Int(11))]),
                    },
                    Stmt::Let {
                        name: core::Token::dummy("kept2"),
                        ty: None,
                        initializer: Expr::Array(vec![Expr::Literal(ArtValue::Int(22))]),
                    },
                ],
            }),
            method_owner: None,
        },
        // Registrar finalizer
        Stmt::Expression(Expr::Call {
            callee: Box::new(Expr::Variable { name: core::Token::dummy("on_finalize") }),
            arguments: vec![Expr::Literal(ArtValue::HeapComposite(core::ast::ObjHandle(id))), Expr::Variable { name: core::Token::dummy("fin_multi") }],
        }),
    ];

    interp.interpret(program).unwrap();
    // Simular remoção do último strong e rodar finalizer
    interp.debug_heap_remove(id);
    interp.debug_run_finalizer(id);

    assert!(interp.debug_get_global("kept1").is_some(), "kept1 não promovido pelo finalizer");
    assert!(interp.debug_get_global("kept2").is_some(), "kept2 não promovido pelo finalizer");
}

// Teste B: return aninhado dentro de performant deve produzir diagnóstico
#[test]
fn nested_performant_return_emits_diagnostic() {
    let mut interp = Interpreter::with_prelude();
    let aid = interp.debug_create_arena();
    let id = interp.debug_heap_register_in_arena(ArtValue::Array(vec![ArtValue::Int(99)]), aid);

    // Função que contém um bloco performant que tenta retornar o objeto de arena
    let program = vec![
        Stmt::Function {
            name: core::Token::dummy("outer"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![Stmt::Performant {
                    statements: vec![
                        Stmt::Let {
                            name: core::Token::dummy("a"),
                            ty: None,
                            initializer: Expr::Literal(ArtValue::HeapComposite(core::ast::ObjHandle(id))),
                        },
                        Stmt::Return { value: Some(Expr::Variable { name: core::Token::dummy("a") }) },
                    ],
                }],
            }),
            method_owner: None,
        },
        // Chamar e atribuir
        Stmt::Let {
            name: core::Token::dummy("r"),
            ty: None,
            initializer: Expr::Call {
                callee: Box::new(Expr::Variable { name: core::Token::dummy("outer") }),
                arguments: vec![],
            },
        },
    ];

    interp.interpret(program).unwrap();
    let diags = interp.take_diagnostics();
    assert!(diags.iter().any(|d| d.message.contains("Attempt to return arena object")), "esperado diagnóstico ao retornar objeto de arena de bloco performant");
}

// Teste C: atribuição de campo onde o campo anterior era um objeto de arena dispara finalizer
#[test]
fn field_assign_prev_arena_triggers_finalizer() {
    let mut interp = Interpreter::with_prelude();
    interp.register_struct_for_test("S2", vec![(core::Token::dummy("child"), "Array".to_string())]);

    // Criar objeto child_arena em arena
    let aid = interp.debug_create_arena();
    let child_arena = interp.debug_heap_register_in_arena(ArtValue::Array(vec![ArtValue::Int(7)]), aid);
    interp.debug_define_global("child_arena", ArtValue::HeapComposite(core::ast::ObjHandle(child_arena)));

    // Criar outro objeto child_plain
    let child_plain = interp.debug_heap_register(ArtValue::Array(vec![ArtValue::Int(8)]));
    interp.debug_define_global("child_plain", ArtValue::HeapComposite(core::ast::ObjHandle(child_plain)));

    // Criar s with child = child_arena
    let mut fields = std::collections::HashMap::new();
    fields.insert("child".to_string(), ArtValue::HeapComposite(core::ast::ObjHandle(child_arena)));
    let s_id = interp.debug_heap_register(ArtValue::StructInstance { struct_name: "S2".to_string(), fields });
    interp.debug_define_global("s", ArtValue::HeapComposite(core::ast::ObjHandle(s_id)));

    // Registrar finalizer que cria flag
    let program = vec![
        Stmt::Function {
            name: core::Token::dummy("fin_prev"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block { statements: vec![Stmt::Let {
                name: core::Token::dummy("prev_flag"),
                ty: None,
                initializer: Expr::Literal(ArtValue::Int(55)),
            }] }),
            method_owner: None,
        },
        // on_finalize(child_arena, fin_prev)
        Stmt::Expression(Expr::Call {
            callee: Box::new(Expr::Variable { name: core::Token::dummy("on_finalize") }),
            arguments: vec![Expr::Variable { name: core::Token::dummy("child_arena") }, Expr::Variable { name: core::Token::dummy("fin_prev") }],
        }),
    ];
    interp.interpret(program).unwrap();

    // Assign s.child = child_plain by creating new struct and replacing global
    let mut new_fields = std::collections::HashMap::new();
    new_fields.insert("child".to_string(), ArtValue::HeapComposite(core::ast::ObjHandle(child_plain)));
    let new_s = interp.debug_heap_register(ArtValue::StructInstance { struct_name: "S2".to_string(), fields: new_fields });
    interp.debug_define_global("s", ArtValue::HeapComposite(core::ast::ObjHandle(new_s)));

    // Remove reference to child_arena and run finalizer
    interp.debug_define_global("child_arena", ArtValue::none());
    interp.debug_run_finalizer(child_arena);

    assert!(interp.debug_get_global("prev_flag").is_some(), "finalizer do campo anterior não executou");
}
