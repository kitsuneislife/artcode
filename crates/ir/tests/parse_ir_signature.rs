//! Coverage for `ir::parse_ir_signature`, carried over from the former `jit`
//! crate when the function moved here.

use ir::parse_ir_signature;

#[test]
fn reads_parameter_count_and_return_type() {
    let (params, ret) = parse_ir_signature("func @sum(i64, i64) -> i64 { entry: ret }")
        .expect("signature should parse");
    assert_eq!(params, 2);
    assert_eq!(ret, "i64");
}

#[test]
fn treats_an_empty_parameter_list_as_zero() {
    let (params, ret) =
        parse_ir_signature("func @f() -> i64 { entry: ret }").expect("signature should parse");
    assert_eq!(params, 0);
    assert_eq!(ret, "i64");
}

#[test]
fn rejects_malformed_signatures() {
    // Missing the `@` sigil before the name.
    assert!(parse_ir_signature("func f() { entry: ret }").is_err());
    // Unbalanced parameter list.
    assert!(parse_ir_signature("func @g(i64 -> i64 {").is_err());
    // No return type.
    assert!(parse_ir_signature("func @h(i64) { entry: ret }").is_err());
    // Empty function name.
    assert!(parse_ir_signature("func @() -> i64 {").is_err());
}
