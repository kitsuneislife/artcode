use interpreter::type_infer::{TypeEnv, TypeInfer};
use lexer::Lexer;
use parser::Parser;

fn infer_prog(src: &str) -> (TypeEnv, Vec<diagnostics::Diagnostic>) {
    let mut lx = Lexer::new(src.to_string());
    let tokens = lx.scan_tokens().unwrap();
    let mut p = Parser::new(tokens);
    let (program, pdiags) = p.parse();
    assert!(pdiags.is_empty(), "parse diagnostics: {:?}", pdiags);
    let mut tenv = TypeEnv::new();
    let mut inf = TypeInfer::new(&mut tenv);
    let _ = inf.run(&program); // collect diags via inf.diags for assertions
    let diags = inf.diags;
    (tenv, diags)
}

#[test]
fn enum_variant_arity_mismatch_type_phase() {
    let (_tenv, diags) = infer_prog("enum E { P(Int, Int) } let x = E.P(1); ");
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("expects 2 arguments")),
        "expected arity diagnostic, got: {:?}",
        diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

#[test]
fn enum_variant_ok_and_result_monomorph() {
    let (tenv, _diags) = infer_prog("enum E { P(Int) } let r = E.P(1); let res = .P(1);");
    assert!(tenv.get_var("r").is_some());
    assert!(tenv.get_var("res").is_some());
}
