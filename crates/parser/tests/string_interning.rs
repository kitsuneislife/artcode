use core::ast::{ArtValue, Expr, MatchPattern, Stmt};
use lexer::lexer::Lexer;
use parser::parser::Parser;

fn parse_program(src: &str) -> Vec<Stmt> {
    let mut lx = Lexer::new(src.to_string());
    let tokens = match lx.scan_tokens() {
        Ok(t) => t,
        Err(e) => panic!("lexer failed in string_interning.rs: {:?}", e),
    };
    let mut p = Parser::new(tokens);
    let (program, diags) = p.parse();
    assert!(
        diags.is_empty(),
        "parser diagnostics in string_interning.rs: {:?}",
        diags
    );
    program
}

#[test]
fn literal_strings_share_same_arc_instance() {
    let program = parse_program("let a = \"alpha\"; let b = \"alpha\";");

    let first = match &program[0] {
        Stmt::Let { initializer, .. } => match initializer {
            Expr::Literal(ArtValue::String(s)) => s.clone(),
            _ => panic!("expected string literal for first let"),
        },
        _ => panic!("expected first statement to be let"),
    };

    let second = match &program[1] {
        Stmt::Let { initializer, .. } => match initializer {
            Expr::Literal(ArtValue::String(s)) => s.clone(),
            _ => panic!("expected string literal for second let"),
        },
        _ => panic!("expected second statement to be let"),
    };

    assert!(
        std::sync::Arc::ptr_eq(&first, &second),
        "equal string literals should share interned Arc"
    );
}

#[test]
fn match_pattern_string_literals_are_interned() {
    let program =
        parse_program("match \"x\" { case \"hit\": println(1); case \"hit\": println(2); }");

    let (p1, p2) = match &program[0] {
        Stmt::Match { cases, .. } => {
            let left = match &cases[0].0 {
                MatchPattern::Literal(ArtValue::String(s)) => s.clone(),
                _ => panic!("expected first case pattern to be string literal"),
            };
            let right = match &cases[1].0 {
                MatchPattern::Literal(ArtValue::String(s)) => s.clone(),
                _ => panic!("expected second case pattern to be string literal"),
            };
            (left, right)
        }
        _ => panic!("expected match statement"),
    };

    assert!(
        std::sync::Arc::ptr_eq(&p1, &p2),
        "equal string match patterns should share interned Arc"
    );
}
