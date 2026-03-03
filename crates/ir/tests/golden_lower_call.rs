use core::ast::{Expr, FunctionParam, Stmt};
use core::Token;
use ir::lower_stmt;

#[test]
fn golden_lower_call() {
    // func caller(a, b) -> i64 { return add(a, b) }
    let name = Token::dummy("caller");
    let a = Token::dummy("a");
    let b = Token::dummy("b");

    let params = vec![
        FunctionParam {
            name: a.clone(),
            ty: None,
        },
        FunctionParam {
            name: b.clone(),
            ty: None,
        },
    ];

    let call_expr = Expr::Call {
        callee: Box::new(Expr::Variable {
            name: Token::dummy("add"),
        }),
        arguments: vec![Expr::Variable { name: a }, Expr::Variable { name: b }],
    };
    let func = Stmt::Function {
        name,
        params,
        return_type: Some("i64".to_string()),
        body: std::rc::Rc::new(Stmt::Return {
            value: Some(call_expr),
        }),
        method_owner: None,
    };

    let irf = lower_stmt(&func).expect("lowering failed");
    let text = irf.emit_text();

    // Espera que a chamada seja convertida para Instr::Call e temp único
    let expected = "func @caller(i64 a, i64 b) -> i64 {\n  entry:\n  %caller_0 = call add(a, b)\n  ret %caller_0\n}\n";
    assert_eq!(text, expected);
}
