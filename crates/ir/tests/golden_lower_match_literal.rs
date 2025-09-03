use ir::lower_stmt;
use core::ast::{Stmt, Expr, MatchPattern, FunctionParam, ArtValue};
use core::Token;

// Test lowering for a match that patterns on a literal integer.
// func sel(x) -> i64 { match x { 1 => return 42, _ => return 0 } }
#[test]
fn golden_lower_match_literal() {
    let name = Token::dummy("sel");
    let x = Token::dummy("x");

    let params = vec![FunctionParam { name: x.clone(), ty: None }];

    let match_expr = Expr::Variable { name: x.clone() };

    let arm0_pat = MatchPattern::Literal(ArtValue::Int(1));
    let arm0_guard = None;
    let arm0_body = Stmt::Return { value: Some(Expr::Literal(ArtValue::Int(42))) };

    let arm1_pat = MatchPattern::Wildcard;
    let arm1_guard = None;
    let arm1_body = Stmt::Return { value: Some(Expr::Literal(ArtValue::Int(0))) };

    let cases = vec![(arm0_pat, arm0_guard, arm0_body), (arm1_pat, arm1_guard, arm1_body)];

    let func = Stmt::Function { name, params, return_type: Some("i64".to_string()), body: std::rc::Rc::new(Stmt::Block { statements: vec![Stmt::Match { expr: match_expr, cases }] }), method_owner: None };

    let irf = lower_stmt(&func).expect("lowering failed");
    let text = irf.emit_text();
    assert!(text.contains("br_cond"));
    assert!(text.contains("phi"));
    assert!(text.contains("42"));
}
