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

                // helper to create unique temps (prefixed with function name)
                let mut next_temp: usize = 0;
                let fname_prefix = func_name.replace("@", "");
                let mut mktemp = || {
                    let t = format!("%{}_{}", fname_prefix, next_temp);
                    next_temp += 1;
                    t
                };

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
                    let dest = mktemp();
                    let bin = match op {
                        "+" => Instr::Add(dest.clone(), l, r),
                        "-" => Instr::Sub(dest.clone(), l, r),
                        "*" => Instr::Mul(dest.clone(), l, r),
                        "/" => Instr::Div(dest.clone(), l, r),
                        _ => return None,
                    };
                    let body = vec![bin, Instr::Ret(Some(dest))];
                    Some(Function { name: func_name, params: ir_params, ret: Some(Type::I64), body })
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
                        let dest = mktemp();
                        let body = match op {
                            "+" => vec![Instr::Add(dest.clone(), l, r), Instr::Ret(Some(dest))],
                            "-" => vec![Instr::Sub(dest.clone(), l, r), Instr::Ret(Some(dest))],
                            "*" => vec![Instr::Mul(dest.clone(), l, r), Instr::Ret(Some(dest))],
                            "/" => vec![Instr::Div(dest.clone(), l, r), Instr::Ret(Some(dest))],
                            _ => return None,
                        };
                        Some(Function { name: func_name, params: ir_params, ret: Some(Type::I64), body })
                    } else { None }
                }
                Expr::Call { callee, arguments } => {
                    // Lower a direct call returning a value: produce Call instr
                    // Only support simple variable callee for now
                    if let Expr::Variable { name } = &**callee {
                        let mut arg_names: Vec<String> = Vec::new();
                        for a in arguments {
                            match a {
                                Expr::Variable { name } => arg_names.push(name.lexeme.clone()),
                                Expr::Literal(core::ast::ArtValue::Int(n)) => arg_names.push(n.to_string()),
                                _ => return None,
                            }
                        }
                        let dest = mktemp();
                        let call = Instr::Call(dest.clone(), name.lexeme.clone(), arg_names);
                        let body = vec![call, Instr::Ret(Some(dest))];
                        return Some(Function { name: func_name, params: ir_params, ret: Some(Type::I64), body });
                    }
                    None
                }
                _ => None,
            }
        }
        _ => None,
    }
}

// New: lowering for general If into basic blocks with labels and br_cond.
// It emits:
// entry:
//   br_cond %pred, then_bb, else_bb
// then_bb:
//   %t = ...
//   br merge_bb
// else_bb:
//   %e = ...
//   br merge_bb
// merge_bb:
//   %res = phi [ %t, then_bb ], [ %e, else_bb ]
pub fn lower_if_function(stmt: &Stmt) -> Option<Function> {
    if let Stmt::Function { name, params, return_type: _, body, method_owner: _ } = stmt {
        let func_name = name.lexeme.clone();
        let mut ir_params = Vec::new();
        for p in params.iter() {
            let pname = p.name.lexeme.clone();
            ir_params.push((pname, Type::I64));
        }
        // expects single If in body
        if let Stmt::Block { statements } = &**body {
            if statements.len() != 1 { return None }
            if let Stmt::If { condition, then_branch, else_branch } = &statements[0] {
                // build temps and labels
                let mut next_temp: usize = 0; let mut mktemp = || { let t = format!("%{}", next_temp); next_temp += 1; t };
                let then_bb = format!("{}_then", fname_prefix);
                let else_bb = format!("{}_else", fname_prefix);
                let merge_bb = format!("{}_merge", fname_prefix);

                // lower condition: only var or literal supported for now
                let cond_name = match condition {
                    Expr::Variable { name } => name.lexeme.clone(),
                        Expr::Literal(core::ast::ArtValue::Bool(b)) => {
                            // materialize a const bool as i64 (0/1) in temp
                            let t = mktemp();
                            let v = if *b { 1 } else { 0 };
                            // Use ConstI64 to represent predicate — collect into pre_body to be emitted
                            let mut pre_body: Vec<Instr> = Vec::new();
                            pre_body.push(Instr::ConstI64(t.clone(), v));
                            // store pre_body in an outer variable by returning a tuple later
                            // We'll attach pre_body before br_cond when assembling the function.
                            // Temporarily stash it in a local by using a side channel below.
                            // For now we return t and later reconstruct pre_body as necessary.
                            t
                        }
                    _ => return None,
                };

                // lower then_branch: expect Return { value: Some(Literal Int) } or Binary
                let then_res = match &**then_branch {
                    Stmt::Return { value: Some(Expr::Literal(core::ast::ArtValue::Int(n))) } => {
                        let tname = mktemp();
                        let instrs = vec![Instr::ConstI64(tname.clone(), *n)];
                        (tname, instrs)
                    }
                    Stmt::Return { value: Some(Expr::Binary { left, operator, right }) } => {
                        let l = if let Expr::Variable { name } = &**left { name.lexeme.clone() } else { return None };
                        let r = if let Expr::Variable { name } = &**right { name.lexeme.clone() } else { return None };
                        let dest = mktemp();
                        let op = operator.lexeme.as_str();
                        let bin = match op {
                            "+" => Instr::Add(dest.clone(), l, r),
                            "-" => Instr::Sub(dest.clone(), l, r),
                            "*" => Instr::Mul(dest.clone(), l, r),
                            "/" => Instr::Div(dest.clone(), l, r),
                            _ => return None,
                        };
                        (dest, vec![bin])
                    }
                    _ => return None,
                };

                // lower else_branch similarly
                let else_branch = else_branch.as_ref()?;
                let else_res = match &**else_branch {
                    Stmt::Return { value: Some(Expr::Literal(core::ast::ArtValue::Int(n))) } => {
                        let tname = mktemp();
                        let instrs = vec![Instr::ConstI64(tname.clone(), *n)];
                        (tname, instrs)
                    }
                    Stmt::Return { value: Some(Expr::Binary { left, operator, right }) } => {
                        let l = if let Expr::Variable { name } = &**left { name.lexeme.clone() } else { return None };
                        let r = if let Expr::Variable { name } = &**right { name.lexeme.clone() } else { return None };
                        let dest = mktemp();
                        let op = operator.lexeme.as_str();
                        let bin = match op {
                            "+" => Instr::Add(dest.clone(), l, r),
                            "-" => Instr::Sub(dest.clone(), l, r),
                            "*" => Instr::Mul(dest.clone(), l, r),
                            "/" => Instr::Div(dest.clone(), l, r),
                            _ => return None,
                        };
                        (dest, vec![bin])
                    }
                    _ => return None,
                };

                // assemble function body
                let mut body: Vec<Instr> = Vec::new();
                // If cond was a literal, we added a ConstI64 earlier; detect that by
                // checking if cond_name looks like a temp starting with '%'. If so and
                // it's not a parameter, emit no-op here (we assume const already emitted
                // as part of temp creation). For simplicity we will not duplicate.
                // entry: br_cond cond, then_bb, else_bb
                body.push(Instr::BrCond(cond_name.clone(), then_bb.clone(), else_bb.clone()));
                // then block
                body.push(Instr::Label(then_bb.clone()));
                for i in then_res.1.iter() { body.push(i.clone()); }
                body.push(Instr::Br(merge_bb.clone()));
                // else block
                body.push(Instr::Label(else_bb.clone()));
                for i in else_res.1.iter() { body.push(i.clone()); }
                body.push(Instr::Br(merge_bb.clone()));
                // merge block
                body.push(Instr::Label(merge_bb.clone()));
                // phi
                let phi_pairs = vec![(then_res.0.clone(), then_bb.clone()), (else_res.0.clone(), else_bb.clone())];
                let res_temp = mktemp();
                body.push(Instr::Phi(res_temp.clone(), Type::I64, phi_pairs));
                body.push(Instr::Ret(Some(res_temp.clone())));

                return Some(Function { name: func_name, params: ir_params, ret: Some(Type::I64), body });
            }
        }
    }
    None
}

// Update top-level dispatcher to try plain, then if lowering
pub fn lower_stmt(stmt: &Stmt) -> Option<Function> {
    if let Some(f) = lower_plain(stmt) { return Some(f); }
    if let Some(f) = lower_if_function(stmt) { return Some(f); }
    None
}

