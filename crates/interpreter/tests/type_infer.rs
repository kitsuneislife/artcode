use interpreter::type_infer::{TypeEnv, TypeInfer};
use lexer::Lexer;
use parser::Parser;

fn infer(src: &str) -> TypeEnv {
    let mut lx = Lexer::new(src.to_string());
    let tokens = lx.scan_tokens().unwrap();
    let mut p = Parser::new(tokens);
    let (program, diags) = p.parse();
    assert!(diags.is_empty(), "parse diagnostics: {:?}", diags);
    let mut tenv = TypeEnv::new();
    assert!(
        TypeInfer::new(&mut tenv).run(&program).is_ok(),
        "type infer failed: {:?}",
        TypeInfer::new(&mut tenv).diags
    );
    tenv
}

#[test]
fn infer_numeric_promotion() {
    let _tenv = infer("let a=1 + 2.0;");
    // Just ensure no crash; detailed lookup not yet exposed publicly.
    // Future: expose query API to map variable names to types.
}

#[test]
fn infer_interpolated_string_type() {
    let _tenv = infer("let s = f\"ok={1+2}\";");
}
