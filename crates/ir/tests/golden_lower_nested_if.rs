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
    // Strict golden: exact expected IR text
    let expected = "func @f_nested() -> i64 {\n  %f_nested_0 = const i64 1\n  br_cond %f_nested_0, f_nested_then, f_nested_else\nf_nested_then:\n  %f_nested_3 = const i64 0\n  br_cond %f_nested_3, f_nested_then_0, f_nested_else_1\nf_nested_then_0:\n  %f_nested_4 = const i64 1\n  %f_nested_1 = add i64 %f_nested_4, 0\n  br f_nested_merge_2\nf_nested_else_1:\n  %f_nested_5 = const i64 2\n  %f_nested_2 = add i64 %f_nested_5, 0\n  br f_nested_merge_2\nf_nested_merge_2:\n  %f_nested_1 = phi i64 [ %f_nested_1, f_nested_then_0 ], [ %f_nested_2, f_nested_else_1 ]\n  br f_nested_merge\nf_nested_else:\n  %f_nested_6 = const i64 3\n  br f_nested_merge\nf_nested_merge:\n  %f_nested_7 = phi i64 [ %f_nested_1, f_nested_then ], [ %f_nested_6, f_nested_else ]\n  ret %f_nested_7\n}\n";
    assert_eq!(text, expected, "nested-if lowering differs from expected golden output\n\nexpected:\n{}\n\nactual:\n{}", expected, text);
}
