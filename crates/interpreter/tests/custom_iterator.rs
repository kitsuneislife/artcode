use core::ast::ArtValue;
use interpreter::interpreter::Interpreter;
use lexer::lexer::Lexer;
use parser::parser::Parser;

#[test]
fn for_loop_over_generator_closure_returns_option() {
    let src = r#"
// Use a mutable map as shared state for the iterator.
let state = map_new();
map_set(state, "i", 0);

func gen() {
    // Increment stored counter and yield up to 3.
    map_set(state, "i", map_get(state, "i").unwrap_or(0) + 1);
    if map_get(state, "i").unwrap_or(0) <= 3 {
        return Option.Some(map_get(state, "i").unwrap_or(0));
    }
    return Option.None;
}

let acc = map_new();
map_set(acc, "sum", 0);
for x in gen {
    map_set(acc, "sum", map_get(acc, "sum").unwrap_or(0) + x);
}
map_get(acc, "sum").unwrap_or(0)
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
    let diags = interp.take_diagnostics();
    assert!(diags.is_empty(), "runtime diagnostics: {:?}", diags);
    assert_eq!(interp.last_value, Some(ArtValue::Int(6)));
}
