use lexer::Lexer;
use parser::Parser;
use interpreter::Interpreter;

#[test]
fn fstring_simple_expr() {
    let mut lexer = Lexer::new("let a = 10; let b = 5; println(f\"sum={a + b}\");".to_string());
    let tokens = lexer.scan_tokens();
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    let mut interp = Interpreter::with_prelude();
    interp.interpret(program).unwrap();
}

#[test]
fn enum_shorthand_ok() {
    let mut lexer = Lexer::new("let x = .Ok(123);".to_string());
    let tokens = lexer.scan_tokens();
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    let mut interp = Interpreter::with_prelude();
    interp.interpret(program).unwrap();
}
