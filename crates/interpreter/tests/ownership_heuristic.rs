use core::ast::{Expr, Stmt};
use interpreter::interpreter::Interpreter;

#[test]
fn candidate_owner_edges_detects_parent_field() {
    let mut interp = Interpreter::with_prelude();
    // Definir structs Parent {}, Child { parent: Parent }
    interp.register_struct_for_test("Parent", vec![]);
    interp.register_struct_for_test(
        "Child",
        vec![(core::Token::dummy("parent"), "parent".to_string())],
    );

    // Construir AST:
    // let p = Parent{};
    // let c = Child{ parent: p };
    // (manter c vivo globalmente)
    let program = vec![
        Stmt::Let {
            name: core::Token::dummy("p"),
            ty: None,
            initializer: Expr::StructInit {
                name: core::Token::dummy("Parent"),
                fields: vec![],
            },
        },
        Stmt::Let {
            name: core::Token::dummy("c"),
            ty: None,
            initializer: Expr::StructInit {
                name: core::Token::dummy("Child"),
                fields: vec![(
                    core::Token::dummy("parent"),
                    Expr::Variable {
                        name: core::Token::dummy("p"),
                    },
                )],
            },
        },
    ];
    assert!(
        interp.interpret(program).is_ok(),
        "interpret program in ownership_heuristic.rs failed"
    );
    let report = interp.cycle_report();
    assert!(
        !report.candidate_owner_edges.is_empty(),
        "esperava pelo menos uma candidate_owner_edge"
    );
    // Verifica que algum edge aponta para objeto parent associado
    // Apenas checamos quantidade >0 porque ids variam.
}
