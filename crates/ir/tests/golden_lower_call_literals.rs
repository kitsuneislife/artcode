use ir::lower_stmt;
use core::ast::{Stmt, Expr, FunctionParam};
use core::Token;

#[test]
fn golden_lower_call_literals() {
    // func caller(a) -> i64 { return add(1, a) }
    let name = Token::dummy("caller_lit");
    let a = Token::dummy("a");
    let params = vec![FunctionParam { name: a.clone(), ty: None }];

    let call_expr = Expr::Call { callee: Box::new(Expr::Variable { name: Token::dummy("add") }), arguments: vec![Expr::Literal(core::ast::ArtValue::Int(1)), Expr::Variable { name: a }] };
    let func = Stmt::Function { name, params, return_type: Some("i64".to_string()), body: std::rc::Rc::new(Stmt::Return { value: Some(call_expr) }), method_owner: None };

    let irf = lower_stmt(&func).expect("lowering failed");
    let text = irf.emit_text();

    // Expect literal 1 to be used as argument textually
    assert!(text.contains("call add(1, a)"));
}
