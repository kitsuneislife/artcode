use core::ast::{Expr, Stmt};
use crate::{Function, Instr, Type};

/// Attempt to lower a `Stmt` to an IR `Function`.
/// Currently supports `Stmt::Function` whose body is a `Return` of a Binary Add
/// between either variables or integer literals. It's intentionally small for
/// the initial golden tests.
pub fn lower_plain(stmt: &Stmt) -> Option<Function> {
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

            // Handle Binary ops (add/sub/mul/div) and produce appropriate Instrs.
            match expr {
                Expr::Binary { left, operator, right } => {
                    // operator lexeme decides the opcode
                    let op = operator.lexeme.as_str();

                    // helper to extract operand name or const
                    let extract = |e: Expr| -> Option<String> {
                        match e {
                            Expr::Variable { name } => Some(name.lexeme),
                            Expr::Literal(v) => match v {
                                core::ast::ArtValue::Int(n) => Some(format!("{}", n)),
                                _ => None,
                            },
                            _ => None,
                        }
                    };

                    let l = extract(*left)?;
                    let r = extract(*right)?;

                    let body = match op {
                        "+" => vec![Instr::Add(l, r), Instr::Ret(Some("%0".to_string()))],
                        "-" => vec![Instr::Sub(l, r), Instr::Ret(Some("%0".to_string()))],
                        "*" => vec![Instr::Mul(l, r), Instr::Ret(Some("%0".to_string()))],
                        "/" => vec![Instr::Div(l, r), Instr::Ret(Some("%0".to_string()))],
                        _ => return None,
                    };

                    Some(Function {
                        name: func_name,
                        params: ir_params,
                        ret: Some(Type::I64),
                        body,
                    })
                }
                Expr::Grouping { expression } => {
                    // unwrap grouping and try again (simple wrapper)
                    if let Expr::Binary { left, operator, right } = *expression {
                        // reuse above by reconstructing
                        let op = operator.lexeme.as_str();
                        let extract = |e: Expr| -> Option<String> {
                            match e {
                                Expr::Variable { name } => Some(name.lexeme),
                                Expr::Literal(v) => match v {
                                    core::ast::ArtValue::Int(n) => Some(format!("{}", n)),
                                    _ => None,
                                },
                                _ => None,
                            }
                        };
                        let l = extract(*left)?;
                        let r = extract(*right)?;
                        let body = match op {
                            "+" => vec![Instr::Add(l, r), Instr::Ret(Some("%0".to_string()))],
                            "-" => vec![Instr::Sub(l, r), Instr::Ret(Some("%0".to_string()))],
                            "*" => vec![Instr::Mul(l, r), Instr::Ret(Some("%0".to_string()))],
                            "/" => vec![Instr::Div(l, r), Instr::Ret(Some("%0".to_string()))],
                            _ => return None,
                        };
                        Some(Function { name: func_name, params: ir_params, ret: Some(Type::I64), body })
                    } else { None }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Public dispatcher: try normal lowering first, then the simple if-based
/// constant-fold lowering.
pub fn lower_stmt(stmt: &Stmt) -> Option<Function> {
    if let Some(f) = lower_plain(stmt) { return Some(f); }
    lower_stmt_with_if(stmt)
}


// Extend lowering to handle simple If when the condition is a literal boolean and
// both branches are Return with integer literals. This is a tiny constant-fold
// optimization helpful for initial tests. If more complex lowering is needed,
// implement proper basic-block generation.
pub fn lower_stmt_with_if(stmt: &Stmt) -> Option<Function> {
    if let Stmt::Function { name, params, return_type: _, body, method_owner: _ } = stmt {
        let func_name = name.lexeme.clone();
        let mut ir_params = Vec::new();
        for p in params.iter() {
            let pname = p.name.lexeme.clone();
            ir_params.push((pname, Type::I64));
        }

        // Expect body to be Block with single If
        match &**body {
            Stmt::Block { statements } if statements.len() == 1 => {
                if let Stmt::If { condition, then_branch, else_branch } = &statements[0] {
                    // condition must be Literal Bool
                    if let Expr::Literal(core::ast::ArtValue::Bool(b)) = condition {
                        // then_branch and else_branch should be Return { value: Some(Literal Int) }
                        let then_val = if let Stmt::Return { value: Some(Expr::Literal(core::ast::ArtValue::Int(n))) } = &**then_branch { *n } else { return None };
                        let else_val = if let Some(eb) = else_branch { if let Stmt::Return { value: Some(Expr::Literal(core::ast::ArtValue::Int(n))) } = &**eb { *n } else { return None } } else { return None };

                        let pick = if *b { then_val } else { else_val };
                        let body = vec![Instr::ConstI64("%0".to_string(), pick), Instr::Ret(Some("%0".to_string()))];
                        return Some(Function { name: func_name, params: ir_params, ret: Some(Type::I64), body });
                    }
                }
            }
            _ => {}
        }
    }
    None
}
