use ir::lower_stmt;
use core::ast::{Stmt, Expr, FunctionParam};
use core::Token;

// Test lowering for calls with literal arguments
// func callee(x) -> i64 { return x }
// func caller() -> i64 { return callee(42) }
#[test]
fn golden_lower_call_literals() {
    let callee_name = Token::dummy("callee");
    let param_name = Token::dummy("x");
    let callee_params = vec![FunctionParam { name: param_name.clone(), ty: None }];
    let callee_body = Stmt::Return { value: Some(Expr::Variable { name: param_name.clone() }) };
    let callee = Stmt::Function { name: callee_name.clone(), params: callee_params, return_type: Some("i64".to_string()), body: std::rc::Rc::new(Stmt::Block { statements: vec![callee_body] }), method_owner: None };

    let caller_name = Token::dummy("caller");
    let caller_params = vec![];
    let call_expr = Expr::Call { callee: Box::new(Expr::Variable { name: callee_name.clone() }), arguments: vec![Expr::Literal(core::ast::ArtValue::Int(42))] };
    let caller_body = Stmt::Return { value: Some(call_expr) };
    let caller = Stmt::Function { name: caller_name, params: caller_params, return_type: Some("i64".to_string()), body: std::rc::Rc::new(Stmt::Block { statements: vec![caller_body] }), method_owner: None };

    // Lower caller function; ensure call instruction or literal materialization appears
    let irf = lower_stmt(&caller).expect("lowering caller failed");
    let text = irf.emit_text();
    assert!(text.contains("call") || text.contains("ConstI64"), "expected call or ConstI64 in caller lowering, got: {}", text);
}
