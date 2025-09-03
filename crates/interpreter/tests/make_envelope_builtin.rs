use core::ast::{ArtValue, Expr, Stmt};
use interpreter::interpreter::Interpreter;

#[test]
fn make_envelope_auto_fills_sender_and_heapifies() {
    let mut interp = Interpreter::with_prelude();
    // spawn a dummy actor to get an id
    interp
        .interpret(vec![Stmt::SpawnActor { body: vec![] }])
        .unwrap();
    let aid = match interp.last_value.clone().unwrap() {
        core::ast::ArtValue::Actor(id) => id,
        core::ast::ArtValue::Int(n) => n as u32,
        _ => panic!(),
    };
    // set current_actor to aid
    interp.current_actor = Some(aid);
    // call make_envelope(7, 2)
    let call = Stmt::Expression(Expr::Call {
        callee: Box::new(Expr::Variable {
            name: core::Token::dummy("make_envelope"),
        }),
        arguments: vec![
            Expr::Literal(ArtValue::Int(7)),
            Expr::Literal(ArtValue::Int(2)),
        ],
    });
    interp.interpret(vec![call]).unwrap();
    let last = interp.last_value.clone().expect("expected last value");
    match last {
        core::ast::ArtValue::HeapComposite(h) => {
            let resolved = interp.debug_heap_upgrade_weak(h.0).expect("heap entry");
            if let core::ast::ArtValue::StructInstance {
                struct_name,
                fields,
            } = resolved
            {
                assert_eq!(struct_name, "Envelope");
                match fields.get("sender").unwrap() {
                    core::ast::ArtValue::Int(n) => assert_eq!(*n, aid as i64),
                    other => panic!("unexpected sender field: {:?}", other),
                }
                match fields.get("payload").unwrap() {
                    core::ast::ArtValue::Int(n) => assert_eq!(*n, 7),
                    other => panic!("unexpected payload: {:?}", other),
                }
                match fields.get("priority").unwrap() {
                    core::ast::ArtValue::Int(n) => assert_eq!(*n, 2),
                    other => panic!("unexpected priority: {:?}", other),
                }
            } else {
                panic!("expected StructInstance in heap");
            }
        }
        other => panic!("expected HeapComposite, got {:?}", other),
    }
}
