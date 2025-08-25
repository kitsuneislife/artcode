use std::env;
use std::fs::write;
use std::path::PathBuf;

use core::ast::{Expr, FunctionParam, Stmt};
use core::Token;
use ir::lower_stmt;

fn build_add() -> (String, String) {
    let name = Token::dummy("add");
    let a = Token::dummy("a");
    let b = Token::dummy("b");
    let params = vec![FunctionParam { name: a.clone(), ty: None }, FunctionParam { name: b.clone(), ty: None }];
    let ret_expr = Expr::Binary { left: Box::new(Expr::Variable { name: a }), operator: Token::dummy("+"), right: Box::new(Expr::Variable { name: b }) };
    let func = Stmt::Function { name, params, return_type: Some("i64".to_string()), body: std::rc::Rc::new(Stmt::Return { value: Some(ret_expr) }), method_owner: None };
    let irf = lower_stmt(&func).expect("lower failed");
    ("add.ir".to_string(), irf.emit_text())
}

fn build_sub() -> (String, String) {
    let name = Token::dummy("sub");
    let a = Token::dummy("a");
    let b = Token::dummy("b");
    let params = vec![FunctionParam { name: a.clone(), ty: None }, FunctionParam { name: b.clone(), ty: None }];
    let ret_expr = Expr::Binary { left: Box::new(Expr::Variable { name: a }), operator: Token::dummy("-"), right: Box::new(Expr::Variable { name: b }) };
    let func = Stmt::Function { name, params, return_type: Some("i64".to_string()), body: std::rc::Rc::new(Stmt::Return { value: Some(ret_expr) }), method_owner: None };
    let irf = lower_stmt(&func).expect("lower failed");
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
    let irf = lower_stmt(&func).expect("lower failed");
    ("if.ir".to_string(), irf.emit_text())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let write_golden = args.iter().any(|s| s == "--write");

    let outdir = PathBuf::from("crates/ir/golden");
    if write_golden && !outdir.exists() {
        let _ = std::fs::create_dir_all(&outdir);
    }

    let fixtures: Vec<(String,String)> = vec![build_add(), build_sub(), build_if()];
    for (name, text) in fixtures {
        if write_golden {
            let path = outdir.join(&name);
            if let Err(e) = write(&path, &text) {
                eprintln!("failed to write {}: {}", path.display(), e);
            } else {
                println!("wrote {}", path.display());
            }
        } else {
            println!("---- {} ----\n{}", name, text);
        }
    }
}
