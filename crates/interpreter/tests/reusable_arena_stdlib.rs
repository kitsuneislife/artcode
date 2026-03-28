use core::ast::ArtValue;
use interpreter::Interpreter;
use lexer::Lexer;
use parser::Parser;

fn parse_program(src: &str) -> Vec<core::ast::Stmt> {
    let mut lexer = Lexer::new(src.to_string());
    let tokens = lexer.scan_tokens().expect("tokens");
    let mut parser = Parser::new(tokens);
    let (program, diags) = parser.parse();
    assert!(
        diags.is_empty(),
        "unexpected parser diagnostics: {:?}",
        diags.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
    );
    program
}

#[test]
fn reusable_arena_with_tracks_allocations_and_finalize_counts() {
    let src = r#"
        let aid = arena_new();

        func alloc_block() {
            let _a = [1, 2, 3];
            let _b = [4, 5, 6];
            let _c = [7, 8, 9];
        }

        arena_with(aid, alloc_block);
        arena_with(aid, alloc_block);
    "#;

    let program = parse_program(src);
    let mut interp = Interpreter::with_prelude();
    interp.interpret(program).expect("interpret");

    let diags = interp.take_diagnostics();
    assert!(
        diags.is_empty(),
        "unexpected runtime diagnostics: {:?}",
        diags.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
    );

    let aid_val = interp.debug_get_global("aid").expect("aid global");
    let aid = match aid_val {
        ArtValue::Int(v) => v as u32,
        other => panic!("expected Int aid, got {:?}", other),
    };

    let alloc_count = interp.arena_alloc_count.get(&aid).copied().unwrap_or(0);
    assert!(
        alloc_count >= 6,
        "expected allocations in reusable arena, got {}",
        alloc_count
    );

    let finalized_count = interp
        .objects_finalized_per_arena
        .get(&aid)
        .copied()
        .unwrap_or(0);
    assert!(
        finalized_count > 0,
        "expected finalized objects in reusable arena, got {}",
        finalized_count
    );
}

#[test]
fn reusable_arena_reports_unknown_id() {
    let src = r#"
        let ok = arena_release(9999);
    "#;

    let program = parse_program(src);
    let mut interp = Interpreter::with_prelude();
    interp.interpret(program).expect("interpret");

    let ok_val = interp.debug_get_global("ok").expect("ok global");
    assert_eq!(ok_val, ArtValue::Bool(false));

    let diags = interp.take_diagnostics();
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("unknown reusable arena id")),
        "expected unknown reusable arena id diagnostic, got {:?}",
        diags.iter().map(|d| d.message.clone()).collect::<Vec<_>>()
    );
}
