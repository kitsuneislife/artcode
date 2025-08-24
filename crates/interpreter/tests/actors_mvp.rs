use interpreter::interpreter::Interpreter;
use core::ast::{Stmt, Expr};

// Test stub para o MVP de atores. Inicialmente marcado como ignored até implementação do runtime.
#[test]
#[ignore]
fn actor_send_receive_fifo() {
    // plano: spawn actor que recebe uma mensagem e coloca em global; envia mensagem e verifica ordem
    let mut interp = Interpreter::with_prelude();
    // TODO: implementar quando runtime de atores estiver disponível
    assert!(true);
}
