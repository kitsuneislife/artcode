use core::ast::ArtValue;
use interpreter::Interpreter;
use lexer::Lexer;
use parser::Parser;

fn run_get(src: &str, var: &str) -> ArtValue {
    let mut lexer = Lexer::new(src.to_string());
    let tokens = lexer.scan_tokens().expect("lex");
    let mut parser = Parser::new(tokens);
    let (program, diags) = parser.parse();
    assert!(diags.is_empty(), "parse errors: {:?}", diags);
    let mut interp = Interpreter::with_prelude();
    interp.interpret(program).expect("interpret");
    assert!(
        interp.diagnostics.is_empty(),
        "runtime diagnostics: {:?}",
        interp.diagnostics
    );
    let val = interp.debug_get_global(var).expect("global not found");
    interp.resolve_composite(&val).clone()
}

fn run_diags(src: &str) -> Vec<String> {
    let mut lexer = Lexer::new(src.to_string());
    let tokens = lexer.scan_tokens().expect("lex");
    let mut parser = Parser::new(tokens);
    let (program, _) = parser.parse();
    let mut interp = Interpreter::with_prelude();
    let _ = interp.interpret(program);
    interp
        .take_diagnostics()
        .into_iter()
        .map(|d| d.message)
        .collect()
}

fn arc(s: &str) -> ArtValue {
    ArtValue::String(std::sync::Arc::from(s))
}

// ── str_split ────────────────────────────────────────────────────────────────

#[test]
fn str_split_basic() {
    let v = run_get(r#"let x = str_split("a,b,c", ",");"#, "x");
    let ArtValue::Array(parts) = v else {
        panic!("expected Array, got {:?}", v)
    };
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], arc("a"));
    assert_eq!(parts[1], arc("b"));
    assert_eq!(parts[2], arc("c"));
}

#[test]
fn str_split_no_separator_found() {
    let v = run_get(r#"let x = str_split("hello", ",");"#, "x");
    let ArtValue::Array(parts) = v else {
        panic!("expected Array, got {:?}", v)
    };
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0], arc("hello"));
}

#[test]
fn str_split_empty_string() {
    let v = run_get(r#"let x = str_split("", ",");"#, "x");
    let ArtValue::Array(parts) = v else {
        panic!("expected Array, got {:?}", v)
    };
    assert_eq!(parts.len(), 1);
}

// ── str_join ─────────────────────────────────────────────────────────────────

#[test]
fn str_join_basic() {
    let v = run_get(r#"let x = str_join(["a", "b", "c"], "-");"#, "x");
    assert_eq!(v, arc("a-b-c"));
}

#[test]
fn str_join_single_element() {
    let v = run_get(r#"let x = str_join(["hello"], ",");"#, "x");
    assert_eq!(v, arc("hello"));
}

#[test]
fn str_join_empty_sep() {
    let v = run_get(r#"let x = str_join(["a", "b", "c"], "");"#, "x");
    assert_eq!(v, arc("abc"));
}

// ── str_contains ─────────────────────────────────────────────────────────────

#[test]
fn str_contains_found() {
    let v = run_get(r#"let x = str_contains("hello world", "world");"#, "x");
    assert_eq!(v, ArtValue::Bool(true));
}

#[test]
fn str_contains_not_found() {
    let v = run_get(r#"let x = str_contains("hello world", "xyz");"#, "x");
    assert_eq!(v, ArtValue::Bool(false));
}

#[test]
fn str_contains_empty_sub() {
    let v = run_get(r#"let x = str_contains("hello", "");"#, "x");
    assert_eq!(v, ArtValue::Bool(true));
}

// ── str_starts_with ──────────────────────────────────────────────────────────

#[test]
fn str_starts_with_true() {
    let v = run_get(r#"let x = str_starts_with("artcode", "art");"#, "x");
    assert_eq!(v, ArtValue::Bool(true));
}

#[test]
fn str_starts_with_false() {
    let v = run_get(r#"let x = str_starts_with("artcode", "code");"#, "x");
    assert_eq!(v, ArtValue::Bool(false));
}

// ── str_replace ──────────────────────────────────────────────────────────────

#[test]
fn str_replace_basic() {
    let v = run_get(
        r#"let x = str_replace("hello world", "world", "artcode");"#,
        "x",
    );
    assert_eq!(v, arc("hello artcode"));
}

#[test]
fn str_replace_all_occurrences() {
    let v = run_get(r#"let x = str_replace("a-a-a", "a", "b");"#, "x");
    assert_eq!(v, arc("b-b-b"));
}

#[test]
fn str_replace_not_found() {
    let v = run_get(r#"let x = str_replace("hello", "xyz", "abc");"#, "x");
    assert_eq!(v, arc("hello"));
}

// ── str_slice ─────────────────────────────────────────────────────────────────

#[test]
fn str_slice_basic() {
    let v = run_get(r#"let x = str_slice("artcode", 0, 3);"#, "x");
    assert_eq!(v, arc("art"));
}

#[test]
fn str_slice_mid() {
    let v = run_get(r#"let x = str_slice("artcode", 3, 7);"#, "x");
    assert_eq!(v, arc("code"));
}

#[test]
fn str_slice_clamped_end() {
    let v = run_get(r#"let x = str_slice("art", 0, 100);"#, "x");
    assert_eq!(v, arc("art"));
}

#[test]
fn str_slice_negative_indices() {
    let v = run_get(r#"let x = str_slice("artcode", -4, -1);"#, "x");
    assert_eq!(v, arc("cod"));
}

// ── str_to_int ───────────────────────────────────────────────────────────────

#[test]
fn str_to_int_ok() {
    let v = run_get(r#"let x = str_to_int("42");"#, "x");
    let ArtValue::EnumInstance {
        variant, values, ..
    } = v
    else {
        panic!("expected EnumInstance, got {:?}", v)
    };
    assert_eq!(variant, "Ok");
    assert_eq!(values[0], ArtValue::Int(42));
}

#[test]
fn str_to_int_negative() {
    let v = run_get(r#"let x = str_to_int("-7");"#, "x");
    let ArtValue::EnumInstance {
        variant, values, ..
    } = v
    else {
        panic!("expected EnumInstance, got {:?}", v)
    };
    assert_eq!(variant, "Ok");
    assert_eq!(values[0], ArtValue::Int(-7));
}

#[test]
fn str_to_int_err() {
    let v = run_get(r#"let x = str_to_int("hello");"#, "x");
    let ArtValue::EnumInstance { variant, .. } = v else {
        panic!("expected EnumInstance, got {:?}", v)
    };
    assert_eq!(variant, "Err");
}

// ── str_to_float ─────────────────────────────────────────────────────────────

#[test]
fn str_to_float_ok() {
    let v = run_get(r#"let x = str_to_float("3.45");"#, "x");
    let ArtValue::EnumInstance {
        variant, values, ..
    } = v
    else {
        panic!("expected EnumInstance, got {:?}", v)
    };
    assert_eq!(variant, "Ok");
    let ArtValue::Float(f) = values[0] else {
        panic!("expected Float")
    };
    assert!((f - 3.45).abs() < 1e-9);
}

#[test]
fn str_to_float_err() {
    let v = run_get(r#"let x = str_to_float("abc");"#, "x");
    let ArtValue::EnumInstance { variant, .. } = v else {
        panic!("expected EnumInstance, got {:?}", v)
    };
    assert_eq!(variant, "Err");
}

// ── type error diagnostics ───────────────────────────────────────────────────

#[test]
fn str_split_type_error_emits_diagnostic() {
    let diags = run_diags(r#"str_split(42, ",");"#);
    assert!(!diags.is_empty(), "expected diagnostic for wrong type");
    assert!(diags[0].contains("str_split"));
}

#[test]
fn str_join_type_error_emits_diagnostic() {
    let diags = run_diags(r#"str_join("not-an-array", ",");"#);
    assert!(!diags.is_empty(), "expected diagnostic for wrong type");
    assert!(diags[0].contains("str_join"));
}
