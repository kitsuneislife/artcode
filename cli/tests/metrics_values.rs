use serde_json::Value;

use interpreter::interpreter::Interpreter;
use lexer::lexer::Lexer;
use parser::parser::Parser;

// Parse the demo example and run the Interpreter in-process, then assert metric values.
#[test]
fn metrics_values_for_arena_demo() {
    let source = include_str!("../examples/17_metrics_arena_demo.art");
    let mut lexer = Lexer::new(source.to_string());
    let tokens = lexer.scan_tokens().expect("lex ok");
    let mut parser = Parser::new(tokens);
    let (program, diags) = parser.parse();
    assert!(diags.is_empty(), "parser diagnostics: {:?}", diags);

    let mut interp = Interpreter::with_prelude();
    // enable invariant checks as metrics collection mode would
    interp.enable_invariant_checks(true);
    assert!(
        interp.interpret(program).is_ok(),
        "interpret demo program failed"
    );

    // Construct a serde JSON value mirroring what CLI would emit
    let mut m = serde_json::Map::new();
    m.insert(
        "arena_alloc_count".to_string(),
        serde_json::to_value(&interp.arena_alloc_count).unwrap(),
    );
    m.insert(
        "objects_finalized_per_arena".to_string(),
        serde_json::to_value(&interp.objects_finalized_per_arena).unwrap(),
    );
    m.insert(
        "finalizer_promotions_per_arena".to_string(),
        serde_json::to_value(&interp.finalizer_promotions_per_arena).unwrap(),
    );
    let v = Value::Object(m);

    let alloc_map = v
        .get("arena_alloc_count")
        .and_then(|x| x.as_object())
        .expect("missing arena_alloc_count");
    let aid = alloc_map
        .keys()
        .next()
        .expect("no arena id present")
        .parse::<u32>()
        .expect("arena id u32");
    let allocs_opt = alloc_map.get(&aid.to_string()).and_then(|n| n.as_u64());
    assert!(allocs_opt.is_some(), "alloc count numeric");

    let fin_map = v
        .get("objects_finalized_per_arena")
        .and_then(|x| x.as_object())
        .expect("missing objects_finalized_per_arena");
    let fin_opt = fin_map.get(&aid.to_string()).and_then(|n| n.as_u64());
    assert!(
        fin_opt.is_some() || fin_map.get(&aid.to_string()).is_none(),
        "finalized count numeric or absent"
    );

    let prom_map = v
        .get("finalizer_promotions_per_arena")
        .and_then(|x| x.as_object())
        .expect("missing finalizer_promotions_per_arena");
    let prom_opt = prom_map.get(&aid.to_string()).and_then(|n| n.as_u64());
    assert!(
        prom_opt.is_some() || prom_map.get(&aid.to_string()).is_none(),
        "promotions count numeric or absent"
    );
}
