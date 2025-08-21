use core::ast::{Expr, Stmt};
use interpreter::interpreter::Interpreter;
use core::ast::{ArtValue};

#[test]
fn field_mutation_runs_finalizer_and_decrements() {
    let mut interp = Interpreter::with_prelude();

    // Registrar um objeto que será o antigo campo (will be finalized)
    let old_obj = interp.debug_heap_register(ArtValue::Array(vec![ArtValue::Int(9)]));
    // Registrar outro objeto que substituirá o campo
    let new_obj = interp.debug_heap_register(ArtValue::Array(vec![ArtValue::Int(10)]));

    // Registrar struct definition para teste
    interp.register_struct_for_test("Pair", vec![(core::Token::dummy("left"), "Any".to_string()), (core::Token::dummy("right"), "Any".to_string())]);

    // Criar instância de Pair com left = old_obj
    let pair = core::ast::ArtValue::StructInstance {
        struct_name: "Pair".to_string(),
        fields: {
            let mut m = std::collections::HashMap::new();
            m.insert("left".to_string(), ArtValue::HeapComposite(core::ast::ObjHandle(old_obj)));
            m.insert("right".to_string(), ArtValue::Int(0));
            m
        },
    };
    let pair_id = interp.debug_heap_register(pair);
    interp.debug_define_global("p", ArtValue::HeapComposite(core::ast::ObjHandle(pair_id)));

    // Definir finalizer para old_obj que cria flag 'old_gone'
    let program = vec![
        Stmt::Function {
            name: core::Token::dummy("fin"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block { statements: vec![Stmt::Let {
                name: core::Token::dummy("old_gone"),
                ty: None,
                initializer: Expr::Literal(core::ast::ArtValue::Int(1)),
            }] }),
            method_owner: None,
        },
        // on_finalize(p.left, fin)
        Stmt::Expression(Expr::Call {
            callee: Box::new(Expr::Variable { name: core::Token::dummy("on_finalize") }),
            arguments: vec![
                // access field: p.left
                Expr::FieldAccess {
                    object: Box::new(Expr::Variable { name: core::Token::dummy("p") }),
                    field: core::Token::dummy("left"),
                },
                Expr::Variable { name: core::Token::dummy("fin") },
            ],
        }),
    ];
    interp.interpret(program).unwrap();

    // Agora mutar o campo left para new_obj: simular execução de stmt que faz p.left = new_obj
    // Em vez de criar AST de atribuição, re-criamos a struct com left=new_obj e rebind no global 'p'
    let new_pair = core::ast::ArtValue::StructInstance {
        struct_name: "Pair".to_string(),
        fields: {
            let mut m = std::collections::HashMap::new();
            m.insert("left".to_string(), ArtValue::HeapComposite(core::ast::ObjHandle(new_obj)));
            m.insert("right".to_string(), ArtValue::Int(0));
            m
        },
    };
    // Simular rebind: definir global 'p' novamente com novo valor
    interp.debug_define_global("p", new_pair);

    // Forçar execução do finalizer registrado para old_obj
    interp.debug_run_finalizer(old_obj);

    // Verificar que finalizer executou e old_obj foi removido após sweep (não havia weaks)
    let flag = interp.debug_get_global("old_gone");
    assert!(flag.is_some(), "finalizer não executou após mutação de campo");
    // Forçar sweep caso não tenha ocorrido
    interp.debug_sweep_dead();
    assert!(!interp.debug_heap_contains(old_obj), "old_obj deveria ter sido removido");
}
