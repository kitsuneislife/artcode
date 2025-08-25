use interpreter::interpreter::Interpreter;
use core::ast::{Stmt, Expr};

#[test]
fn actor_priority_and_backpressure_stress() {
    let mut interp = Interpreter::with_prelude();

    // spawn actor
    interp.interpret(vec![Stmt::SpawnActor { body: vec![] }]).unwrap();
    let aid = match interp.last_value.clone().unwrap() {
        core::ast::ArtValue::Actor(id) => id,
        core::ast::ArtValue::Int(n) => n as u32,
        _ => panic!(),
    };

    // send many messages with varying priorities
    for i in 0..200 {
        let pri = (i % 5) as i64; // priorities 0..4
        interp.interpret(vec![Stmt::Expression(Expr::Call { callee: Box::new(Expr::Variable { name: core::Token::dummy("actor_send") }), arguments: vec![Expr::Literal(core::ast::ArtValue::Int(aid as i64)), Expr::Literal(core::ast::ArtValue::Int(i as i64)), Expr::Literal(core::ast::ArtValue::Int(pri))] })]).unwrap();
    }

    // mailbox should contain 200 items and highest priority items should come first
    let mailbox = &interp.actors.get(&aid).unwrap().mailbox;
    assert_eq!(mailbox.len(), 200);
    // check first few priorities are highest (4)
    let front = mailbox.front().unwrap();
    if let core::ast::ArtValue::Int(_) = &front.payload {
        assert!(front.priority == 4);
    }

    // set small mailbox limit and verify backpressure
    interp.interpret(vec![Stmt::SpawnActor { body: vec![] }]).unwrap();
    let aid2 = match interp.last_value.clone().unwrap() {
        core::ast::ArtValue::Actor(id) => id,
        core::ast::ArtValue::Int(n) => n as u32,
        _ => panic!(),
    };
    interp.interpret(vec![Stmt::Expression(Expr::Call { callee: Box::new(Expr::Variable { name: core::Token::dummy("actor_set_mailbox_limit") }), arguments: vec![Expr::Literal(core::ast::ArtValue::Int(aid2 as i64)), Expr::Literal(core::ast::ArtValue::Int(3))] })]).unwrap();
    // send 3 messages
    for i in 0..3 {
        interp.interpret(vec![Stmt::Expression(Expr::Call { callee: Box::new(Expr::Variable { name: core::Token::dummy("actor_send") }), arguments: vec![Expr::Literal(core::ast::ArtValue::Int(aid2 as i64)), Expr::Literal(core::ast::ArtValue::Int(i as i64))] })]).unwrap();
    }
    // fourth should fail (backpressure)
    interp.interpret(vec![Stmt::Expression(Expr::Call { callee: Box::new(Expr::Variable { name: core::Token::dummy("actor_send") }), arguments: vec![Expr::Literal(core::ast::ArtValue::Int(aid2 as i64)), Expr::Literal(core::ast::ArtValue::Int(99))] })]).unwrap();
    let res = interp.last_value.clone().unwrap();
    assert!(matches!(res, core::ast::ArtValue::Bool(false)));
}
