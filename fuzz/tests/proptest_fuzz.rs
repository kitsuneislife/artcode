use interpreter::Interpreter;
use lexer::Lexer;
use parser::Parser;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn fuzzing_parser_symmetry_and_stability(s in "\\PC{0,200}") {
        // Stress-tests the Lexer + Parser on fully random UTF-8.
        // Goal: no panics, no infinite loops, handle any malformed input gracefully.
        let mut lexer = Lexer::new(s.clone());
        if let Ok(tokens) = lexer.scan_tokens() {
            let mut parser = Parser::new(tokens);
            let _ = parser.parse();
        }
    }
}

/// The evaluator test runs in a thread with a large stack (32 MB) to give the
/// interpreter plenty of room even in unoptimised debug builds.
#[test]
fn fuzzing_evaluator_stability() {
    let config = ProptestConfig::with_cases(500);
    let strategy = "[a-zA-Z0-9_ \n\t(){}\\.\\[\\]\\+\\-\\*/=!:.;]{0,80}".to_string();
    let mut runner = proptest::test_runner::TestRunner::new(config);
    let strategy = strategy.as_str();

    runner
        .run(&proptest::string::string_regex(strategy).unwrap(), |s| {
            // Each case runs in its own thread with 32 MB stack.
            let result = std::thread::Builder::new()
                .stack_size(32 * 1024 * 1024)
                .spawn(move || {
                    let mut lexer = Lexer::new(String::from(s.clone()));
                    if let Ok(tokens) = lexer.scan_tokens() {
                        let mut parser = Parser::new(tokens);
                        let (program, _) = parser.parse();
                        let mut interp = Interpreter::with_prelude();
                        let _ = interp.interpret(program);
                    }
                })
                .expect("thread spawn failed")
                .join();

            match result {
                Ok(()) => Ok(()),
                Err(_) => Err(proptest::test_runner::TestCaseError::fail(
                    "evaluator panicked / overflowed stack on this input",
                )),
            }
        })
        .unwrap_or_else(|e| {
            // If proptest finds a shrunk failing case, it will panic here.
            panic!("proptest found a failure: {e}");
        });
}
