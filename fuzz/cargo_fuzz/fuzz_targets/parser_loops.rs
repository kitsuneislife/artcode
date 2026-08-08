#![no_main]

use interpreter::Interpreter;
use lexer::Lexer;
use libfuzzer_sys::fuzz_target;
use parser::Parser;

fuzz_target!(|data: &[u8]| {
    let input = String::from_utf8_lossy(data).to_string();
    let mut lexer = Lexer::new(input);
    if let Ok(tokens) = lexer.scan_tokens() {
        let mut parser = Parser::new(tokens);
        let (program, _) = parser.parse();

        // Fuzz worker intentionally executes parser output to stress loop/runtime paths
        // and assert panic-free handling across malformed and edge-case programs.
        //
        // Pure mode is required, not a nicety. Artcode has shell syntax, so a
        // fuzzed program can reach `Command::new(cmd).output()`, which spawns a
        // real process and blocks until it exits. That turned the fuzzer into a
        // process launcher — the CI log showed it invoking `cc` — and any child
        // that waits on input burns the whole `-timeout` budget. Pure mode
        // rejects shell and file IO, so only the language is under test.
        let mut interpreter = Interpreter::with_prelude();
        interpreter.set_pure_mode(true);
        let _ = interpreter.interpret(program);
    }
});
