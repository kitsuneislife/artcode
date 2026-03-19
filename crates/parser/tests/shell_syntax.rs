use core::ast::Stmt;
use lexer::lexer::Lexer;
use parser::parser::Parser;

#[test]
fn parses_shell_command_statement() {
    let src = "$ echo -n hello;\nprintln(1);";
    let mut lx = Lexer::new(src.to_string());
    let tokens = lx.scan_tokens().expect("lex ok");
    let mut p = Parser::new(tokens);
    let (program, diags) = p.parse();
    assert!(diags.is_empty(), "parse diagnostics: {:?}", diags);

    match &program[0] {
        Stmt::ShellCommand { program, args } => {
            assert_eq!(program, "echo");
            assert_eq!(args, &vec!["-n".to_string(), "hello".to_string()]);
        }
        other => panic!("expected shell command stmt, got {:?}", other),
    }
}

#[test]
fn parses_shell_pipeline_statement() {
    let src = "$ echo hello |> tr a-z A-Z;";
    let mut lx = Lexer::new(src.to_string());
    let tokens = lx.scan_tokens().expect("lex ok");
    let mut p = Parser::new(tokens);
    let (program, diags) = p.parse();
    assert!(diags.is_empty(), "parse diagnostics: {:?}", diags);

    match &program[0] {
        Stmt::ShellCommand { program, args } => {
            assert_eq!(program, "echo");
            assert!(args.iter().any(|a| a == "|>"));
            assert_eq!(
                args,
                &vec![
                    "hello".to_string(),
                    "|>".to_string(),
                    "tr".to_string(),
                    "a-z".to_string(),
                    "A-Z".to_string()
                ]
            );
        }
        other => panic!("expected shell command stmt, got {:?}", other),
    }
}
