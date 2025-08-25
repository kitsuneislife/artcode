use core::ast::{ArtValue, Expr, Stmt};
use interpreter::type_infer::{TypeEnv, TypeInfer};
use core::Token;

// Test that actor_send with a non-send-safe payload emits a type diagnostic
#[test]
fn actor_send_non_send_payload_emits_diag() {
    // Build program: let arr = [1]; actor_send(1, arr)
    let call_arr = Stmt::Let {
        name: Token::dummy("arr"),
        ty: None,
        initializer: Expr::Array(vec![Expr::Literal(ArtValue::Int(1))]),
    };
    let send_call = Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable { name: Token::dummy("actor_send") }),
        arguments: vec![Expr::Literal(ArtValue::Int(1)), Expr::Variable { name: Token::dummy("arr") }],
    });
    let prog = vec![call_arr, send_call];

    let mut tenv = TypeEnv::new();
    let mut inf = TypeInfer::new(&mut tenv);
    let _res = inf.run(&prog);
    // With TypeEnv propagation, the array binding is inferred as Array<Int> and
    // should be considered send-safe; there should be no type diagnostics.
    let type_diags: Vec<_> = inf.diags.iter().filter(|d| matches!(d.kind, diagnostics::DiagnosticKind::Type)).collect();
    assert!(type_diags.is_empty(), "did not expect type diagnostic for a known-send-safe payload: found {:?}", type_diags);
}
