use lexer::Lexer;
use parser::Parser;
use interpreter::Interpreter;

fn run(src: &str) {
    let mut lexer = Lexer::new(src.to_string());
    let tokens = lexer.scan_tokens();
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    let mut interp = Interpreter::with_prelude();
    interp.interpret(program).unwrap();
}

#[test]
fn fstring_expression_arithmetic() {
    run("let a=2; let b=3; println(f\"res={a * (b + 4)}\");");
}

#[test]
fn fstring_nested_braces_and_escape() {
    run("let x=1; println(f\"x={{ {x} }}\");"); // should produce literal '{ ' then value then ' }'
}

#[test]
fn enum_shorthand_inference_ok() {
    run("let v = .Ok(10);");
}

#[test]
#[should_panic]
fn enum_shorthand_ambiguous() {
    // Define two enums with same variant name to trigger ambiguity
    run("enum A { X(Int) } enum B { X(Int) } let v = .X(1);");
}

#[test]
fn scope_preserved_in_function_call() {
    run("let z=5; func inc(a){ return a + z; } println(inc(10));");
}

#[test]
fn field_access_array_sum() {
    run("let arr=[1,2,3]; println(arr.sum());");
}
