use lexer::lexer::Lexer; use parser::parser::Parser; use interpreter::interpreter::Interpreter; use diagnostics::DiagnosticKind;

#[test]
fn weak_dies_after_scope() {
    // Estratégia revisada: como não existe atribuição pós-declaração, usamos uma função que cria o weak e o retorna.
    // O objeto forte vive apenas dentro da função; ao retornar, o weak deve já estar dangling.
    let src = r#"
    func make_w() {
        let a = [1,2];
        return weak(a);
    }
    let outside = make_w();
    // tentar upgrade (deve produzir Optional(None))
    let res = outside?;
    "#;
    let mut lx=Lexer::new(src.to_string()); let tokens=lx.scan_tokens().unwrap(); let mut p=Parser::new(tokens); let (program, pdiags)=p.parse(); assert!(pdiags.is_empty());
    let mut interp=Interpreter::with_prelude(); interp.interpret(program).unwrap();
    // Procurar diagnostics de runtime (não deve haver) e verificar que res é None (Optional(None))
    let diags=interp.take_diagnostics(); assert!(diags.iter().all(|d| d.kind!=DiagnosticKind::Runtime), "runtime diags: {:?}", diags);
    if let Some(val) = interp.debug_get_global("res") {
        match val {
            core::ast::ArtValue::Optional(b) => assert!(b.is_none(), "expected None from weak upgrade, got {:?}", b),
            other => panic!("res not optional: {:?}", other)
        }
    } else { panic!("res not found"); }
}
