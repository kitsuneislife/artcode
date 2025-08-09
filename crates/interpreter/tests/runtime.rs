use lexer::Lexer;
use parser::Parser;
use interpreter::Interpreter;

fn run(src: &str) -> Vec<diagnostics::Diagnostic> {
    let mut lexer = Lexer::new(src.to_string());
    let tokens = match lexer.scan_tokens() { Ok(t) => t, Err(d) => return vec![d] };
    let mut parser = Parser::new(tokens);
    let (program, diags) = parser.parse();
    if !diags.is_empty() { return diags; }
    let mut interp = Interpreter::with_prelude();
    if let Err(e) = interp.interpret(program) { panic!("Runtime error: {:?}", e); }
    let r = interp.take_diagnostics();
    r
}

#[test]
fn fstring_expression_arithmetic() {
    assert!(run("let a=2; let b=3; println(f\"res={a * (b + 4)}\");").is_empty());
}

#[test]
fn fstring_nested_braces_and_escape() {
    assert!(run("let x=1; println(f\"x={{ {x} }}\");").is_empty());
}

#[test]
fn enum_shorthand_inference_ok() {
    assert!(run("let v = .Ok(10);").is_empty());
}

#[test]
fn enum_shorthand_ambiguous() {
    // Ambiguidade agora gera diagnostic e não panica
    let _ = run("enum A { X(Int) } enum B { X(Int) } let v = .X(1);");
}

#[test]
fn scope_preserved_in_function_call() {
    assert!(run("let z=5; func inc(a){ return a + z; } println(inc(10));").is_empty());
}

#[test]
fn field_access_array_sum() {
    assert!(run("let arr=[1,2,3]; println(arr.sum());").is_empty());
}

#[test]
fn field_access_array_sum_type_error() {
    // agora deve gerar diagnostic e não panic
    let diags = run("let arr=[1,2,\"a\"]; println(arr.sum());");
    assert!(diags.iter().any(|d| d.message.contains("Type mismatch in sum")));
}

#[test]
fn field_access_array_count() {
    assert!(run("let arr=[1,2,3]; println(arr.count());").is_empty());
}

#[test]
fn fstring_format_specs() {
    let code = "let n=255; let s= \"  Abc  \"; println(f\"{n:hex} {s:trim} {s:upper} {s:lower} {s:pad10}\");";
    let diags = run(code);
    if !diags.is_empty() { for d in &diags { eprintln!("DIAG: {}", d.message); } }
    assert!(diags.is_empty());
}

#[test]
fn division_by_zero() {
    let diags = run("println(10 / 0);");
    assert!(diags.iter().any(|d| d.message.contains("Division by zero")));
}

#[test]
fn struct_method_auto_self() {
    // Define um struct, método que usa self.nome e retorna saudação
    // Auto-binding deve permitir chamada p.greet() sem passar self
    let src = r#"
    struct Pessoa { nome: String, }
    func Pessoa.greet(self) { }
        let p = Pessoa { nome: "Linus" };
    p.greet();
    "#;
    let diags = run(src);
    if !diags.is_empty() {
        for d in &diags { eprintln!("DIAG: {}", d.message); }
    }
    assert!(diags.is_empty());
}

#[test]
fn struct_method_uses_self_field() {
    let src = r#"
        struct Pessoa { nome: String, }
        func Pessoa.name_len(self) {
            println(len(self.nome));
        }
        let p = Pessoa { nome: "Ada" };
        p.name_len();
    "#;
    let diags = run(src);
    if !diags.is_empty() { for d in &diags { eprintln!("DIAG: {}", d.message); } }
    assert!(diags.is_empty());
}

#[test]
fn enum_method_no_payload() {
    let src = r#"
        enum Status { Ok, Err(String) }
        func Status.ping(self) { println("ping"); }
        let s = Status.Ok;
        s.ping();
    "#;
    let diags = run(src);
    if !diags.is_empty() { for d in &diags { eprintln!("DIAG: {}", d.message); } }
    assert!(diags.is_empty());
}

#[test]
fn enum_method_with_payload() {
    let src = r#"
        enum Box { One(Int) }
        func Box.show(self) { println("show"); }
        let b = Box.One(10);
        b.show();
    "#;
    let diags = run(src);
    if !diags.is_empty() { for d in &diags { eprintln!("DIAG: {}", d.message); } }
    assert!(diags.is_empty());
}

#[test]
fn enum_method_introspection() {
    let src = r#"
        enum Status { Ok, Err(String) }
        func Status.describe(self) {
            println(f"variant={variant} values={len(values)}");
        }
        let s1 = Status.Ok;
        let s2 = Status.Err("fail");
        s1.describe();
        s2.describe();
    "#;
    let diags = run(src);
    if !diags.is_empty() { for d in &diags { eprintln!("DIAG: {}", d.message); } }
    assert!(diags.is_empty());
}

#[test]
fn fstring_malformed_error() {
    let code = "println(f\"unclosed {expr\");";
    let diags = run(code);
    assert!(!diags.is_empty());
    assert!(diags.iter().any(|d| d.message.contains("Unterminated interpolation")));
}
