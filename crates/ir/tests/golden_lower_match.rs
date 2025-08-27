use ir::lower_stmt;
use core::ast::{Stmt, Expr, MatchPattern, FunctionParam, ArtValue};
use core::Token;

// Test lowering for a match that returns different integer literals per arm.
// Example in source:
// func selector(x) -> i64 { match x { 0 => return 10, _ => return 20 } }
#[test]
fn golden_lower_match_literals() {
    let name = Token::dummy("selector");
    let x = Token::dummy("x");

    let params = vec![FunctionParam { name: x.clone(), ty: None }];

    let match_expr = Expr::Variable { name: x.clone() };

    // arms: 0 => return 10, _ => return 20
    let arm0_pat = MatchPattern::Literal(ArtValue::Int(0));
    let arm0_guard = None;
    let arm0_body = Stmt::Return { value: Some(Expr::Literal(ArtValue::Int(10))) };

    let arm1_pat = MatchPattern::Wildcard;
    let arm1_guard = None;
    let arm1_body = Stmt::Return { value: Some(Expr::Literal(ArtValue::Int(20))) };

    let cases = vec![(arm0_pat, arm0_guard, arm0_body), (arm1_pat, arm1_guard, arm1_body)];

    let func = Stmt::Function { name, params, return_type: Some("i64".to_string()), body: std::rc::Rc::new(Stmt::Block { statements: vec![Stmt::Match { expr: match_expr, cases }] }), method_owner: None };

    let irf = lower_stmt(&func).expect("lowering failed");
    let text = irf.emit_text();
    // We expect a br_switch or sequence of br_cond labels; for our simple lowering we'll
    // compare a substring to ensure shape.
    assert!(text.contains("br_cond"));
    assert!(text.contains("phi"));
}
