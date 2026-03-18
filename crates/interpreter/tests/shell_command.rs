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
    match interp.last_value.clone().expect("shell should publish result") {
        core::ast::ArtValue::EnumInstance {
            enum_name,
            variant,
            values,
        } => {
            assert_eq!(enum_name, "Result");
            assert_eq!(variant, "Ok");
            assert_eq!(values.len(), 1);
            match &values[0] {
                core::ast::ArtValue::String(s) => {
                    assert!(s.contains("shell_ok"), "stdout payload should include shell output")
                }
                other => panic!("expected string payload, got {:?}", other),
            }
        }
        other => panic!("expected Result enum value, got {:?}", other),
    }
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

#[test]
fn runs_shell_pipeline_statement() {
    let src = "$ echo shell_pipe_ok |> tr a-z A-Z";
    let mut lx = Lexer::new(src.to_string());
    let tokens = lx.scan_tokens().expect("lex ok");
    let mut p = Parser::new(tokens);
    let (program, diags) = p.parse();
    assert!(diags.is_empty(), "parse diagnostics: {:?}", diags);

    let mut interp = Interpreter::with_prelude();
    assert!(interp.interpret(program).is_ok(), "interpreter should not fail");
    match interp.last_value.clone().expect("shell should publish result") {
        core::ast::ArtValue::EnumInstance { variant, values, .. } => {
            assert_eq!(variant, "Ok");
            match &values[0] {
                core::ast::ArtValue::String(s) => {
                    assert!(s.contains("SHELL_PIPE_OK"), "pipeline stdout should be captured")
                }
                other => panic!("expected string payload, got {:?}", other),
            }
        }
        other => panic!("expected Result enum value, got {:?}", other),
    }
}

#[test]
fn shell_command_failure_returns_result_err() {
    let src = "$ sh -c \"echo shell_err 1>&2; exit 7\"";
    let mut lx = Lexer::new(src.to_string());
    let tokens = lx.scan_tokens().expect("lex ok");
    let mut p = Parser::new(tokens);
    let (program, diags) = p.parse();
    assert!(diags.is_empty(), "parse diagnostics: {:?}", diags);

    let mut interp = Interpreter::with_prelude();
    assert!(interp.interpret(program).is_ok(), "interpreter should not fail");
    match interp.last_value.clone().expect("shell should publish result") {
        core::ast::ArtValue::EnumInstance { variant, values, .. } => {
            assert_eq!(variant, "Err");
            match &values[0] {
                core::ast::ArtValue::String(s) => {
                    assert!(s.contains("shell_err"), "stderr payload should be captured")
                }
                other => panic!("expected string payload, got {:?}", other),
            }
        }
        other => panic!("expected Result enum value, got {:?}", other),
    }
}
