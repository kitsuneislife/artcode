use core::ast::{Expr, Stmt};
use interpreter::interpreter::Interpreter;

/// Teste de estresse: verifica se o Agendador de Atores suporta o envio e o processamento de
/// um grande volume de mensagens roteadas concorrentemente entre múltiplas threads
/// nativas sem panics e de forma assíncrona até exaustão.
#[test]
fn test_scheduler_actor_massive_message_stress() {
    // Aumentar o limite de pilha da thread nativa para não afetar
    // loops de eventos curtos.
    let builder = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .name("actor_tester_main".to_string());

    builder
        .spawn(|| {
            let mut interp = Interpreter::with_prelude();
            let actors_qty = 50;
            let messages_per_actor = 1000;
            let total_messages_expected = actors_qty * messages_per_actor;

            let mut actor_ids = Vec::new();

            // 1. Criar os atores virtuais com um bloco de "receive"
            // Cada ator só consome a mensagem do mailbox local.
            // Para testar em nível AST puro sem Parser num bloco contínuo, faremos "Loop Unrolling", 
            // injetando `messages_per_actor` chamadas de receiver em seu bloco.
            for _ in 0..actors_qty {
                let mut receive_statements = Vec::with_capacity(messages_per_actor);
                for _ in 0..messages_per_actor {
                    receive_statements.push(Stmt::Expression(Expr::Call {
                        type_args: None,
                        callee: Box::new(Expr::Variable {
                            name: core::Token::dummy("actor_receive"),
                        }),
                        arguments: vec![],
                    }));
                }

                let body = vec![
                    Stmt::Block {
                        statements: receive_statements,
                    }
                ];
                
                interp.interpret(vec![Stmt::SpawnActor { body }]).unwrap();
                let aid = match interp.last_value.clone().unwrap() {
                    core::ast::ArtValue::Actor(id) => id,
                    _ => panic!("Expected actor ID"),
                };
                actor_ids.push(aid);
            }

            // 2. Enviar massivamente (Roteamento Assíncrono para os mailboxes sem execução)
            // Isso encherá as filas locais sem consumir ainda (exceto as preemptadas).
            for (i, aid) in actor_ids.iter().enumerate() {
                for m in 0..messages_per_actor {
                    let target = actor_ids[(i + (m as usize)) % actors_qty as usize];
                    
                    // actor_send(target, message_value)
                    interp.interpret(vec![Stmt::Expression(Expr::Call {
                        type_args: None,
                        callee: Box::new(Expr::Variable {
                            name: core::Token::dummy("actor_send"),
                        }),
                        arguments: vec![
                            Expr::Literal(core::ast::ArtValue::Int(target as i64)),
                            Expr::Literal(core::ast::ArtValue::Int(m as i64)),
                        ],
                    })]).unwrap();
                }
            }

            // 3. Forçar o agendador nativo a resolver todo o pool assíncrono enfileirado nas threads C.
            // `run_actors_round_robin(N)` irá drenar as pilhas iterativamente pelo Agendador Nativo VM.
            interp.run_actors_round_robin((total_messages_expected + 10000) as usize);

            // 4. Verificação final de estado: 
            // Todos os mailboxes devem estar vazios sem panics do Rust Lock (sem deadlock de mutex).
            let mut remaining = 0;
            for actor_id in &actor_ids {
                if let Some(actor) = interp.actors.get(actor_id) {
                    remaining += actor.mailbox.len();
                }
            }

            assert_eq!(
                remaining, 
                0,
                "Não pode haver mensagens trancadas nos mailboxes após run_actors! Scheduler Deadlock detectado!"
            );
        })
        .unwrap()
        .join()
        .unwrap();
}
