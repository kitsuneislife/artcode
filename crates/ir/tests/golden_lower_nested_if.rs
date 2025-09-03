use ir::lower_stmt;
use core::ast::{Stmt, Expr};
use core::Token;

// Test lowering for nested ifs
// func g() -> i64 { if true { if false { return 1 } else { return 2 } } else { return 3 } }
#[test]
fn golden_lower_nested_if() {
    let name = Token::dummy("g");
    let params = vec![];
    let inner_cond = Expr::Literal(core::ast::ArtValue::Bool(false));
    let inner_then = Stmt::Return { value: Some(Expr::Literal(core::ast::ArtValue::Int(1))) };
    let inner_else = Stmt::Return { value: Some(Expr::Literal(core::ast::ArtValue::Int(2))) };
    let inner_if = Stmt::If { condition: inner_cond, then_branch: Box::new(inner_then), else_branch: Some(Box::new(inner_else)) };

    let outer_cond = Expr::Literal(core::ast::ArtValue::Bool(true));
    let outer_then = inner_if;
    let outer_else = Stmt::Return { value: Some(Expr::Literal(core::ast::ArtValue::Int(3))) };
    let outer_if = Stmt::If { condition: outer_cond, then_branch: Box::new(outer_then), else_branch: Some(Box::new(outer_else)) };

    let func = Stmt::Function { name, params, return_type: Some("i64".to_string()), body: std::rc::Rc::new(Stmt::Block { statements: vec![outer_if] }), method_owner: None };
    let irf = lower_stmt(&func).expect("lowering failed");
    let text = irf.emit_text();
    // Expect nested labels/phis/br_cond to appear for nested control flow
    assert!(text.contains("br_cond"), "expected br_cond in nested if lowering, got: {}", text);
    assert!(text.contains("phi") || text.contains("ret"), "expected phi or ret in nested if lowering, got: {}", text);
}
