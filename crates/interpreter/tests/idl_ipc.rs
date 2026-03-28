use core::ast::ArtValue;
use interpreter::Interpreter;
use lexer::Lexer;
use parser::Parser;

fn parse_program(src: &str) -> Vec<core::ast::Stmt> {
    let mut lexer = Lexer::new(src.to_string());
    let tokens = lexer.scan_tokens().expect("tokens");
    let mut parser = Parser::new(tokens);
    let (program, diags) = parser.parse();
    assert!(
        diags.is_empty(),
        "unexpected parser diagnostics: {:?}",
        diags.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
    );
    program
}

#[test]
fn idl_schema_returns_struct_field_map() {
    let src = r#"
        struct BootMsg { service: String, retries: Int }
        let schema = idl_schema("BootMsg")
    "#;

    let program = parse_program(src);
    let mut interp = Interpreter::with_prelude();
    interp.interpret(program).expect("interpret");

    let schema_val = interp.debug_get_global("schema").expect("schema global");
    let ArtValue::Map(map_ref) = schema_val else {
        panic!("expected Map schema");
    };
    let map = map_ref.0.lock().unwrap_or_else(|e| e.into_inner());

    assert_eq!(map.get("service"), Some(&ArtValue::String("String".into())));
    assert_eq!(map.get("retries"), Some(&ArtValue::String("Int".into())));

    let diags = interp.take_diagnostics();
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diags.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn idl_validate_checks_runtime_field_types() {
    let src = r#"
        struct BootMsg { service: String, retries: Int }
        let good = BootMsg { service: "nexus", retries: 3 }
        let bad = BootMsg { service: "nexus", retries: "oops" }

        let ok = idl_validate(good, "BootMsg")
        let nok = idl_validate(bad, "BootMsg")
    "#;

    let program = parse_program(src);
    let mut interp = Interpreter::with_prelude();
    interp.interpret(program).expect("interpret");

    assert_eq!(interp.debug_get_global("ok"), Some(ArtValue::Bool(true)));
    assert_eq!(interp.debug_get_global("nok"), Some(ArtValue::Bool(false)));

    let diags = interp.take_diagnostics();
    assert!(
        diags.iter().any(|d| d
            .message
            .contains("idl_validate: field 'retries' expected 'Int'")),
        "expected type mismatch diagnostic, got {:?}",
        diags.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
    );
}
