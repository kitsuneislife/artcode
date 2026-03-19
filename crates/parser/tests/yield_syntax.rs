use core::ast::{ArtValue, Expr, Stmt};
use lexer::lexer::Lexer;
use parser::parser::Parser;

#[test]
fn parses_yield_as_return_option_some() {
    let src = "func gen() { yield 7; return Option.None; }";
    let mut lx = Lexer::new(src.to_string());
    let tokens = lx.scan_tokens().expect("lex ok");
    let mut p = Parser::new(tokens);
    let (program, diags) = p.parse();
    assert!(diags.is_empty(), "parse diagnostics: {:?}", diags);

    match &program[0] {
        Stmt::Function { body, .. } => match body.as_ref() {
            Stmt::Block { statements } => match &statements[0] {
                Stmt::Return {
                    value:
                        Some(Expr::EnumInit {
                            name: Some(name),
                            variant,
                            values,
                        }),
                } => {
                    assert_eq!(name.lexeme, "Option");
                    assert_eq!(variant.lexeme, "Some");
                    assert_eq!(values.len(), 1);
                    assert_eq!(values[0], Expr::Literal(ArtValue::Int(7)));
                }
                other => panic!("expected desugared return Option.Some, got {:?}", other),
            },
            other => panic!("expected function block, got {:?}", other),
        },
        other => panic!("expected function declaration, got {:?}", other),
    }
}
