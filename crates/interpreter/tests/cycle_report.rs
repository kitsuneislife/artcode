use interpreter::interpreter::Interpreter;

#[test]
fn cycle_report_counts_basic_refs() {
    let mut interp = Interpreter::with_prelude();
    // criar valores e weak/unowned via builtins açúcar
    let src = "let a = 1; let w1 = weak a; let w2 = weak a; let u = unowned(a); w1?;";
    // interpret manualmente (reutilizando pipeline simplificado)
    {
        let mut lexer = lexer::lexer::Lexer::new(src.to_string());
        let tokens = lexer.scan_tokens().unwrap();
        let mut parser = parser::parser::Parser::new(tokens);
        let (program, diags) = parser.parse();
        assert!(diags.is_empty(), "parse diags: {:?}", diags);
        interp.interpret(program).unwrap();
    }
    let rep = interp.cycle_report();
    assert_eq!(rep.weak_total, 2);
    assert_eq!(rep.unowned_total, 1);
    assert_eq!(rep.weak_dead, 0);
}
