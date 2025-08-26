use interpreter::type_infer::TypeEnv;
use interpreter::type_infer::TypeInfer;
use core::ast::{Stmt, Expr, FunctionParam};
use core::token::Token;

#[test]
fn typeinfer_restores_outer_bindings_on_shadow() {
    let mut tenv = TypeEnv::new();
    let mut ti = TypeInfer::new(&mut tenv);
    // simulate: let x = 1; { let x = 2; } ; x should still be Int
    let name = Token::dummy("x");
    let let_outer = Stmt::Let { name: name.clone(), ty: None, initializer: Expr::Literal(core::ast::ArtValue::Int(1)) };
    let inner_name = name.clone();
    let let_inner = Stmt::Block { statements: vec![Stmt::Let { name: inner_name.clone(), ty: None, initializer: Expr::Literal(core::ast::ArtValue::Int(2)) }] };
    ti.visit_stmt(&let_outer);
    // outer binding is set
    assert_eq!(ti.tenv.get_var("x").cloned().unwrap(), core::Type::Int);
    ti.visit_stmt(&let_inner);
    // after inner scope popped, outer binding should still be Int
    assert_eq!(ti.tenv.get_var("x").cloned().unwrap(), core::Type::Int);
}
