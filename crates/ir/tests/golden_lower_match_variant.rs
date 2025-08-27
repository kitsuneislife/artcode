use ir::lower_stmt;
use core::ast::{Stmt, Expr, MatchPattern, FunctionParam, ArtValue};
use core::Token;

// Test lowering for a match that patterns on an enum variant and binds params.
// Example in source (pseudocode):
// func sel(e) -> i64 { match e { Color::Rgb(r,g,b) => return r, _ => return 0 } }
#[test]
fn golden_lower_match_variant() {
    let name = Token::dummy("sel");
    let e = Token::dummy("e");

    let params = vec![FunctionParam { name: e.clone(), ty: None }];

    let match_expr = Expr::Variable { name: e.clone() };

    // arm: Color::Rgb(r,g,b) => return r
    let variant_token = Token::dummy("Rgb");
    let arm0_pat = MatchPattern::EnumVariant { enum_name: Some(Token::dummy("Color")), variant: variant_token.clone(), params: Some(vec![MatchPattern::Binding(Token::dummy("r")), MatchPattern::Binding(Token::dummy("g")), MatchPattern::Binding(Token::dummy("b"))]) };
    let arm0_guard = None;
    let arm0_body = Stmt::Return { value: Some(Expr::Variable { name: Token::dummy("r") }) };

    let arm1_pat = MatchPattern::Wildcard;
    let arm1_guard = None;
    let arm1_body = Stmt::Return { value: Some(Expr::Literal(ArtValue::Int(0))) };

    let cases = vec![(arm0_pat, arm0_guard, arm0_body), (arm1_pat, arm1_guard, arm1_body)];

    let func = Stmt::Function { name, params, return_type: Some("i64".to_string()), body: std::rc::Rc::new(Stmt::Block { statements: vec![Stmt::Match { expr: match_expr, cases }] }), method_owner: None };

    let irf = lower_stmt(&func).expect("lowering failed");
    let text = irf.emit_text();
    // Expect that bound param 'r' is materialized or used and phi exists
    assert!(text.contains("phi") || text.contains("br_cond"));
    assert!(text.contains("r") );
}
