use core::ast::ArtValue;
use interpreter::Interpreter;
use interpreter::type_infer::{TypeEnv, TypeInfer};
use lexer::Lexer;
use parser::Parser;

// ==========================================================================
// Helpers
// ==========================================================================

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

fn run_and_interpret(src: &str) -> Interpreter {
    let program = parse_program(src);
    let mut interp = Interpreter::with_prelude();
    interp.interpret(program).expect("interpret");
    interp
}

// ==========================================================================
// Testes de aquisição e inspeção básica
// ==========================================================================

#[test]
fn capability_kind_returns_correct_string() {
    let mut interp = run_and_interpret(
        r#"
        let cap = capability_acquire("NetBind")
        let k = capability_kind(cap)
    "#,
    );

    let k_val = interp.debug_get_global("k").expect("k global");
    assert_eq!(
        k_val,
        ArtValue::String("NetBind".into()),
        "capability_kind deve retornar 'NetBind'"
    );

    let diags = interp.take_diagnostics();
    assert!(
        diags.is_empty(),
        "unexpected diagnostics: {:?}",
        diags.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn capability_acquire_produces_capability_value() {
    let mut interp = run_and_interpret(
        r#"
        let cap = capability_acquire("FileSystem")
    "#,
    );

    let cap_val = interp.debug_get_global("cap").expect("cap global");
    assert!(
        matches!(cap_val, ArtValue::Capability { .. }),
        "deve produzir ArtValue::Capability, got {:?}",
        cap_val
    );
    assert!(interp.take_diagnostics().is_empty());
}

#[test]
fn capability_type_of_returns_capability_string() {
    let interp = run_and_interpret(
        r#"
        let cap = capability_acquire("FileSystem")
        let t = type_of(cap)
    "#,
    );

    let t_val = interp.debug_get_global("t").expect("t global");
    assert_eq!(
        t_val,
        ArtValue::String("Capability".into()),
        "type_of deve retornar 'Capability'"
    );
}

#[test]
fn capability_arbitrary_kind() {
    let interp = run_and_interpret(
        r#"
        let cap = capability_acquire("CustomCap")
        let k = capability_kind(cap)
    "#,
    );

    assert_eq!(
        interp.debug_get_global("k"),
        Some(ArtValue::String("CustomCap".into()))
    );
}

// ==========================================================================
// Testes de move-semantics no runtime
// ==========================================================================

#[test]
fn capability_double_use_emits_moved_diagnostic() {
    // Após ser consumida (let cap2 = cap), cap fica como MovedCapability.
    // A segunda leitura (let cap3 = cap) deve emitir diagnóstico de runtime.
    let mut interp = run_and_interpret(
        r#"
        let cap = capability_acquire("NetBind")
        let cap2 = cap
        let cap3 = cap
    "#,
    );

    let diags = interp.take_diagnostics();
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("already moved") || d.message.contains("cannot be reused")),
        "deve detectar reuso de capability movida, diagnostics: {:?}",
        diags.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn capability_consumed_in_function_arg_becomes_moved() {
    let mut interp = run_and_interpret(
        r#"
        func consume(cap) {
            let _k = capability_kind(cap)
        }
        let c = capability_acquire("NetBind")
        consume(c)
        let k2 = capability_kind(c)
    "#,
    );

    let diags = interp.take_diagnostics();
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("already moved") || d.message.contains("cannot be reused")),
        "reuso pós-função deve emitir diagnóstico, diagnostics: {:?}",
        diags.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
    );
}

// ==========================================================================
// Testes de capabilities independentes
// ==========================================================================

#[test]
fn multiple_capabilities_are_independent() {
    let mut interp = run_and_interpret(
        r#"
        let net = capability_acquire("NetBind")
        let fs  = capability_acquire("FileSystem")
        let kn = capability_kind(net)
        let kf = capability_kind(fs)
    "#,
    );

    assert_eq!(
        interp.debug_get_global("kn"),
        Some(ArtValue::String("NetBind".into()))
    );
    assert_eq!(
        interp.debug_get_global("kf"),
        Some(ArtValue::String("FileSystem".into()))
    );

    assert!(
        interp.take_diagnostics().is_empty(),
        "capabilities independentes não devem gerar diagnosticos"
    );
}

// ==========================================================================
// Testes do type checker (TypeInfer)
// ==========================================================================

#[test]
fn type_infer_reports_capability_reuse() {
    let src = r#"
        let cap = capability_acquire("NetBind")
        let cap2 = cap
        let cap3 = cap
    "#;

    let program = parse_program(src);
    let mut tenv = TypeEnv::new();
    // run() retorna Err(diags) quando ha erros de tipo
    let result = TypeInfer::new(&mut tenv).run(&program);

    let diags = match result {
        Ok(()) => vec![],
        Err(d) => d,
    };

    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("already moved") || d.message.contains("cannot be reused")),
        "type checker deve reportar reuso de capability, got: {:?}",
        diags.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
    );
}
