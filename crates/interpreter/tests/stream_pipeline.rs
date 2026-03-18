use core::ast::ArtValue;
use interpreter::interpreter::Interpreter;
use lexer::lexer::Lexer;
use parser::parser::Parser;

#[test]
fn evaluates_lazy_stream_pipeline_count_without_intermediate_arrays() {
    let src = r#"
func inc(x: Int) -> Int { return x + 1 }
func is_even(x: Int) -> Bool { return ((x / 2) * 2) == x }

[1, 2, 3, 4, 5] |> stream |> map(inc) |> filter(is_even) |> count
"#;

    let mut lx = Lexer::new(src.to_string());
    let tokens = lx.scan_tokens().expect("lex ok");
    let mut p = Parser::new(tokens);
    let (program, diags) = p.parse();
    assert!(diags.is_empty(), "parse diagnostics: {:?}", diags);

    let mut interp = Interpreter::with_prelude();
    assert!(interp.interpret(program).is_ok(), "interpreter should not fail");
    assert_eq!(interp.last_value, Some(ArtValue::Int(3)));
}
