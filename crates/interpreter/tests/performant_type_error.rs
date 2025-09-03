use core::Token;
use core::ast::{ArtValue, Expr, Program, Stmt};
use interpreter::type_infer::{TypeEnv, TypeInfer};

#[test]
fn performant_return_is_type_error() {
    // Construir AST manualmente: performant { let x = [1,2]; return x; }
    let let_stmt = Stmt::Let {
        name: Token::dummy("x"),
        ty: None,
        initializer: Expr::Array(vec![
            Expr::Literal(ArtValue::Int(1)),
            Expr::Literal(ArtValue::Int(2)),
        ]),
    };
    let return_stmt = Stmt::Return {
        value: Some(Expr::Variable {
            name: Token::dummy("x"),
        }),
    };
    let program: Program = vec![Stmt::Performant {
        statements: vec![let_stmt, return_stmt],
    }];

    let mut tenv = TypeEnv::new();
    let mut inf = TypeInfer::new(&mut tenv);
    let res = inf.run(&program);
    assert!(
        res.is_err(),
        "TypeInfer should reject return inside performant"
    );
}
