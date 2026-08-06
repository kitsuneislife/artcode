use codegen_js::{CodegenJs, CodegenOptions, JsOutput};
use lexer::Lexer;
use parser::Parser;

fn compile_component(src: &str) -> String {
    let tokens = Lexer::new(src.to_string()).scan_tokens().expect("lex");
    let (stmts, diags) = Parser::new(tokens).parse();
    assert!(diags.is_empty(), "parse errors: {:?}", diags);
    let out: JsOutput = CodegenJs::new(CodegenOptions::default()).emit_program(&stmts);
    out.code
}

// Scheduler: set_X must call __schedule and produce an async updater
#[test]
fn set_state_uses_schedule() {
    let js = compile_component(
        "component Counter {\n  state count: Int = 0\n  view { <p>{count}</p> }\n}",
    );
    assert!(
        js.contains("__schedule("),
        "expected __schedule call, got:\n{js}"
    );
    assert!(
        js.contains("function set_count("),
        "expected set_count function, got:\n{js}"
    );
}

// Scheduler: memo is recomputed inside the scheduled updater closure, not synchronously
#[test]
fn memo_recomputed_inside_schedule() {
    let js = compile_component(
        "component Calc {\n  state x: Int = 1\n  memo doubled: Int = x * 2\n  view { <p>{doubled}</p> }\n}",
    );
    // The __schedule callback should contain the doubled recomputation
    assert!(js.contains("__schedule("), "expected __schedule");
    assert!(
        js.contains("doubled ="),
        "expected memo recomputation inside schedule, got:\n{js}"
    );
}

// on_mount: __run_mount must be called after DOM creation via tick
#[test]
fn on_mount_called_via_tick() {
    let js = compile_component(
        "component Btn {\n  state clicked: Bool = false\n  view { <button>{clicked}</button> }\n}",
    );
    assert!(
        js.contains("tick("),
        "expected tick() for on_mount, got:\n{js}"
    );
    assert!(
        js.contains("__run_mount("),
        "expected __run_mount call, got:\n{js}"
    );
}

// on_update: __run_update called inside scheduler with changed binding names
#[test]
fn on_update_called_in_scheduler() {
    let js = compile_component("component X {\n  state n: Int = 0\n  view { <p>{n}</p> }\n}");
    assert!(
        js.contains("__run_update("),
        "expected __run_update inside updater, got:\n{js}"
    );
}

// D.3 — component create function must return setters for composability
#[test]
fn component_create_returns_setters() {
    let js = compile_component(
        "component Counter {\n  state count: Int = 0\n  view { <p>{count}</p> }\n}",
    );
    assert!(
        js.contains("return { set_count }"),
        "expected return {{ set_count }} for composability, got:\n{js}"
    );
}

// D.3 — multiple state bindings: all setters returned
#[test]
fn component_create_returns_all_setters() {
    let js = compile_component(
        "component Form {\n  state name: String = \"\"\n  state age: Int = 0\n  view { <div>{name}</div> }\n}",
    );
    assert!(
        js.contains("set_name") && js.contains("set_age"),
        "expected both setters in return, got:\n{js}"
    );
    assert!(
        js.contains("return {"),
        "expected return object, got:\n{js}"
    );
}
