use crate::{Function, Instr, Type};
use core::ast::{Expr, Stmt};
use std::collections::HashMap;

/// Attempt to lower a `Stmt` to an IR `Function`.
/// Currently supports `Stmt::Function` whose body is a `Return` of a Binary Add
/// between either variables or integer literals. It's intentionally small for
/// the initial golden tests.
pub fn lower_plain(stmt: &Stmt) -> Option<Function> {
    match stmt {
        Stmt::Function {
            name,
            params,
            return_type: _,
            body,
            method_owner: _,
        } => {
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
                    } else {
                        return None;
                    }
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
                Expr::Binary {
                    left,
                    operator,
                    right,
                } => {
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
                    Some(Function {
                        name: func_name,
                        params: ir_params,
                        ret: Some(Type::I64),
                        body,
                    })
                }
                Expr::Grouping { expression } => {
                    // unwrap grouping and try again (simple wrapper)
                    if let Expr::Binary {
                        left,
                        operator,
                        right,
                    } = *expression
                    {
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
                Expr::Call { callee, arguments } => {
                    // Lower a direct call returning a value: produce Call instr
                    // Only support simple variable callee for now
                    if let Expr::Variable { name: callee_name } = &*callee {
                        let mut arg_names: Vec<String> = Vec::new();
                        for a in arguments {
                            match a {
                                Expr::Variable { name } => arg_names.push(name.lexeme.clone()),
                                Expr::Literal(core::ast::ArtValue::Int(n)) => {
                                    arg_names.push(n.to_string())
                                }
                                _ => return None,
                            }
                        }
                        let dest = mktemp();
                        let call = Instr::Call(dest.clone(), callee_name.lexeme.clone(), arg_names);
                        let body = vec![call, Instr::Ret(Some(dest))];
                        return Some(Function {
                            name: func_name,
                            params: ir_params,
                            ret: Some(Type::I64),
                            body,
                        });
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
    if let Stmt::Function {
        name,
        params,
        return_type: _,
        body,
        method_owner: _,
    } = stmt
    {
        let func_name = name.lexeme.clone();
        let mut ir_params = Vec::new();
        for p in params.iter() {
            let pname = p.name.lexeme.clone();
            ir_params.push((pname, Type::I64));
        }
        // expects single If in body
        if let Stmt::Block { statements } = &**body {
            if statements.len() != 1 {
                return None;
            }
            if let Stmt::If {
                condition,
                then_branch,
                else_branch,
            } = &statements[0]
            {
                // build temps and labels
                let mut next_temp: usize = 0;
                let fname_prefix = func_name.replace("@", "");
                let mut mktemp = || {
                    let t = format!("%{}_{}", fname_prefix, next_temp);
                    next_temp += 1;
                    t
                };
                let then_bb = format!("{}_then", fname_prefix);
                let else_bb = format!("{}_else", fname_prefix);
                let merge_bb = format!("{}_merge", fname_prefix);

                // optional pre-body (e.g. materialized consts for literal conditions)
                let mut pre_body_opt: Option<Vec<Instr>> = None;
                // lower condition: only var or literal supported for now
                let cond_name = match condition {
                    Expr::Variable { name } => name.lexeme.clone(),
                    Expr::Literal(core::ast::ArtValue::Bool(b)) => {
                        // materialize a const bool as i64 (0/1) in temp; record into pre_body
                        let t = mktemp();
                        let v = if *b { 1 } else { 0 };
                        let pb = vec![Instr::ConstI64(t.clone(), v)];
                        // attach pre_body to be emitted before entry br
                        pre_body_opt = Some(pb);
                        t
                    }
                    _ => return None,
                };

                // Helper: lower a branch stmt (Return or nested If) into a pair
                // (result_temp, instrs). For nested If the helper will emit
                // its own inner labels and a phi at the inner merge point so
                // outer lowering can treat the branch as producing a single
                // temp holding the branch result.
                let mut next_label: usize = 0;
                let mut mklabel = |s: &str| {
                    let id = next_label;
                    next_label += 1;
                    format!("{}_{}_{}", fname_prefix, s, id)
                };

                // recursive lowering helper as an inner function so it can call itself
                fn lower_branch<F, G>(
                    s: &Stmt,
                    mktemp: &mut F,
                    mklabel: &mut G,
                ) -> Option<(String, Vec<Instr>)>
                where
                    F: FnMut() -> String,
                    G: FnMut(&str) -> String,
                {
                    match s {
                        // simple returns
                        Stmt::Return {
                            value: Some(Expr::Literal(core::ast::ArtValue::Int(n))),
                        } => {
                            let tname = mktemp();
                            let instrs = vec![Instr::ConstI64(tname.clone(), *n)];
                            Some((tname, instrs))
                        }
                        Stmt::Return {
                            value: Some(Expr::Variable { name }),
                        } => {
                            let v = name.lexeme.clone();
                            Some((v, vec![]))
                        }
                        Stmt::Return {
                            value:
                                Some(Expr::Binary {
                                    left,
                                    operator,
                                    right,
                                }),
                        } => {
                            let l = if let Expr::Variable { name } = &**left {
                                name.lexeme.clone()
                            } else {
                                return None;
                            };
                            let r = if let Expr::Variable { name } = &**right {
                                name.lexeme.clone()
                            } else {
                                return None;
                            };
                            let dest = mktemp();
                            let op = operator.lexeme.as_str();
                            let bin = match op {
                                "+" => Instr::Add(dest.clone(), l, r),
                                "-" => Instr::Sub(dest.clone(), l, r),
                                "*" => Instr::Mul(dest.clone(), l, r),
                                "/" => Instr::Div(dest.clone(), l, r),
                                _ => return None,
                            };
                            Some((dest, vec![bin]))
                        }
                        // nested if: produce its own inner labels and phi, but do
                        // not produce a final br to the outer merge (outer will do that).
                        Stmt::If {
                            condition: icond,
                            then_branch: ib_then,
                            else_branch: ib_else,
                        } => {
                            let ib_else = ib_else.as_ref()?;
                            // produce unique inner labels
                            let inner_then = mklabel("then");
                            let inner_else = mklabel("else");
                            let inner_merge = mklabel("merge");
                            // temps for inner arm results
                            let then_temp = mktemp();
                            let else_temp = mktemp();
                            let mut instrs: Vec<Instr> = Vec::new();

                            // lower condition (variable or literal bool)
                            let cond_owned = icond.clone();
                            let cond_name = match cond_owned {
                                Expr::Variable { name } => name.lexeme.clone(),
                                Expr::Literal(core::ast::ArtValue::Bool(b)) => {
                                    let t = mktemp();
                                    let v = if b { 1 } else { 0 };
                                    instrs.push(Instr::ConstI64(t.clone(), v));
                                    t
                                }
                                _ => return None,
                            };

                            instrs.push(Instr::BrCond(
                                cond_name.clone(),
                                inner_then.clone(),
                                inner_else.clone(),
                            ));

                            // inner then
                            instrs.push(Instr::Label(inner_then.clone()));
                            // lower inner then into then_temp (we expect a Return or nested)
                            if let Some((tres, mut tins)) = lower_branch(&*ib_then, mktemp, mklabel)
                            {
                                // if the nested returned a different temp, move its value into then_temp
                                if tres != then_temp {
                                    // if tins already produced the desired temp name, keep; otherwise
                                    // ensure the computed value is in then_temp by emitting a move-like op
                                    // since we don't have a mov instr, we materialize by creating a ConstI64
                                    // fallback: emit a phi between tres and itself by using a temp copy via add 0
                                    tins.push(Instr::Add(
                                        then_temp.clone(),
                                        tres.clone(),
                                        "0".to_string(),
                                    ));
                                }
                                for i in tins.into_iter() {
                                    instrs.push(i);
                                }
                            } else {
                                return None;
                            }
                            instrs.push(Instr::Br(inner_merge.clone()));

                            // inner else
                            instrs.push(Instr::Label(inner_else.clone()));
                            if let Some((eres, mut eins)) = lower_branch(&*ib_else, mktemp, mklabel)
                            {
                                if eres != else_temp {
                                    eins.push(Instr::Add(
                                        else_temp.clone(),
                                        eres.clone(),
                                        "0".to_string(),
                                    ));
                                }
                                for i in eins.into_iter() {
                                    instrs.push(i);
                                }
                            } else {
                                return None;
                            }
                            instrs.push(Instr::Br(inner_merge.clone()));

                            // inner merge: phi into a single temp (we'll use then_temp as result)
                            instrs.push(Instr::Label(inner_merge.clone()));
                            let phi_pairs = vec![
                                (then_temp.clone(), inner_then.clone()),
                                (else_temp.clone(), inner_else.clone()),
                            ];
                            instrs.push(Instr::Phi(then_temp.clone(), Type::I64, phi_pairs));

                            Some((then_temp, instrs))
                        }
                        _ => None,
                    }
                };

                // assemble function body
                let mut body: Vec<Instr> = Vec::new();
                // emit pre_body if present
                if let Some(pb) = pre_body_opt.take() {
                    for i in pb.into_iter() {
                        body.push(i);
                    }
                }
                // entry: br_cond cond, then_bb, else_bb
                body.push(Instr::BrCond(
                    cond_name.clone(),
                    then_bb.clone(),
                    else_bb.clone(),
                ));

                // then block: lower the then_branch (may be nested If)
                body.push(Instr::Label(then_bb.clone()));
                let then_res = lower_branch(&*then_branch, &mut mktemp, &mut mklabel)?;
                for i in then_res.1.into_iter() {
                    body.push(i);
                }
                body.push(Instr::Br(merge_bb.clone()));

                // else block
                body.push(Instr::Label(else_bb.clone()));
                let else_branch = else_branch.as_ref()?;
                let else_res = lower_branch(&*else_branch, &mut mktemp, &mut mklabel)?;
                for i in else_res.1.into_iter() {
                    body.push(i);
                }
                body.push(Instr::Br(merge_bb.clone()));

                // merge block
                body.push(Instr::Label(merge_bb.clone()));
                // phi
                let phi_pairs = vec![
                    (then_res.0.clone(), then_bb.clone()),
                    (else_res.0.clone(), else_bb.clone()),
                ];
                let res_temp = mktemp();
                body.push(Instr::Phi(res_temp.clone(), Type::I64, phi_pairs));
                body.push(Instr::Ret(Some(res_temp.clone())));

                return Some(Function {
                    name: func_name,
                    params: ir_params,
                    ret: Some(Type::I64),
                    body,
                });
            }
        }
    }
    None
}

// Update top-level dispatcher to try plain, then if lowering
pub fn lower_stmt(stmt: &Stmt) -> Option<Function> {
    if let Some(f) = lower_plain(stmt) {
        return Some(f);
    }
    if let Some(f) = lower_if_function(stmt) {
        return Some(f);
    }
    // try match lowering
    if let Some(f) = lower_match_function(stmt) {
        return Some(f);
    }
    None
}

// Very small lowering for `match` expressions used in golden tests.
// Currently supports a function whose body is a Block with a single Match
// statement with two arms: a literal arm and a wildcard arm. It lowers to
// a br_cond on the matched expression (treating non-zero as true) and
// produces then/else labels, materializes constants in each arm, and
// merges with a phi.
pub fn lower_match_function(stmt: &Stmt) -> Option<Function> {
    if let Stmt::Function {
        name,
        params,
        return_type: _,
        body,
        method_owner: _,
    } = stmt
    {
        let func_name = name.lexeme.clone();
        let mut ir_params = Vec::new();
        for p in params.iter() {
            let pname = p.name.lexeme.clone();
            ir_params.push((pname, Type::I64));
        }

        if let Stmt::Block { statements } = &**body {
            if statements.len() != 1 {
                return None;
            }
            if let Stmt::Match { expr, cases } = &statements[0] {
                // only support simple variable match and exactly two cases
                if cases.len() != 2 {
                    return None;
                }
                // get the match operand name
                let match_var = match expr {
                    Expr::Variable { name } => name.lexeme.clone(),
                    _ => return None,
                };

                // prepare names
                let mut next_temp: usize = 0;
                let fname_prefix = func_name.replace("@", "");
                let mut mktemp = || {
                    let t = format!("%{}_{}", fname_prefix, next_temp);
                    next_temp += 1;
                    t
                };

                let then_bb = format!("{}_case0", fname_prefix);
                let else_bb = format!("{}_case1", fname_prefix);
                let merge_bb = format!("{}_merge", fname_prefix);

                // optional pre-body (materialized consts for literal match expr)
                let mut pre_body_opt: Option<Vec<Instr>> = None;

                // lower each arm: only accept Return { Some(Literal Int) } or Return { Some(Binary) }
                let arm0 = &cases[0].2;
                let arm1 = &cases[1].2;

                // (old lower_arm removed; replaced below by a variant-aware lower_arm)

                // If the first pattern contains bindings, pre-create temps for them
                // and keep a map from binding name -> temp so arm bodies can reference
                // the placeholder temps.
                let mut binding_map: HashMap<String, String> = HashMap::new();
                let mut binding_instrs: Vec<Instr> = Vec::new();
                if let core::ast::MatchPattern::EnumVariant {
                    params: Some(pats), ..
                } = &cases[0].0
                {
                    for pat in pats.iter() {
                        if let core::ast::MatchPattern::Binding(tok) = pat {
                            let tmp = mktemp();
                            let bname = tok.lexeme.clone();
                            let prefix = format!("%{}_", fname_prefix);
                            let suffix = if let Some(s) = tmp.strip_prefix(&prefix) {
                                s.to_string()
                            } else {
                                tmp.clone()
                            };
                            let btemp = format!("%{}_{}", bname, suffix);
                            // materialize 0 as placeholder for bound values
                            binding_instrs.push(Instr::ConstI64(btemp.clone(), 0));
                            binding_map.insert(bname, btemp);
                        }
                    }
                }

                // updated lower_arm: recognizes variable returns and maps bound names
                let lower_arm = |s: &Stmt,
                                 mktemp: &mut dyn FnMut() -> String,
                                 binding_map: &HashMap<String, String>|
                 -> Option<(String, Vec<crate::Instr>)> {
                    match s {
                        Stmt::Return {
                            value: Some(Expr::Literal(core::ast::ArtValue::Int(n))),
                        } => {
                            let tname = mktemp();
                            let instrs = vec![Instr::ConstI64(tname.clone(), *n)];
                            Some((tname, instrs))
                        }
                        Stmt::Return {
                            value: Some(Expr::Variable { name }),
                        } => {
                            let v = name.lexeme.clone();
                            // if this variable is a bound name, use the mapped temp
                            if let Some(mapped) = binding_map.get(&v) {
                                Some((mapped.clone(), vec![]))
                            } else {
                                // otherwise return the variable name itself (arg or local)
                                Some((v, vec![]))
                            }
                        }
                        Stmt::Return {
                            value:
                                Some(Expr::Binary {
                                    left,
                                    operator,
                                    right,
                                }),
                        } => {
                            let l = if let Expr::Variable { name } = &**left {
                                name.lexeme.clone()
                            } else {
                                return None;
                            };
                            let r = if let Expr::Variable { name } = &**right {
                                name.lexeme.clone()
                            } else {
                                return None;
                            };
                            let dest = mktemp();
                            let op = operator.lexeme.as_str();
                            let bin = match op {
                                "+" => Instr::Add(dest.clone(), l, r),
                                "-" => Instr::Sub(dest.clone(), l, r),
                                "*" => Instr::Mul(dest.clone(), l, r),
                                "/" => Instr::Div(dest.clone(), l, r),
                                _ => return None,
                            };
                            Some((dest, vec![bin]))
                        }
                        _ => None,
                    }
                };

                let then_res = lower_arm(arm0, &mut mktemp, &binding_map)?;
                let else_res = lower_arm(arm1, &mut mktemp, &binding_map)?;

                // assemble
                let mut body: Vec<Instr> = Vec::new();
                // emit pre_body if match expr was a literal and we materialized it
                if let Some(pb) = pre_body_opt.take() {
                    for i in pb.into_iter() {
                        body.push(i);
                    }
                }

                // If the first pattern is a literal, lower equality check.
                // We generate `tmp = sub match_var, <lit>` and branch on tmp (non-zero means not equal).
                // Use inverted targets so then_bb is taken when equal.
                match &cases[0].0 {
                    core::ast::MatchPattern::Literal(core::ast::ArtValue::Int(lit)) => {
                        // If the match expression is a variable, emit subtraction temp
                        match expr {
                            Expr::Variable { name } => {
                                let cmp = mktemp();
                                body.push(Instr::Sub(
                                    cmp.clone(),
                                    name.lexeme.clone(),
                                    format!("{}", lit),
                                ));
                                // cmp != 0 -> not equal -> go to else; cmp == 0 -> equal -> then
                                body.push(Instr::BrCond(
                                    cmp.clone(),
                                    else_bb.clone(),
                                    then_bb.clone(),
                                ));
                            }
                            Expr::Literal(core::ast::ArtValue::Int(v)) => {
                                // constant expression: evaluate equality at compile/lower time
                                let val = if *v == *lit { 1 } else { 0 };
                                let t = mktemp();
                                body.push(Instr::ConstI64(t.clone(), val));
                                body.push(Instr::BrCond(
                                    t.clone(),
                                    then_bb.clone(),
                                    else_bb.clone(),
                                ));
                            }
                            _ => {
                                // unsupported match expr form
                                return None;
                            }
                        }
                    }
                    core::ast::MatchPattern::EnumVariant {
                        enum_name: _ename,
                        variant,
                        params: _,
                    } => {
                        // If the match expression is an EnumInstance literal, compare variant names at lowering time
                        match expr {
                            Expr::Literal(core::ast::ArtValue::EnumInstance {
                                enum_name: _,
                                variant: vname,
                                values: _,
                            }) => {
                                let equal = if vname == &variant.lexeme { 1 } else { 0 };
                                let t = mktemp();
                                body.push(Instr::ConstI64(t.clone(), equal));
                                body.push(Instr::BrCond(
                                    t.clone(),
                                    then_bb.clone(),
                                    else_bb.clone(),
                                ));
                            }
                            _ => {
                                // fallback: branch on truthiness of match_var
                                body.push(Instr::BrCond(
                                    match_var.clone(),
                                    then_bb.clone(),
                                    else_bb.clone(),
                                ));
                            }
                        }
                    }
                    _ => {
                        // default: branch on truthiness of match_var (non-zero true)
                        body.push(Instr::BrCond(
                            match_var.clone(),
                            then_bb.clone(),
                            else_bb.clone(),
                        ));
                    }
                }
                body.push(Instr::Label(then_bb.clone()));
                // emit binding placeholders (if any)
                for i in binding_instrs.iter() {
                    body.push(i.clone());
                }
                for i in then_res.1.iter() {
                    body.push(i.clone());
                }
                body.push(Instr::Br(merge_bb.clone()));
                body.push(Instr::Label(else_bb.clone()));
                for i in else_res.1.iter() {
                    body.push(i.clone());
                }
                body.push(Instr::Br(merge_bb.clone()));
                body.push(Instr::Label(merge_bb.clone()));
                let phi_pairs = vec![
                    (then_res.0.clone(), then_bb.clone()),
                    (else_res.0.clone(), else_bb.clone()),
                ];
                let res_temp = mktemp();
                body.push(Instr::Phi(res_temp.clone(), Type::I64, phi_pairs));
                body.push(Instr::Ret(Some(res_temp.clone())));

                return Some(Function {
                    name: func_name,
                    params: ir_params,
                    ret: Some(Type::I64),
                    body,
                });
            }
        }
    }
    None
}
