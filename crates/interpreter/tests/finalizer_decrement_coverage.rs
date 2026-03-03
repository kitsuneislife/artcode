use core::ast::{ArtValue, Expr, Stmt};
use interpreter::interpreter::Interpreter;

// Coverage tests for decrement/finalizer paths
#[test]
fn rebind_triggers_finalizer_and_clears_handles() {
    let mut interp = Interpreter::with_prelude();
    // define a finalizer function that sets a global flag
    let program = vec![
        Stmt::Function {
            name: core::Token::dummy("fin"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![Stmt::Let {
                    name: core::Token::dummy("flag"),
                    ty: None,
                    initializer: Expr::Literal(ArtValue::Int(1)),
                }],
            }),
            method_owner: None,
        },
        // let a = [1]
        Stmt::Let {
            name: core::Token::dummy("a"),
            ty: None,
            initializer: Expr::Array(vec![Expr::Literal(ArtValue::Int(1))]),
        },
        // on_finalize(a, fin)
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
        // rebind a
        Stmt::Let {
            name: core::Token::dummy("a"),
            ty: None,
            initializer: Expr::Array(vec![]),
        },
    ];
    interp.enable_invariant_checks(true);
    assert!(
        interp.interpret(program).is_ok(),
        "interpret program in finalizer_decrement_coverage.rs failed"
    );
    // finalizer should have run and set flag
    assert!(
        interp.debug_get_global("flag").is_some(),
        "finalizer did not set flag on rebind"
    );
    assert!(
        interp.debug_check_invariants(),
        "invariants failed after rebind finalizer"
    );
}

#[test]
fn return_of_arena_object_is_reported() {
    let mut interp = Interpreter::with_prelude();
    // create arena and object, then create a function that returns the arena object
    let aid = interp.debug_create_arena();
    let id = interp.debug_heap_register_in_arena(ArtValue::Array(vec![ArtValue::Int(9)]), aid);
    let program = vec![
        Stmt::Function {
            name: core::Token::dummy("ret_a"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![Stmt::Return {
                    value: Some(Expr::Literal(ArtValue::HeapComposite(
                        core::ast::ObjHandle(id),
                    ))),
                }],
            }),
            method_owner: None,
        },
        Stmt::Let {
            name: core::Token::dummy("g"),
            ty: None,
            initializer: Expr::Call {
                callee: Box::new(Expr::Variable {
                    name: core::Token::dummy("ret_a"),
                }),
                arguments: vec![],
            },
        },
    ];
    interp.enable_invariant_checks(true);
    assert!(
        interp.interpret(program).is_ok(),
        "interpret program in finalizer_decrement_coverage.rs failed"
    );
    let diags = interp.take_diagnostics();
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("Attempt to return arena object")),
        "return of arena object was not reported"
    );
}

#[test]
fn field_mutation_runs_finalizer_on_previous_value() {
    let mut interp = Interpreter::with_prelude();
    interp.register_struct_for_test(
        "S",
        vec![(core::Token::dummy("child"), "Array".to_string())],
    );
    // create x and s globals
    let id = interp.debug_heap_register(ArtValue::Array(vec![ArtValue::Int(1)]));
    interp.debug_define_global("x", ArtValue::HeapComposite(core::ast::ObjHandle(id)));
    let mut fields = std::collections::HashMap::new();
    fields.insert("child".to_string(), ArtValue::Array(vec![]));
    let s_val = ArtValue::StructInstance {
        struct_name: "S".to_string(),
        fields,
    };
    let sid = interp.debug_heap_register(s_val.clone());
    interp.debug_define_global("s", ArtValue::HeapComposite(core::ast::ObjHandle(sid)));
    // finalizer
    let program = vec![
        Stmt::Function {
            name: core::Token::dummy("finf"),
            params: vec![],
            return_type: None,
            body: std::rc::Rc::new(Stmt::Block {
                statements: vec![Stmt::Let {
                    name: core::Token::dummy("ff"),
                    ty: None,
                    initializer: Expr::Literal(ArtValue::Int(7)),
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
                    name: core::Token::dummy("x"),
                },
                Expr::Variable {
                    name: core::Token::dummy("finf"),
                },
            ],
        }),
    ];
    assert!(
        interp.interpret(program).is_ok(),
        "interpret program in finalizer_decrement_coverage.rs failed"
    );
    // simulate field assignment: set s.child = x and then drop x
    if let Some(ArtValue::HeapComposite(hx)) = interp.debug_get_global("x") {
        let mut new_fields = std::collections::HashMap::new();
        new_fields.insert("child".to_string(), ArtValue::HeapComposite(hx));
        let new_s = ArtValue::StructInstance {
            struct_name: "S".to_string(),
            fields: new_fields,
        };
        let new_sid = interp.debug_heap_register(new_s);
        interp.debug_define_global("s", ArtValue::HeapComposite(core::ast::ObjHandle(new_sid)));
        interp.debug_define_global("x", ArtValue::none());
        // run finalizers
        interp.debug_run_finalizer(id);
        assert!(
            interp.debug_get_global("ff").is_some(),
            "field finalizer did not run"
        );
        if !interp.debug_check_invariants() {
            let v = interp.debug_invariant_violations();
            assert!(
                false,
                "invariants failed after field mutation finalizer: {:?}",
                v
            );
        }
    } else {
        assert!(false, "x missing in test setup");
    }
}

#[test]
fn performant_arena_finalization_promotes_and_cleans() {
    let mut interp = Interpreter::with_prelude();
    interp.enable_invariant_checks(true);
    // build a performant block programmatically: create an arena, allocate a heap composite there and ensure finalize_arena cleans it
    let aid = interp.debug_create_arena();
    let id = interp.debug_heap_register_in_arena(ArtValue::Array(vec![ArtValue::Int(5)]), aid);
    // register a finalizer that creates a global handle (promotion)
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
        "interpret program in finalizer_decrement_coverage.rs failed"
    );
    // finalize arena
    interp.debug_finalize_arena(aid);
    // after finalization, either object removed or promoted; invariants should hold
    assert!(
        interp.debug_check_invariants(),
        "invariants failed after finalize_arena"
    );
}
