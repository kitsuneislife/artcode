use ir::lower_stmt;
use core::ast::{Stmt, Expr, FunctionParam};
use core::Token;

#[test]
fn golden_lower_intrinsic_call() {
    // func alloc_one() -> i64 { return gc_alloc(16) }
    let name = Token::dummy("alloc_one");
    let params = vec![];

    let call_expr = Expr::Call { callee: Box::new(Expr::Variable { name: Token::dummy("gc_alloc") }), arguments: vec![Expr::Literal(core::ast::ArtValue::Int(16))] };
    let func = Stmt::Function { name, params, return_type: Some("i64".to_string()), body: std::rc::Rc::new(Stmt::Return { value: Some(call_expr) }), method_owner: None };

    let irf = lower_stmt(&func).expect("lowering failed");
    let text = irf.emit_text();

    // Expect lowering to produce a call to gc_alloc with literal
    assert!(text.contains("call gc_alloc(16)"));
}
