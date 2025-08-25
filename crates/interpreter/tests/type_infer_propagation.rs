use core::ast::{ArtValue, Expr, Stmt};
use interpreter::type_infer::{TypeEnv, TypeInfer};
use core::Token;

// let a = [1]; let b = a; actor_send(1, b) -> should be send-safe
#[test]
fn propagation_rebind_allows_send() {
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
    let _ = inf.run(&prog);
    let type_diags: Vec<_> = inf.diags.iter().filter(|d| matches!(d.kind, diagnostics::DiagnosticKind::Type)).collect();
    assert!(type_diags.is_empty(), "expected none, found: {:?}", type_diags);
}

// Shadowing: inner scope shadows and after exit outer type is restored
#[test]
fn shadowing_restores_outer_type() {
    let let_a = Stmt::Let {
        name: Token::dummy("a"),
        ty: None,
        initializer: Expr::Array(vec![Expr::Literal(ArtValue::Int(1))]),
    };
    let block = Stmt::Block { statements: vec![Stmt::Let {
        name: Token::dummy("a"),
        ty: None,
        initializer: Expr::Literal(ArtValue::Int(42)),
    }] };
    // after block, actor_send should accept original array in outer 'a'
    let send_call = Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable { name: Token::dummy("actor_send") }),
        arguments: vec![Expr::Literal(ArtValue::Int(1)), Expr::Variable { name: Token::dummy("a") }],
    });
    let prog = vec![let_a, block, send_call];

    let mut tenv = TypeEnv::new();
    let mut inf = TypeInfer::new(&mut tenv);
    let _ = inf.run(&prog);
    let type_diags: Vec<_> = inf.diags.iter().filter(|d| matches!(d.kind, diagnostics::DiagnosticKind::Type)).collect();
    assert!(type_diags.is_empty(), "expected none after shadow restore, found: {:?}", type_diags);
}

// Simple function param propagation: fn f(x) { actor_send(1, x) } ; f([1])
#[test]
fn function_param_propagation() {
    use std::rc::Rc;
    let fn_body = Stmt::Block { statements: vec![Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable { name: Token::dummy("actor_send") }),
        arguments: vec![Expr::Literal(ArtValue::Int(1)), Expr::Variable { name: Token::dummy("x") }],
    })] };
    let func = Stmt::Function {
        name: Token::dummy("f"),
        params: vec![core::ast::FunctionParam { name: Token::dummy("x"), ty: None }],
        return_type: None,
        body: Rc::new(fn_body),
        method_owner: None,
    };
    let call = Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable { name: Token::dummy("f") }),
        arguments: vec![Expr::Array(vec![Expr::Literal(ArtValue::Int(1))])],
    });
    let prog = vec![func, call];

    let mut tenv = TypeEnv::new();
    let mut inf = TypeInfer::new(&mut tenv);
    let _ = inf.run(&prog);
    let type_diags: Vec<_> = inf.diags.iter().filter(|d| matches!(d.kind, diagnostics::DiagnosticKind::Type)).collect();
    assert!(type_diags.is_empty(), "expected no type diags for function param propagation, found: {:?}", type_diags);
}

// Negative: unknown variable should still produce a diagnostic when used as payload
#[test]
fn unknown_var_still_warns() {
    let send_call = Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable { name: Token::dummy("actor_send") }),
        arguments: vec![Expr::Literal(ArtValue::Int(1)), Expr::Variable { name: Token::dummy("unknown") }],
    });
    let prog = vec![send_call];
    let mut tenv = TypeEnv::new();
    let mut inf = TypeInfer::new(&mut tenv);
    let _ = inf.run(&prog);
    let type_diags: Vec<_> = inf.diags.iter().filter(|d| matches!(d.kind, diagnostics::DiagnosticKind::Type)).collect();
    assert!(!type_diags.is_empty(), "expected diagnostic for unknown variable payload");
}
