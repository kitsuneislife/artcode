use cli::formatter::format_string;
use lexer::Lexer;
use parser::Parser;

fn parse(src: &str) -> Vec<core::ast::Stmt> {
    let mut lexer = Lexer::new(src.to_string());
    let tokens = lexer.scan_tokens().expect("lex");
    let mut parser = Parser::new(tokens);
    let (program, _) = parser.parse();
    program
}

// Property: parse(format(src)) produces the same AST as parse(src),
// for any source that parses without errors.
fn assert_roundtrip(src: &str) {
    let ast1 = parse(src);
    let formatted = format_string(src);
    let ast2 = parse(&formatted);
    assert_eq!(
        ast1, ast2,
        "AST mismatch after formatting.\n  original:  {src:?}\n  formatted: {formatted:?}"
    );
}

#[test]
fn roundtrip_let_binding() {
    assert_roundtrip("let x = 42");
}

#[test]
fn roundtrip_function_definition() {
    assert_roundtrip("func add(a, b) { return a + b }");
}

#[test]
fn roundtrip_nested_blocks() {
    assert_roundtrip("func f(x) { if x > 0 { return x } return 0 }");
}

#[test]
fn roundtrip_while_loop() {
    assert_roundtrip("let i = 0\nwhile i < 10 { i = i + 1 }");
}

#[test]
fn roundtrip_for_loop() {
    assert_roundtrip("for x in items { println(x) }");
}

#[test]
fn roundtrip_struct_definition() {
    assert_roundtrip("struct Point { x, y }");
}

#[test]
fn roundtrip_enum_definition() {
    assert_roundtrip("enum Color { Red, Green, Blue }");
}

#[test]
fn roundtrip_match_expression() {
    assert_roundtrip("match val { 1 => println(\"one\"), _ => println(\"other\") }");
}

#[test]
fn roundtrip_idempotent() {
    // format(format(src)) == format(src)
    let src = "func f(x) { return x + 1 }";
    let once = format_string(src);
    let twice = format_string(&once);
    assert_eq!(once, twice, "formatter is not idempotent");
}
