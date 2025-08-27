use std::env;
use std::fs::write;
use std::path::PathBuf;

use core::ast::{Expr, FunctionParam, Stmt};
use core::Token;
use ir::{lower_stmt, ssa};

fn build_add() -> (String, String) {
    let name = Token::dummy("add");
    let a = Token::dummy("a");
    let b = Token::dummy("b");
    let params = vec![FunctionParam { name: a.clone(), ty: None }, FunctionParam { name: b.clone(), ty: None }];
    let ret_expr = Expr::Binary { left: Box::new(Expr::Variable { name: a }), operator: Token::dummy("+"), right: Box::new(Expr::Variable { name: b }) };
    let func = Stmt::Function { name, params, return_type: Some("i64".to_string()), body: std::rc::Rc::new(Stmt::Return { value: Some(ret_expr) }), method_owner: None };
    let mut irf = lower_stmt(&func).expect("lower failed");
    ssa::rename_temps(&mut irf);
    ("add.ir".to_string(), irf.emit_text())
}

fn build_sub() -> (String, String) {
    let name = Token::dummy("sub");
    let a = Token::dummy("a");
    let b = Token::dummy("b");
    let params = vec![FunctionParam { name: a.clone(), ty: None }, FunctionParam { name: b.clone(), ty: None }];
    let ret_expr = Expr::Binary { left: Box::new(Expr::Variable { name: a }), operator: Token::dummy("-"), right: Box::new(Expr::Variable { name: b }) };
    let func = Stmt::Function { name, params, return_type: Some("i64".to_string()), body: std::rc::Rc::new(Stmt::Return { value: Some(ret_expr) }), method_owner: None };
    let mut irf = lower_stmt(&func).expect("lower failed");
    ssa::rename_temps(&mut irf);
    ("sub.ir".to_string(), irf.emit_text())
}

fn build_if() -> (String, String) {
    let name = Token::dummy("f");
    let params = vec![];
    let cond = Expr::Literal(core::ast::ArtValue::Bool(true));
    let then_stmt = Stmt::Return { value: Some(Expr::Literal(core::ast::ArtValue::Int(1))) };
    let else_stmt = Stmt::Return { value: Some(Expr::Literal(core::ast::ArtValue::Int(2))) };
    let if_stmt = Stmt::If { condition: cond, then_branch: Box::new(then_stmt), else_branch: Some(Box::new(else_stmt)) };
    let func = Stmt::Function { name, params, return_type: Some("i64".to_string()), body: std::rc::Rc::new(Stmt::Block { statements: vec![if_stmt] }), method_owner: None };
    let mut irf = lower_stmt(&func).expect("lower failed");
    ssa::rename_temps(&mut irf);
    ("if.ir".to_string(), irf.emit_text())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let write_golden = args.iter().any(|s| s == "--write");
    let check_golden = args.iter().any(|s| s == "--check");

    let outdir = match args.windows(2).find(|w| w[0] == "--outdir") {
        Some(pair) => PathBuf::from(&pair[1]),
        None => PathBuf::from("crates/ir/golden"),
    };
    if write_golden && !outdir.exists() {
        let _ = std::fs::create_dir_all(&outdir);
    }

    let fixtures: Vec<(String,String)> = vec![build_add(), build_sub(), build_if()];
    let mut failed = false;
    for (name, text) in fixtures {
        let path = outdir.join(&name);
        if write_golden {
            if let Err(e) = write(&path, &text) {
                eprintln!("failed to write {}: {}", path.display(), e);
                failed = true;
            } else {
                println!("wrote {}", path.display());
            }
            continue;
        }

        if check_golden {
            match std::fs::read_to_string(&path) {
                Ok(existing) => {
                    if existing != text {
                        eprintln!("golden mismatch: {}", path.display());
                        eprintln!("--- expected ---\n{}\n--- actual ---\n{}", existing, text);
                        failed = true;
                    } else {
                        println!("ok: {}", path.display());
                    }
                }
                Err(_) => {
                    eprintln!("missing golden: {}", path.display());
                    failed = true;
                }
            }
            continue;
        }

        // default: print to stdout
        println!("---- {} ----\n{}", name, text);
    }

    if failed {
        std::process::exit(2);
    }
}
