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
    let res = inf.run(&prog);
    // Expect TypeInfer to have produced a type diagnostic about actor_send payload
    let type_diags: Vec<_> = inf.diags.iter().filter(|d| matches!(d.kind, diagnostics::DiagnosticKind::Type)).collect();
    assert!(!type_diags.is_empty(), "expected type diagnostic for non-send-safe payload");
}
