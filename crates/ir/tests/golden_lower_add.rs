use ir::lower_stmt;
use core::ast::{Stmt, Expr, FunctionParam};
use core::Token;

#[test]
fn golden_lower_add() {
    // construct AST for: func add(a, b) -> i64 { return a + b }
    let name = Token::dummy("add");
    let a = Token::dummy("a");
    let b = Token::dummy("b");

    let params = vec![
        FunctionParam { name: a.clone(), ty: None },
        FunctionParam { name: b.clone(), ty: None },
    ];

    let ret_expr = Expr::Binary { left: Box::new(Expr::Variable { name: a }), operator: Token::dummy("+"), right: Box::new(Expr::Variable { name: b }) };

    let func = Stmt::Function { name, params, return_type: Some("i64".to_string()), body: std::rc::Rc::new(Stmt::Return { value: Some(ret_expr) }), method_owner: None };

    let irf = lower_stmt(&func).expect("lowering failed");
    let text = irf.emit_text();
    let expected = "func @add(i64 a, i64 b) -> i64 {\n  entry:\n  %0 = add i64 a, b\n  ret %0\n}\n";
    assert_eq!(text, expected);
}
