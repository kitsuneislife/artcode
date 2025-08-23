use interpreter::Interpreter;
use lexer::Lexer;
use parser::Parser;

#[test]
fn struct_missing_field_diagnostic() {
    let src = r#"
        struct Pessoa { nome: String, idade: Int }
        let p = Pessoa { nome: "Ana" }
    "#;
    let mut lx = Lexer::new(src.to_string());
    let tokens = match lx.scan_tokens() {
        Ok(t) => t,
        Err(e) => {
            assert!(false, "lexer scan_tokens in runtime_missing_fields.rs failed: {:?}", e);
            Vec::new()
        }
    };
    let mut parser = Parser::new(tokens);
    let (program, _) = parser.parse();
    let mut interp = Interpreter::new();
    let _ = interp.interpret(program);
    let diags = interp.take_diagnostics();
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("Missing field 'idade'"))
    );
}

#[test]
fn enum_missing_field_diagnostic() {
    let src = r#"
        enum Status { Ok(Int), Err(String) }
        let s = Status.Ok()
    "#;
    let mut lx = Lexer::new(src.to_string());
    let tokens = match lx.scan_tokens() {
        Ok(t) => t,
        Err(e) => {
            assert!(false, "lexer scan_tokens in runtime_missing_fields.rs failed: {:?}", e);
            Vec::new()
        }
    };
    let mut parser = Parser::new(tokens);
    let (program, _) = parser.parse();
    let mut interp = Interpreter::new();
    let _ = interp.interpret(program);
    let diags = interp.take_diagnostics();
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("Wrong number of arguments"))
    );
}
