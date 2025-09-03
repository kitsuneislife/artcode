use ir::lower_stmt;
use core::ast::{Stmt, Expr};
use core::Token;

// Test lowering for nested if: outer if's branches are themselves ifs.
// func f() -> i64 { if true { if false { return 1 } else { return 2 } } else { return 3 } }
#[test]
fn golden_lower_nested_if() {
    let name = Token::dummy("f_nested");
    let params = vec![];
    let inner_then = Stmt::Return { value: Some(Expr::Literal(core::ast::ArtValue::Int(1))) };
    let inner_else = Stmt::Return { value: Some(Expr::Literal(core::ast::ArtValue::Int(2))) };
    let inner_if = Stmt::If { condition: Expr::Literal(core::ast::ArtValue::Bool(false)), then_branch: Box::new(inner_then), else_branch: Some(Box::new(inner_else)) };

    let outer_else = Stmt::Return { value: Some(Expr::Literal(core::ast::ArtValue::Int(3))) };
    let outer_if = Stmt::If { condition: Expr::Literal(core::ast::ArtValue::Bool(true)), then_branch: Box::new(inner_if), else_branch: Some(Box::new(outer_else)) };

    let func = Stmt::Function { name, params, return_type: Some("i64".to_string()), body: std::rc::Rc::new(Stmt::Block { statements: vec![outer_if] }), method_owner: None };
    let irf = lower_stmt(&func).expect("lowering failed");
    let text = irf.emit_text();
    // Check that nested lowering emitted inner labels and phis
    assert!(text.contains("_then:"));
    assert!(text.contains("_else:"));
    assert!(text.contains("phi i64"));
}
