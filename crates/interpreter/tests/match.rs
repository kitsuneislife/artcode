use interpreter::interpreter::Interpreter;
use lexer::lexer::Lexer;
use parser::parser::Parser;

fn run(
    src: &str,
) -> (
    Result<(), interpreter::values::RuntimeError>,
    Vec<diagnostics::Diagnostic>,
) {
    let mut lx = Lexer::new(src.to_string());
    let tokens = match lx.scan_tokens() {
        Ok(t) => t,
        Err(e) => {
            assert!(false, "lexer scan_tokens in match.rs failed: {:?}", e);
            Vec::new()
        }
    };
    let mut p = Parser::new(tokens);
    let (program, pdiags) = p.parse();
    if !pdiags.is_empty() {
        return (Ok(()), pdiags);
    }
    let mut interp = Interpreter::with_prelude();
    let res = interp.interpret(program);
    (res, interp.take_diagnostics())
}

#[test]
fn match_enum_multi_params() {
    let (res, diags) =
        run("enum E { P(Int, Int) } let v=E.P(1,2); match v { case .P(a,b): println(a + b) }");
    assert!(res.is_ok(), "runtime error: {:?}", res);
    assert!(diags.is_empty());
}

#[test]
fn match_enum_wrong_arity() {
    let (res, diags) =
        run("enum E { P(Int, Int) } let v=E.P(1,2); match v { case .P(a): println(a) }");
    assert!(res.is_ok(), "runtime error: {:?}", res);
    assert!(
        diags.iter().any(|d| d.message.contains("Arity mismatch")),
        "expected arity mismatch diagnostic, got: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

#[test]
fn match_binding_and_wildcard() {
    let (res, diags) =
        run("enum E { P(Int, Int) } let v=E.P(7,8); match v { case .P(let a, _): println(a) }");
    assert!(res.is_ok(), "runtime error: {:?}", res);
    assert!(diags.is_empty());
}
