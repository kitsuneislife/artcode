use codegen_js::{CodegenJs, CodegenOptions, JsOutput};
use lexer::Lexer;
use parser::Parser;

fn compile(src: &str) -> String {
    let tokens = Lexer::new(src.to_string()).scan_tokens().expect("lex");
    let (program, diags) = Parser::new(tokens).parse();
    assert!(diags.is_empty(), "parse diagnostics: {:?}", diags);
    let out: JsOutput = CodegenJs::new(CodegenOptions::default()).emit_program(&program);
    out.code
}

#[test]
fn test_self_closing_div_generates_create_element() {
    let js = compile("let v = <div />");
    assert!(
        js.contains("document.createElement(\"div\")"),
        "got: {}",
        js
    );
}

#[test]
fn test_static_attr_generates_set_attribute() {
    let js = compile(r#"let v = <div class="app" />"#);
    assert!(
        js.contains("setAttribute(\"class\", \"app\")"),
        "got: {}",
        js
    );
}

#[test]
fn test_dynamic_attr_generates_set_attribute_with_string_cast() {
    let js = compile("let v = <input value={count} />");
    assert!(js.contains("setAttribute(\"value\""), "got: {}", js);
    assert!(js.contains("String("), "got: {}", js);
}

#[test]
fn test_event_handler_generates_addeventlistener() {
    let js = compile("let v = <button on:click={count} />");
    assert!(js.contains("addEventListener(\"click\""), "got: {}", js);
}

#[test]
fn test_text_node_child_generates_create_text_node() {
    let js = compile("let v = <h1>{title}</h1>");
    assert!(
        js.contains("document.createTextNode(String(title))"),
        "got: {}",
        js
    );
}

#[test]
fn test_component_generates_new_call() {
    // Counter must be defined (struct or import) to pass the component-import check.
    let js = compile("struct Counter { }\nlet v = <Counter />");
    assert!(js.contains("new Counter("), "got: {}", js);
}

#[test]
fn test_template_iife_wraps_single_element() {
    let js = compile("let v = <div />");
    assert!(js.contains("(() => {"), "expected IIFE, got: {}", js);
    assert!(js.contains("return __el_0"), "expected return, got: {}", js);
}

#[test]
fn test_for_node_generates_fragment_and_loop() {
    let js = compile("<for item in {items} key={item}><div /></for>");
    assert!(js.contains("createDocumentFragment"), "got: {}", js);
    assert!(js.contains("for (const item of items)"), "got: {}", js);
}

#[test]
fn test_if_node_generates_conditional() {
    let js = compile("<if cond={ok}><span /></if>");
    assert!(js.contains("if (ok)"), "got: {}", js);
}

#[test]
fn test_slot_generates_slot_element() {
    let js = compile(r#"let v = <slot name="header" />"#);
    assert!(js.contains("createElement(\"slot\")"), "got: {}", js);
    assert!(
        js.contains("setAttribute(\"name\", \"header\")"),
        "got: {}",
        js
    );
}
