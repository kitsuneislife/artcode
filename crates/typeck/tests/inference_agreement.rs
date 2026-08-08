//! Pins down what the crate's two inference passes must agree on.
//!
//! `TypeChecker::infer_expr` and `TypeInfer::infer_expr` walk the same AST and
//! produce the same `core::types::Type`, but they were written independently.
//! `art run` reaches one and `art check` the other, so a disagreement means the
//! same expression is understood differently depending on the command.
//!
//! Each test states the type the language assigns, so these double as the
//! specification for collapsing the two passes onto one.

use core::types::Type;
use lexer::Lexer;
use parser::Parser;
use typeck::TypeChecker;
use typeck::type_infer::{TypeEnv, TypeInfer};

fn parse(src: &str) -> Vec<core::ast::Stmt> {
    let mut lx = Lexer::new(src.to_string());
    let tokens = lx.scan_tokens().expect("lex");
    let (program, parse_diags) = Parser::new(tokens).parse();
    assert!(parse_diags.is_empty(), "parse errors: {:?}", parse_diags);
    program
}

/// Type that `TypeInfer` assigns to each named binding.
fn inferred_types(src: &str, names: &[&str]) -> Vec<Option<Type>> {
    let program = parse(src);
    let mut tenv = TypeEnv::new();
    let mut inf = TypeInfer::new(&mut tenv);
    let _ = inf.run(&program);
    names.iter().map(|n| tenv.get_var(n).cloned()).collect()
}

/// Diagnostic messages produced by each pass over the same source.
fn diagnostics(src: &str) -> (Vec<String>, Vec<String>) {
    let program = parse(src);

    let from_check: Vec<String> = TypeChecker::new()
        .check(&program)
        .iter()
        .map(|d| d.message.clone())
        .collect();

    let mut tenv = TypeEnv::new();
    let mut inf = TypeInfer::new(&mut tenv);
    let from_infer = match inf.run(&program) {
        Ok(()) => Vec::new(),
        Err(diags) => diags.into_iter().map(|d| d.message).collect(),
    };

    (from_check, from_infer)
}

#[test]
fn comparison_operators_produce_bool() {
    // `TypeInfer` matched on operand types alone and ignored the operator, so
    // every comparison over two Ints was typed `Int`.
    let types = inferred_types(
        "let a = 1; let b = 2; let lt = a < b; let eq = a == b; let ge = a >= b;",
        &["lt", "eq", "ge"],
    );

    for (name, ty) in ["lt", "eq", "ge"].iter().zip(&types) {
        assert_eq!(
            ty.as_ref(),
            Some(&Type::Bool),
            "`{}` is a comparison and must be Bool, got {:?}",
            name,
            ty
        );
    }
}

#[test]
fn struct_initialisation_is_typed_as_the_struct() {
    // `TypeInfer` returned `Unknown` here, so any rule downstream of it was
    // reasoning about a type it did not have.
    let types = inferred_types(
        "struct Point { x: Int, y: Int } let p = Point { x: 1, y: 2 };",
        &["p"],
    );

    assert_eq!(
        types[0].as_ref(),
        Some(&Type::Struct("Point".to_string())),
        "struct initialisation must be typed as its struct, got {:?}",
        types[0]
    );
}

#[test]
fn both_passes_agree_on_string_concatenation() {
    // `TypeChecker` widens `+` to String when either side is a String;
    // `TypeInfer` rejected the same expression outright. Whichever rule the
    // language settles on, one command must not accept what the other refuses.
    let (from_check, from_infer) = diagnostics(r#"let label = "n=" + 1;"#);

    assert_eq!(
        from_check.is_empty(),
        from_infer.is_empty(),
        "passes disagree on `\"n=\" + 1`: TypeChecker={:?}, TypeInfer={:?}",
        from_check,
        from_infer
    );
}
