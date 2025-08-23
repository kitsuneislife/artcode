use interpreter::interpreter::Interpreter;
use lexer::lexer::Lexer;
use parser::parser::Parser;

#[test]
fn enum_shorthand_ambiguous_diagnostic() {
    let source = "enum A { X(Int) } enum B { X(Int) } let v = .X(1);";
    let mut lx = Lexer::new(source.to_string());
    let tokens = match lx.scan_tokens() {
        Ok(t) => t,
        Err(e) => {
            assert!(false, "lexer scan_tokens in enum_ambiguous.rs failed: {:?}", e);
            Vec::new()
        }
    };
    let mut p = Parser::new(tokens);
    let (program, pdiags) = p.parse();
    assert!(pdiags.is_empty());
    let mut interp = Interpreter::with_prelude();
    let _ = interp.interpret(program);
    let rdiags = interp.take_diagnostics();
    assert!(
        rdiags
            .iter()
            .any(|d| d.message.contains("Ambiguous enum variant"))
    );
}
