use core::ast::{ArtValue, Expr, FunctionParam, Stmt};
use core::Token;
use ir::{lower_stmt, Function};

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

fn function(name: &str, params: &[&str], body: Stmt) -> Function {
    let func = Stmt::Function {
        type_params: None,
        is_async: false,
        name: Token::dummy(name),
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
    };
    lower_stmt(&func).expect("lowering failed")
}

fn ret(value: Expr) -> Stmt {
    Stmt::Return { value: Some(value) }
}

fn block(stmts: Vec<Stmt>) -> Stmt {
    Stmt::Block { statements: stmts }
}

#[test]
fn emits_valid_llvm_text() {
    let f = function("add", &["a", "b"], ret(bin(var("a"), "+", var("b"))));
    let module = ir::llvm_emitter::emit_llvm_module(&[f], "add");
    assert!(module.contains("define i64 @add(i64 %a, i64 %b)"));
    assert!(module.contains("add i64 %a, %b"));
    assert!(module.contains("ret i64"));
    assert!(module.contains("define i32 @main()"));
    assert!(module.contains("@printf"));
}

fn clang_available() -> bool {
    std::process::Command::new("clang")
        .arg("--version")
        .output()
        .is_ok()
}

/// Compile an emitted module with clang and return its stdout. Returns `None`
/// when clang is unavailable so callers can skip cleanly on minimal machines.
fn compile_and_run(module: &str, tag: &str) -> Option<String> {
    if !clang_available() {
        eprintln!("skipping: clang not available");
        return None;
    }
    let dir = std::env::temp_dir();
    let ll = dir.join(format!("art_llvm_{}.ll", tag));
    let bin = dir.join(format!("art_llvm_{}.bin", tag));
    std::fs::write(&ll, module).expect("write .ll");

    let status = std::process::Command::new("clang")
        .arg("-Wno-override-module")
        .arg(&ll)
        .arg("-o")
        .arg(&bin)
        .status()
        .expect("invoke clang");
    assert!(status.success(), "clang failed on emitted IR:\n{}", module);

    let output = std::process::Command::new(&bin)
        .output()
        .expect("run native binary");
    let _ = std::fs::remove_file(&ll);
    let _ = std::fs::remove_file(&bin);
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[test]
fn roundtrip_arithmetic() {
    // func answer() -> i64 { return 40 + 2 }
    let f = function("answer", &[], ret(bin(int(40), "+", int(2))));
    let module = ir::llvm_emitter::emit_llvm_module(&[f], "answer");
    if let Some(out) = compile_and_run(&module, "arith") {
        assert_eq!(out, "42");
    }
}

#[test]
fn roundtrip_if_else() {
    // func main() -> i64 { if true { return 10 } else { return 20 } }
    let if_stmt = Stmt::If {
        condition: Expr::Literal(ArtValue::Bool(true)),
        then_branch: Box::new(block(vec![ret(int(10))])),
        else_branch: Some(Box::new(block(vec![ret(int(20))]))),
    };
    let f = function("main", &[], block(vec![if_stmt]));
    let module = ir::llvm_emitter::emit_llvm_module(&[f], "main");
    assert!(module.contains("phi i64"), "if/else must emit a phi node");
    if let Some(out) = compile_and_run(&module, "ifelse") {
        assert_eq!(out, "10");
    }
}

#[test]
fn roundtrip_function_call() {
    // func dbl(x) -> i64 { return x + x }
    // func main() -> i64 { return dbl(21) }
    let dbl = function("dbl", &["x"], ret(bin(var("x"), "+", var("x"))));
    let main = function(
        "main",
        &[],
        ret(Expr::Call {
            callee: Box::new(var("dbl")),
            type_args: None,
            arguments: vec![int(21)],
        }),
    );
    let module = ir::llvm_emitter::emit_llvm_module(&[dbl, main], "main");
    assert!(module.contains("call i64 @dbl"));
    if let Some(out) = compile_and_run(&module, "call") {
        assert_eq!(out, "42");
    }
}
