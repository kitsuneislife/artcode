use ir::{Function, Instr, Type};

#[test]
fn golden_add() {
    let f = Function {
        name: "add".to_string(),
        params: vec![("a".to_string(), Type::I64), ("b".to_string(), Type::I64)],
        ret: Some(Type::I64),
        body: vec![
            Instr::Add("%0".to_string(), "a".to_string(), "b".to_string()),
            Instr::Ret(Some("%0".to_string())),
        ],
    };
    let text = f.emit_text();
    let expected = "func @add(i64 a, i64 b) -> i64 {\n  entry:\n  %0 = add i64 a, b\n  ret %0\n}\n";
    assert_eq!(text, expected);
}
