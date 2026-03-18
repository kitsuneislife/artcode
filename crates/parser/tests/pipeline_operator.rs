use core::ast::{ArtValue, Expr, Stmt};
use lexer::lexer::Lexer;
use parser::parser::Parser;

#[test]
fn parses_pipeline_into_single_argument_call() {
    let src = "let x = 1 |> inc;";
    let mut lx = Lexer::new(src.to_string());
    let tokens = lx.scan_tokens().expect("lex ok");
    let mut p = Parser::new(tokens);
    let (program, diags) = p.parse();
    assert!(diags.is_empty(), "parse diagnostics: {:?}", diags);

    match &program[0] {
        Stmt::Let { initializer, .. } => match initializer {
            Expr::Call {
                callee, arguments, ..
            } => {
                match &**callee {
                    Expr::Variable { name } => assert_eq!(name.lexeme, "inc"),
                    other => panic!("expected callee variable, got {:?}", other),
                }
                assert_eq!(arguments.len(), 1);
                assert_eq!(arguments[0], Expr::Literal(ArtValue::Int(1)));
            }
            other => panic!("expected call expression, got {:?}", other),
        },
        other => panic!("expected let statement, got {:?}", other),
    }
}

#[test]
fn parses_pipeline_into_prepended_call_argument() {
    let src = "let x = 2 |> add(3);";
    let mut lx = Lexer::new(src.to_string());
    let tokens = lx.scan_tokens().expect("lex ok");
    let mut p = Parser::new(tokens);
    let (program, diags) = p.parse();
    assert!(diags.is_empty(), "parse diagnostics: {:?}", diags);

    match &program[0] {
        Stmt::Let { initializer, .. } => match initializer {
            Expr::Call {
                callee, arguments, ..
            } => {
                match &**callee {
                    Expr::Variable { name } => assert_eq!(name.lexeme, "add"),
                    other => panic!("expected callee variable, got {:?}", other),
                }
                assert_eq!(arguments.len(), 2);
                assert_eq!(arguments[0], Expr::Literal(ArtValue::Int(2)));
                assert_eq!(arguments[1], Expr::Literal(ArtValue::Int(3)));
            }
            other => panic!("expected call expression, got {:?}", other),
        },
        other => panic!("expected let statement, got {:?}", other),
    }
}
