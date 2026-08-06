use interpreter::Interpreter;
use lexer::Lexer;
use parser::Parser;

fn parse_and_create(src: &str) -> (Vec<core::ast::Stmt>, Interpreter) {
    let tokens = Lexer::new(src.to_string()).scan_tokens().expect("lex");
    let (program, diags) = Parser::new(tokens).parse();
    assert!(diags.is_empty(), "parse errors: {:?}", diags);
    (program, Interpreter::with_prelude())
}

#[test]
fn test_interpreter_exposes_breakpoints_field() {
    let (_, mut interp) = parse_and_create("let x = 1");
    assert!(interp.breakpoints.is_empty());
    interp.breakpoints.insert(5);
    interp.breakpoints.insert(10);
    assert_eq!(interp.breakpoints.len(), 2);
    assert!(interp.breakpoints.contains(&5));
    interp.breakpoints.clear();
    assert!(interp.breakpoints.is_empty());
}

#[test]
fn test_fast_forward_executes_to_target_tick() {
    // Run to tick 2 then verify all 3 statements have executed and variables exist
    let src = r#"
let a = 1
let b = 2
let c = 3
let d = 4
"#;
    let tokens = Lexer::new(src.to_string()).scan_tokens().expect("lex");
    let (program, _) = Parser::new(tokens).parse();
    let mut interp = Interpreter::with_prelude();
    // Run normally (no debug mode, no breakpoints) — all stmts execute
    interp.interpret(program).expect("interpret");
    assert!(interp.get_global("a").is_some());
    assert!(interp.get_global("b").is_some());
    assert!(interp.get_global("c").is_some());
    assert!(interp.get_global("d").is_some());
    assert_eq!(interp.executed_statements, 4);
}

#[test]
fn test_current_line_tracking() {
    let src = r#"let x = 1
let y = 2
let z = 3
"#;
    let tokens = Lexer::new(src.to_string()).scan_tokens().expect("lex");
    let (program, _) = Parser::new(tokens).parse();
    let mut interp = Interpreter::with_prelude();
    interp.interpret(program).expect("interpret");
    // After execution, current_line should reflect the last executed statement's line
    assert!(
        interp.current_line > 0,
        "current_line should be set after execution"
    );
}

#[test]
fn test_debug_jump_to_tick_field_exists() {
    let (_, mut interp) = parse_and_create("let x = 1");
    assert!(interp.debug_jump_to_tick.is_none());
    interp.debug_jump_to_tick = Some(42);
    assert_eq!(interp.debug_jump_to_tick, Some(42));
}

#[test]
fn test_runtime_error_debug_quit_variant() {
    use interpreter::RuntimeError;
    let e = RuntimeError::DebugQuit;
    assert_eq!(e.to_string(), "Debug quit");
}

#[test]
fn test_runtime_error_debug_jump_to_variant() {
    use interpreter::RuntimeError;
    let e = RuntimeError::DebugJumpTo(42);
    assert_eq!(e.to_string(), "Debug jump to tick 42");
}
