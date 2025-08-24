use interpreter::interpreter::Interpreter;
use core::ast::{Stmt, Expr};

#[test]
fn actor_mailbox_fifo_and_backpressure_and_scheduler() {
    let mut interp = Interpreter::with_prelude();

    // 1) FIFO mailbox: spawn actor and send two messages
    let spawn_stmt = Stmt::SpawnActor { body: vec![] };
    interp.interpret(vec![spawn_stmt]).unwrap();
    let actor_id = match interp.last_value.clone().unwrap() {
        core::ast::ArtValue::Int(n) => n as u32,
        _ => panic!("unexpected actor id value"),
    };
    // send 1 and 2
    let call1 = Stmt::Expression(Expr::Call { callee: Box::new(Expr::Variable { name: core::Token::dummy("actor_send") }), arguments: vec![Expr::Literal(core::ast::ArtValue::Int(actor_id as i64)), Expr::Literal(core::ast::ArtValue::Int(1))] });
    interp.interpret(vec![call1]).unwrap();
    // because call_builtin returns ArtValue, ensure Bool(true) or none; use ActorSend wrapper by calling via evaluate above
    // second send
    let call2 = Stmt::Expression(Expr::Call { callee: Box::new(Expr::Variable { name: core::Token::dummy("actor_send") }), arguments: vec![Expr::Literal(core::ast::ArtValue::Int(actor_id as i64)), Expr::Literal(core::ast::ArtValue::Int(2))] });
    interp.interpret(vec![call2]).unwrap();
    // inspect mailbox order
    let mailbox_vals: Vec<i64> = interp
        .actors
        .get(&actor_id)
        .unwrap()
        .mailbox
        .iter()
        .iter()
        .filter_map(|env| if let core::ast::ArtValue::Int(n) = &env.payload { Some(*n) } else { None })
        .collect();
    assert_eq!(mailbox_vals, vec![1, 2]);

    // sender propagation: send from a 'current_actor' context
    // Spawn actor A and B; set current_actor artificially and send
    interp.interpret(vec![Stmt::SpawnActor { body: vec![] }]).unwrap();
    let aid_sender = match interp.last_value.clone().unwrap() { core::ast::ArtValue::Int(n) => n as u32, _ => panic!() };
    interp.interpret(vec![Stmt::SpawnActor { body: vec![] }]).unwrap();
    let aid_target = match interp.last_value.clone().unwrap() { core::ast::ArtValue::Int(n) => n as u32, _ => panic!() };
    // simulate running as sender by setting current_actor and using actor_send
    interp.current_actor = Some(aid_sender);
    interp.interpret(vec![Stmt::Expression(Expr::Call { callee: Box::new(Expr::Variable { name: core::Token::dummy("actor_send") }), arguments: vec![Expr::Literal(core::ast::ArtValue::Int(aid_target as i64)), Expr::Literal(core::ast::ArtValue::Int(99))] })]).unwrap();
    interp.current_actor = None;
    let mailbox = &interp.actors.get(&aid_target).unwrap().mailbox;
    assert_eq!(mailbox.front().unwrap().sender, Some(aid_sender));

    // priority ordering: send low priority then high priority
    interp.interpret(vec![Stmt::SpawnActor { body: vec![] }]).unwrap();
    let aid_pri = match interp.last_value.clone().unwrap() { core::ast::ArtValue::Int(n) => n as u32, _ => panic!() };
    // send priority 0 then priority 10
    interp.interpret(vec![Stmt::Expression(Expr::Call { callee: Box::new(Expr::Variable { name: core::Token::dummy("actor_send") }), arguments: vec![Expr::Literal(core::ast::ArtValue::Int(aid_pri as i64)), Expr::Literal(core::ast::ArtValue::Int(1)), Expr::Literal(core::ast::ArtValue::Int(0))] })]).unwrap();
    interp.interpret(vec![Stmt::Expression(Expr::Call { callee: Box::new(Expr::Variable { name: core::Token::dummy("actor_send") }), arguments: vec![Expr::Literal(core::ast::ArtValue::Int(aid_pri as i64)), Expr::Literal(core::ast::ArtValue::Int(2)), Expr::Literal(core::ast::ArtValue::Int(10))] })]).unwrap();
    let pri_vals: Vec<i64> = interp.actors.get(&aid_pri).unwrap().mailbox.iter().iter().map(|e| if let core::ast::ArtValue::Int(n) = &e.payload { *n } else { -1 }).collect();
    assert_eq!(pri_vals, vec![2, 1]);

    // 2) backpressure: set mailbox limit to 1 and verify second send fails
    let mut interp2 = Interpreter::with_prelude();
    interp2.actor_mailbox_limit = 1;
    interp2.interpret(vec![Stmt::SpawnActor { body: vec![] }]).unwrap();
    let aid2 = match interp2.last_value.clone().unwrap() {
        core::ast::ArtValue::Int(n) => n as u32,
        _ => panic!("unexpected actor id"),
    };
    interp2.interpret(vec![Stmt::Expression(Expr::Call { callee: Box::new(Expr::Variable { name: core::Token::dummy("actor_send") }), arguments: vec![Expr::Literal(core::ast::ArtValue::Int(aid2 as i64)), Expr::Literal(core::ast::ArtValue::Int(10))] })]).unwrap();
    // first should succeed
    let res1 = interp2.last_value.clone().unwrap();
    assert!(matches!(res1, core::ast::ArtValue::Bool(true) | core::ast::ArtValue::Optional(_)));
    interp2.interpret(vec![Stmt::Expression(Expr::Call { callee: Box::new(Expr::Variable { name: core::Token::dummy("actor_send") }), arguments: vec![Expr::Literal(core::ast::ArtValue::Int(aid2 as i64)), Expr::Literal(core::ast::ArtValue::Int(11))] })]).unwrap();
    let res2 = interp2.last_value.clone().unwrap();
    // second should be Bool(false) due to backpressure
    assert!(matches!(res2, core::ast::ArtValue::Bool(false)));

    // 3) scheduler: spawn actor with two println statements; after running scheduler, actor should be removed
    let mut interp3 = Interpreter::with_prelude();
    let body = vec![
        Stmt::Expression(Expr::Call { callee: Box::new(Expr::Variable { name: core::Token::dummy("println") }), arguments: vec![Expr::Literal(core::ast::ArtValue::Int(1))] }),
        Stmt::Expression(Expr::Call { callee: Box::new(Expr::Variable { name: core::Token::dummy("println") }), arguments: vec![Expr::Literal(core::ast::ArtValue::Int(2))] }),
    ];
    interp3.interpret(vec![Stmt::SpawnActor { body }]).unwrap();
    let aid3 = match interp3.last_value.clone().unwrap() { core::ast::ArtValue::Int(n) => n as u32, _ => panic!() };
    interp3.run_actors_round_robin(10);
    assert!(!interp3.actors.contains_key(&aid3));
}
