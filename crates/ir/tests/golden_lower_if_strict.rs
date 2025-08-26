use ir::lower_stmt;
use core::ast::{Stmt, Expr};
use core::Token;

#[test]
fn golden_lower_if_strict() {
    // func f() -> i64 { if true { return 1 } else { return 2 } }
    let name = Token::dummy("f_strict");
    let params = vec![];
    let cond = Expr::Literal(core::ast::ArtValue::Bool(true));
    let then_stmt = Stmt::Return { value: Some(Expr::Literal(core::ast::ArtValue::Int(1))) };
    let else_stmt = Stmt::Return { value: Some(Expr::Literal(core::ast::ArtValue::Int(2))) };
    let if_stmt = Stmt::If { condition: cond, then_branch: Box::new(then_stmt), else_branch: Some(Box::new(else_stmt)) };
    let func = Stmt::Function { name, params, return_type: Some("i64".to_string()), body: std::rc::Rc::new(Stmt::Block { statements: vec![if_stmt] }), method_owner: None };

    let irf = lower_stmt(&func).expect("lowering failed");
    let text = irf.emit_text();

    // Validate key structure
    assert!(text.contains("br_cond"));
    assert!(text.contains("_then:"));
    assert!(text.contains("_else:"));
    assert!(text.contains("_merge:"));
    assert!(text.contains("phi i64"));
}
