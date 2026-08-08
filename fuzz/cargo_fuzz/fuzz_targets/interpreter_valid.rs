#![no_main]

use interpreter::Interpreter;
use lexer::Lexer;
use libfuzzer_sys::fuzz_target;
use parser::Parser;

// Seeds from corpus of valid programs; goal is zero panics regardless of input.
// Unlike parser_loops.rs which stress-tests the full pipeline, this target runs
// the interpreter on any parseable input, ensuring no panics on valid-ish programs.
fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data).to_string();
    let mut lexer = Lexer::new(input);
    let tokens = match lexer.scan_tokens() {
        Ok(t) => t,
        Err(_) => return, // lex errors are expected; skip
    };

    let mut parser = Parser::new(tokens);
    let (program, diags) = parser.parse();

    // Only run interpreter on programs without parse errors
    let has_errors = diags
        .iter()
        .any(|d| matches!(d.kind, diagnostics::DiagnosticKind::Parse | diagnostics::DiagnosticKind::Lex));
    if !has_errors {
        // Pure mode keeps the fuzzer inside the language: Artcode's shell
        // syntax otherwise reaches `Command::new(cmd).output()`, spawning a
        // real process and blocking on it. See `parser_loops.rs`.
        let mut interpreter = Interpreter::with_prelude();
        interpreter.set_pure_mode(true);
        let _ = interpreter.interpret(program);
    }
});
