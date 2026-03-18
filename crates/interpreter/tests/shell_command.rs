use interpreter::interpreter::Interpreter;
use lexer::lexer::Lexer;
use parser::parser::Parser;

#[test]
fn runs_shell_command_statement() {
    let src = "$ echo shell_ok";
    let mut lx = Lexer::new(src.to_string());
    let tokens = lx.scan_tokens().expect("lex ok");
    let mut p = Parser::new(tokens);
    let (program, diags) = p.parse();
    assert!(diags.is_empty(), "parse diagnostics: {:?}", diags);

    let mut interp = Interpreter::with_prelude();
    assert!(interp.interpret(program).is_ok(), "interpreter should not fail");
}

#[test]
fn shell_command_is_blocked_in_pure_mode() {
    let src = "$ echo blocked";
    let mut lx = Lexer::new(src.to_string());
    let tokens = lx.scan_tokens().expect("lex ok");
    let mut p = Parser::new(tokens);
    let (program, diags) = p.parse();
    assert!(diags.is_empty(), "parse diagnostics: {:?}", diags);

    let mut interp = Interpreter::with_prelude();
    interp.set_pure_mode(true);
    assert!(interp.interpret(program).is_ok(), "interpreter should not hard-fail");
    let diags = interp.take_diagnostics();
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("Operation 'shell' is not allowed in --pure mode")),
        "expected pure-mode diagnostic for shell command"
    );
}
