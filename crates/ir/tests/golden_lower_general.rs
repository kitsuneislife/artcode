use core::Token;
/// Golden tests for the general lowering engine in `lower_fn`.
///
/// These cover AOT lowering of procedural constructs — `let` bindings,
/// `while` loops, `if`/`else` with locals, and comparisons — which are
/// outside the narrow subset handled by `lower_plain` / `lower_if_function`.
use core::ast::{ArtValue, Expr, FunctionParam, MatchPattern, Stmt};
use ir::llvm_emitter::emit_llvm_module;
use ir::lower_fn::lower_function;

fn var(name: &str) -> Expr {
    Expr::Variable {
        name: Token::dummy(name),
    }
}

fn int(n: i64) -> Expr {
    Expr::Literal(ArtValue::Int(n))
}

fn bin(l: Expr, op: &str, r: Expr) -> Expr {
    Expr::Binary {
        left: Box::new(l),
        operator: Token::dummy(op),
        right: Box::new(r),
    }
}

fn let_bind(name: &str, init: Expr) -> Stmt {
    Stmt::Let {
        pattern: MatchPattern::Variable(Token::dummy(name)),
        ty: None,
        initializer: init,
    }
}

fn ret(e: Expr) -> Stmt {
    Stmt::Return { value: Some(e) }
}

fn block(stmts: Vec<Stmt>) -> Stmt {
    Stmt::Block { statements: stmts }
}

fn make_fn(name: &str, params: &[&str], body: Stmt) -> Stmt {
    Stmt::Function {
        name: Token::dummy(name),
        type_params: None,
        params: params
            .iter()
            .map(|p| FunctionParam {
                name: Token::dummy(p),
                ty: None,
            })
            .collect(),
        return_type: Some("i64".to_string()),
        body: std::rc::Rc::new(body),
        method_owner: None,
        is_async: false,
    }
}

fn clang_available() -> bool {
    std::process::Command::new("clang")
        .arg("--version")
        .output()
        .is_ok()
}

fn compile_and_run(module: &str, tag: &str) -> Option<String> {
    if !clang_available() {
        return None;
    }
    let dir = std::env::temp_dir();
    let ll = dir.join(format!("art_gen_{}.ll", tag));
    let bin = dir.join(format!("art_gen_{}.bin", tag));
    std::fs::write(&ll, module).expect("write .ll");
    let status = std::process::Command::new("clang")
        .args(["-Wno-override-module", "-O2"])
        .arg(&ll)
        .arg("-o")
        .arg(&bin)
        .status()
        .expect("invoke clang");
    assert!(status.success(), "clang failed:\n{}", module);
    let out = std::process::Command::new(&bin)
        .output()
        .expect("run binary");
    let _ = std::fs::remove_file(&ll);
    let _ = std::fs::remove_file(&bin);
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// let binding + variable read: verifies that alloca/load instructions are emitted
#[test]
fn lower_let_and_return() {
    // func add_via_let(a, b) { let s = a + b; return s }
    let body = block(vec![
        let_bind("s", bin(var("a"), "+", var("b"))),
        ret(var("s")),
    ]);
    let stmt = make_fn("add_via_let", &["a", "b"], body);
    let f = lower_function(&stmt).expect("lower_fn failed");
    assert!(f.body.iter().any(|i| matches!(i, ir::Instr::Alloca(_))));
    assert!(f.body.iter().any(|i| matches!(i, ir::Instr::Load(_, _))));
    assert!(f.body.iter().any(|i| matches!(i, ir::Instr::Store(_, _))));
}

/// Roundtrip: add_via_let(19, 23) -> 42
#[test]
fn roundtrip_let_add() {
    let body = block(vec![
        let_bind("s", bin(var("a"), "+", var("b"))),
        ret(var("s")),
    ]);
    let add_fn = make_fn("add_via_let", &["a", "b"], body);
    let add_ir = lower_function(&add_fn).expect("lower add");

    // wrap in main that calls add_via_let(19, 23)
    let main_body = ret(Expr::Call {
        callee: Box::new(var("add_via_let")),
        type_args: None,
        arguments: vec![int(19), int(23)],
    });
    let main_fn = make_fn("main", &[], main_body);
    let main_ir = lower_function(&main_fn).expect("lower main");

    let module = emit_llvm_module(&[add_ir, main_ir], "main");
    if let Some(out) = compile_and_run(&module, "let_add") {
        assert_eq!(out, "42");
    }
}

/// while loop: count up to n and return the counter
#[test]
fn roundtrip_while_count() {
    // func count(n) {
    //   let i = 0
    //   while i < n { let i = i + 1 }
    //   return i
    // }
    let while_body = block(vec![let_bind("i", bin(var("i"), "+", int(1)))]);
    let body = block(vec![
        let_bind("i", int(0)),
        Stmt::While {
            condition: bin(var("i"), "<", var("n")),
            body: Box::new(while_body),
        },
        ret(var("i")),
    ]);
    let count_fn = make_fn("count", &["n"], body);
    let count_ir = lower_function(&count_fn).expect("lower count");

    let main_body = ret(Expr::Call {
        callee: Box::new(var("count")),
        type_args: None,
        arguments: vec![int(7)],
    });
    let main_fn = make_fn("main", &[], main_body);
    let main_ir = lower_function(&main_fn).expect("lower main");

    let module = emit_llvm_module(&[count_ir, main_ir], "main");
    if let Some(out) = compile_and_run(&module, "while_count") {
        assert_eq!(out, "7");
    }
}

/// if/else with local variable
#[test]
fn roundtrip_if_with_let() {
    // func clamp_pos(x) {
    //   let r = x
    //   if x < 0 { let r = 0 }
    //   return r
    // }
    let if_body = block(vec![let_bind("r", int(0))]);
    let body = block(vec![
        let_bind("r", var("x")),
        Stmt::If {
            condition: bin(var("x"), "<", int(0)),
            then_branch: Box::new(if_body),
            else_branch: None,
        },
        ret(var("r")),
    ]);
    let fn_stmt = make_fn("clamp_pos", &["x"], body);
    let fn_ir = lower_function(&fn_stmt).expect("lower clamp_pos");

    // call clamp_pos(-5) -> 0
    let main_body = ret(Expr::Call {
        callee: Box::new(var("clamp_pos")),
        type_args: None,
        arguments: vec![int(-5)],
    });
    let main_fn = make_fn("main", &[], main_body);
    let main_ir = lower_function(&main_fn).expect("lower main");

    let module = emit_llvm_module(&[fn_ir, main_ir], "main");
    if let Some(out) = compile_and_run(&module, "if_let") {
        assert_eq!(out, "0");
    }
}
