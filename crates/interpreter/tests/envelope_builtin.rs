use interpreter::interpreter::Interpreter;
use core::ast::{Expr, Stmt, ArtValue};

#[test]
fn envelope_builtin_constructs_and_heapifies() {
    let mut interp = Interpreter::with_prelude();
    // call envelope(None, 42, 5)
    let call = Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable { name: core::Token::dummy("envelope") }),
        arguments: vec![
            // sender: None
            Expr::Literal(ArtValue::Optional(Box::new(None))),
            // payload
            Expr::Literal(ArtValue::Int(42)),
            // priority
            Expr::Literal(ArtValue::Int(5)),
        ],
    });
    interp.interpret(vec![call]).unwrap();
    let last = interp.last_value.clone().expect("expected last value");
    // Expect a heapified composite (HeapComposite)
    match last {
        core::ast::ArtValue::HeapComposite(h) => {
            // retrieve underlying value from heap
            let resolved = interp.debug_heap_upgrade_weak(h.0).expect("heap entry");
            if let core::ast::ArtValue::StructInstance { struct_name, fields } = resolved {
                assert_eq!(struct_name, "Envelope");
                // sender should be Optional(None)
                match fields.get("sender").unwrap() {
                    core::ast::ArtValue::Optional(boxed) => assert!(boxed.is_none()),
                    other => panic!("unexpected sender field: {:?}", other),
                }
                // payload == 42
                match fields.get("payload").unwrap() {
                    core::ast::ArtValue::Int(n) => assert_eq!(*n, 42),
                    other => panic!("unexpected payload: {:?}", other),
                }
                // priority == 5
                match fields.get("priority").unwrap() {
                    core::ast::ArtValue::Int(n) => assert_eq!(*n, 5),
                    other => panic!("unexpected priority: {:?}", other),
                }
            } else {
                panic!("expected StructInstance in heap");
            }
        }
        other => panic!("expected HeapComposite, got {:?}", other),
    }
}
