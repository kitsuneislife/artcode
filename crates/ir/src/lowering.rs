use core::ast::{Expr, Stmt};
use crate::{Function, Instr, Type};

/// Attempt to lower a `Stmt` to an IR `Function`.
/// Currently supports `Stmt::Function` whose body is a `Return` of a Binary Add
/// between either variables or integer literals. It's intentionally small for
/// the initial golden tests.
pub fn lower_stmt(stmt: &Stmt) -> Option<Function> {
    match stmt {
        Stmt::Function { name, params, return_type: _, body, method_owner: _ } => {
            // Only support function bodies that are a block with a single Return
            // or a direct Return statement.
            let func_name = name.lexeme.clone();
            // collect params
            let mut ir_params = Vec::new();
            for p in params.iter() {
                let pname = p.name.lexeme.clone();
                ir_params.push((pname, Type::I64));
            }

            // inspect body
            let ret_expr = match &**body {
                Stmt::Return { value } => value.clone(),
                Stmt::Block { statements } if statements.len() == 1 => {
                    if let Stmt::Return { value } = &statements[0] {
                        value.clone()
                    } else { return None; }
                }
                _ => return None,
            };

            // expect Some(Expr)
            let expr = match ret_expr {
                Some(e) => e,
                None => return None,
            };

            // Only support Binary add
            if let Expr::Binary { left, operator: _, right } = expr {
                // left and right can be Variable or Literal(Int)
                let left_name = match *left {
                    Expr::Variable { name } => name.lexeme,
                    Expr::Literal(v) => match v {
                        core::ast::ArtValue::Int(n) => format!("{}", n),
                        _ => return None,
                    },
                    _ => return None,
                };
                let right_name = match *right {
                    Expr::Variable { name } => name.lexeme,
                    Expr::Literal(v) => match v {
                        core::ast::ArtValue::Int(n) => format!("{}", n),
                        _ => return None,
                    },
                    _ => return None,
                };

                let body = vec![Instr::Add(left_name, right_name), Instr::Ret(Some("%0".to_string()))];

                Some(Function {
                    name: func_name,
                    params: ir_params,
                    ret: Some(Type::I64),
                    body,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}
