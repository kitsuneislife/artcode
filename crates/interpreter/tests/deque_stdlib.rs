use core::ast::ArtValue;
use interpreter::interpreter::Interpreter;

fn run(src: &str) -> ArtValue {
    let tokens = lexer::Lexer::new(src.to_string())
        .scan_tokens()
        .expect("lex");
    let (program, diags) = parser::Parser::new(tokens).parse();
    assert!(diags.is_empty(), "parse errors: {:?}", diags);
    let mut interp = Interpreter::with_prelude();
    interp.interpret(program).expect("runtime error");
    interp.last_value.unwrap_or(ArtValue::none())
}

#[test]
fn deque_new_creates_empty_deque() {
    let val = run("let d = deque_new()\ndeque_len(d)");
    assert_eq!(val, ArtValue::Int(0));
}

#[test]
fn deque_push_back_and_len() {
    let val =
        run("let d = deque_new()\ndeque_push_back(d, 1)\ndeque_push_back(d, 2)\ndeque_len(d)");
    assert_eq!(val, ArtValue::Int(2));
}

#[test]
fn deque_push_front_prepends() {
    let val = run(
        "let d = deque_new()\ndeque_push_back(d, 2)\ndeque_push_front(d, 1)\ndeque_pop_front(d).unwrap_or(99)",
    );
    assert_eq!(val, ArtValue::Int(1));
}

#[test]
fn deque_pop_front_is_fifo() {
    let val = run(
        "let d = deque_new()\ndeque_push_back(d, 10)\ndeque_push_back(d, 20)\ndeque_pop_front(d).unwrap_or(0)",
    );
    assert_eq!(val, ArtValue::Int(10));
}

#[test]
fn deque_pop_back_is_lifo_from_back() {
    let val = run(
        "let d = deque_new()\ndeque_push_back(d, 10)\ndeque_push_back(d, 20)\ndeque_pop_back(d).unwrap_or(0)",
    );
    assert_eq!(val, ArtValue::Int(20));
}

#[test]
fn deque_pop_front_on_empty_returns_none() {
    let val = run("let d = deque_new()\ndeque_pop_front(d).unwrap_or(42)");
    assert_eq!(val, ArtValue::Int(42));
}
