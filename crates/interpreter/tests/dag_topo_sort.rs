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
fn dag_topo_sort_returns_valid_order_for_acyclic_graph() {
    let src = r#"
        let nodes = ["kernel", "fs", "shell", "ui"];
        let deps = [("fs", "kernel"), ("shell", "fs"), ("ui", "shell")];
        let order = dag_topo_sort(nodes, deps);
    "#;

    let program = parse_program(src);
    let mut interp = Interpreter::with_prelude();
    interp.interpret(program).expect("interpret");

    let diags = interp.take_diagnostics();
    assert!(
        diags.is_empty(),
        "unexpected runtime diagnostics: {:?}",
        diags.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
    );

    let order_val = interp.debug_get_global("order").expect("global order");
    let resolved = interp.resolve_composite(&order_val).clone();
    let ArtValue::Array(items) = resolved else {
        panic!("expected array order, got {:?}", resolved);
    };

    let names: Vec<String> = items
        .into_iter()
        .map(|v| match v {
            ArtValue::String(s) => s.to_string(),
            other => panic!("expected string in order, got {:?}", other),
        })
        .collect();

    let pos = |name: &str| -> usize {
        names
            .iter()
            .position(|n| n == name)
            .unwrap_or(usize::MAX)
    };

    assert!(pos("kernel") < pos("fs"));
    assert!(pos("fs") < pos("shell"));
    assert!(pos("shell") < pos("ui"));
}

#[test]
fn dag_topo_sort_reports_cycle() {
    let src = r#"
        let nodes = ["a", "b"];
        let deps = [("a", "b"), ("b", "a")];
        let order = dag_topo_sort(nodes, deps);
    "#;

    let program = parse_program(src);
    let mut interp = Interpreter::with_prelude();
    interp.interpret(program).expect("interpret");

    let diags = interp.take_diagnostics();
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("cycle detected")),
        "expected cycle diagnostic, got {:?}",
        diags.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
    );

    let order_val = interp.debug_get_global("order").expect("global order");
    assert_eq!(order_val, ArtValue::none());
}
