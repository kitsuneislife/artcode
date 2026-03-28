use core::ast::ArtValue;
use interpreter::interpreter::Interpreter;
use lexer::lexer::Lexer;
use parser::parser::Parser;

#[test]
fn unresolved_call_executes_shell_command_and_returns_ok_result() {
    let src = r#"
let r = echo("func_shell_ok")
r
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

    match interp
        .last_value
        .clone()
        .expect("call should produce value")
    {
        ArtValue::EnumInstance {
            enum_name,
            variant,
            values,
        } => {
            assert_eq!(enum_name, "Result");
            assert_eq!(variant, "Ok");
            assert_eq!(values.len(), 1);
            match &values[0] {
                ArtValue::String(s) => assert!(s.contains("func_shell_ok")),
                other => panic!("expected Result.Ok(string), got {:?}", other),
            }
        }
        other => panic!("expected Result enum from shell call, got {:?}", other),
    }
}

#[test]
fn unresolved_call_non_zero_exit_returns_err_result() {
    let src = r#"
let r = sh("-c", "echo func_shell_err 1>&2; exit 9")
r
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

    match interp
        .last_value
        .clone()
        .expect("call should produce value")
    {
        ArtValue::EnumInstance {
            variant, values, ..
        } => {
            assert_eq!(variant, "Err");
            assert_eq!(values.len(), 1);
            match &values[0] {
                ArtValue::String(s) => assert!(s.contains("func_shell_err")),
                other => panic!("expected Result.Err(string), got {:?}", other),
            }
        }
        other => panic!("expected Result enum from shell call, got {:?}", other),
    }
}

#[test]
fn unresolved_call_is_blocked_in_pure_mode() {
    let src = "echo(\"blocked\")";

    let mut lx = Lexer::new(src.to_string());
    let tokens = lx.scan_tokens().expect("lex ok");
    let mut p = Parser::new(tokens);
    let (program, diags) = p.parse();
    assert!(diags.is_empty(), "parse diagnostics: {:?}", diags);

    let mut interp = Interpreter::with_prelude();
    interp.set_pure_mode(true);
    assert!(
        interp.interpret(program).is_ok(),
        "interpreter should not fail"
    );

    match interp
        .last_value
        .clone()
        .expect("call should produce value")
    {
        ArtValue::EnumInstance {
            variant, values, ..
        } => {
            assert_eq!(variant, "Err");
            assert_eq!(values.len(), 1);
            match &values[0] {
                ArtValue::String(s) => {
                    assert!(s.contains("Operation 'shell' is not allowed in --pure mode"))
                }
                other => panic!("expected Result.Err(string), got {:?}", other),
            }
        }
        other => panic!("expected Result enum from shell call, got {:?}", other),
    }
}
