use core::ast::ArtValue;
use interpreter::interpreter::Interpreter;
use lexer::lexer::Lexer;
use parser::parser::Parser;

#[test]
fn evaluates_pipeline_as_function_call_composition() {
    let src = r#"
func inc(x: Int) -> Int { return x + 1 }
func mul(a: Int, b: Int) -> Int { return a * b }
10 |> inc |> mul(3)
"#;

    let mut lx = Lexer::new(src.to_string());
    let tokens = lx.scan_tokens().expect("lex ok");
    let mut p = Parser::new(tokens);
    let (program, diags) = p.parse();
    assert!(diags.is_empty(), "parse diagnostics: {:?}", diags);

    let mut interp = Interpreter::with_prelude();
    assert!(
        interp.interpret(program).is_ok(),
        "interpreter should not fail"
    );
    assert_eq!(interp.last_value, Some(ArtValue::Int(33)));
}

#[test]
fn evaluates_pipeline_with_existing_call_arguments() {
    let src = r#"
func add(a: Int, b: Int) -> Int { return a + b }
5 |> add(7)
"#;

    let mut lx = Lexer::new(src.to_string());
    let tokens = lx.scan_tokens().expect("lex ok");
    let mut p = Parser::new(tokens);
    let (program, diags) = p.parse();
    assert!(diags.is_empty(), "parse diagnostics: {:?}", diags);

    let mut interp = Interpreter::with_prelude();
    assert!(
        interp.interpret(program).is_ok(),
        "interpreter should not fail"
    );
    assert_eq!(interp.last_value, Some(ArtValue::Int(12)));
}
