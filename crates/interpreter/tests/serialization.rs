use core::ast::ArtValue;
use interpreter::Interpreter;
use lexer::Lexer;
use parser::Parser;

fn run_and_interpret(src: &str) -> Interpreter {
    let mut lexer = Lexer::new(src.to_string());
    let tokens = lexer.scan_tokens().expect("tokens");
    let mut parser = Parser::new(tokens);
    let (program, diags) = parser.parse();
    assert!(
        diags.is_empty(),
        "unexpected parser diagnostics: {:?}",
        diags.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
    );
    let mut interp = Interpreter::with_prelude();
    interp.interpret(program).expect("interpret");
    interp
}

#[test]
fn test_serialize_primitives() {
    let code = r#"
        let n = 42
        let buf = serialize(n)
        let n2 = deserialize(buf)

        let s = "hello"
        let buf_s = serialize(s)
        let s2 = deserialize(buf_s)

        let b = true
        let buf_b = serialize(b)
        let b2 = deserialize(buf_b)
    "#;
    let inter = run_and_interpret(code);
    assert_eq!(inter.debug_get_global("n2"), Some(ArtValue::Int(42)));
    assert_eq!(
        inter.debug_get_global("s2"),
        Some(ArtValue::String("hello".into()))
    );
    assert_eq!(inter.debug_get_global("b2"), Some(ArtValue::Bool(true)));
}

#[test]
fn test_serialize_complex() {
    let code = r#"
        let arr = [1, 2, 3]
        let s_arr = serialize(arr)
        let o_arr = deserialize(s_arr)

        let map = map_new()
        map_set(map, "key", 99)
        let s_map = serialize(map)
        let o_map = deserialize(s_map)
    "#;
    let inter = run_and_interpret(code);

    let arr_val_ref = inter.debug_get_global("o_arr").unwrap();
    let arr_val = inter.resolve_composite(&arr_val_ref).clone();
    if let ArtValue::Array(items) = arr_val {
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], ArtValue::Int(1));
    } else {
        panic!("Expected Array");
    }

    let map_val_ref = inter.debug_get_global("o_map").unwrap();
    let map_val = inter.resolve_composite(&map_val_ref).clone();
    if let ArtValue::Map(map_ref) = map_val {
        let m = map_ref.0.lock().unwrap();
        assert_eq!(m.get("key"), Some(&ArtValue::Int(99)));
    } else {
        panic!("Expected Map");
    }
}

#[test]
fn test_serialize_opaque_types_fails() {
    let code = r#"
        let cap = capability_acquire("NetBind")
        let result = serialize(cap)
    "#;
    let mut inter = run_and_interpret(code);
    let diags = inter.take_diagnostics();
    assert!(!diags.is_empty());
    assert!(
        diags[0]
            .message
            .contains("Cannot serialize type Capability")
    );
    assert_eq!(inter.debug_get_global("result"), Some(ArtValue::none()));
}
