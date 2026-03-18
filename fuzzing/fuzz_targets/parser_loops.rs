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
        let mut interpreter = Interpreter::with_prelude();
        let _ = interpreter.interpret(program);
    }
});
