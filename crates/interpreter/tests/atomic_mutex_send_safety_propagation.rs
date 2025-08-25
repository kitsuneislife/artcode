use core::ast::{ArtValue, Expr, Stmt};
use interpreter::type_infer::{TypeEnv, TypeInfer};
use core::Token;

// Test simple variable-to-variable propagation: let a = [1]; let b = a; actor_send(1, b)
// With simple propagation via TypeEnv, `b` should have a known Array<Int> type and be send-safe.
#[test]
fn actor_send_propagation_allows_forwarded_array() {
    let let_a = Stmt::Let {
        name: Token::dummy("a"),
        ty: None,
        initializer: Expr::Array(vec![Expr::Literal(ArtValue::Int(1))]),
    };
    let let_b = Stmt::Let {
        name: Token::dummy("b"),
        ty: None,
        initializer: Expr::Variable { name: Token::dummy("a") },
    };
    let send_call = Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable { name: Token::dummy("actor_send") }),
        arguments: vec![Expr::Literal(ArtValue::Int(1)), Expr::Variable { name: Token::dummy("b") }],
    });
    let prog = vec![let_a, let_b, send_call];

    let mut tenv = TypeEnv::new();
    let mut inf = TypeInfer::new(&mut tenv);
    let _res = inf.run(&prog);

    let type_diags: Vec<_> = inf.diags.iter().filter(|d| matches!(d.kind, diagnostics::DiagnosticKind::Type)).collect();
    assert!(type_diags.is_empty(), "expected no type diagnostic for forwarded send-safe array, found: {:?}", type_diags);
}
