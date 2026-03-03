use core::ast::{Expr, FunctionParam, Stmt};
use core::Token;
use ir::lower_stmt;

#[test]
fn golden_ssa_unique() {
    // func dup(a) -> i64 { return a + a }
    let name = Token::dummy("dup");
    let a = Token::dummy("a");
    let params = vec![FunctionParam {
        name: a.clone(),
        ty: None,
    }];
    let ret_expr = Expr::Binary {
        left: Box::new(Expr::Variable { name: a.clone() }),
        operator: Token::dummy("+"),
        right: Box::new(Expr::Variable { name: a.clone() }),
    };
    let func = Stmt::Function {
        name,
        params,
        return_type: Some("i64".to_string()),
        body: std::rc::Rc::new(Stmt::Return {
            value: Some(ret_expr),
        }),
        method_owner: None,
    };

    let irf = lower_stmt(&func).expect("lowering failed");
    let text = irf.emit_text();

    // Espera que o nome temporário inclua o prefixo da função e índice 0
    assert!(text.contains("%dup_0 = add i64 a, a"));
}
