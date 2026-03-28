use interpreter::Interpreter;
use lexer::Lexer;
use parser::Parser;
use std::fs;

fn run_and_interpret_with_tracer(src: &str, trace_path: &str) -> Interpreter {
    let mut lexer = Lexer::new(src.to_string());
    let tokens = lexer.scan_tokens().expect("tokens");
    let mut parser = Parser::new(tokens);
    let (program, diags) = parser.parse();
    assert!(diags.is_empty(), "parser errs: {:?}", diags);

    let mut interp = Interpreter::with_prelude();
    interp.enable_tracer(trace_path).expect("enable tracer");
    interp.interpret(program).expect("interpret");
    interp
}

#[test]
fn test_tracer_generates_artlog_on_nondeterministic_calls() {
    let trace_path = "test_trace_rand_time.artlog";
    let _ = fs::remove_file(trace_path);

    let src = r#"
 let n = time_now()
 let r = rand_next()
 "#;

    let _interp = run_and_interpret_with_tracer(src, trace_path);

    assert!(
        std::path::Path::new(trace_path).exists(),
        "Tracer devera criar o log"
    );

    let content = fs::read(trace_path).expect("read trace");
    assert!(
        content.starts_with(b"ARTLOG01"),
        "Deve ter o header ARTLOG01"
    );

    let _ = fs::remove_file(trace_path);
}

#[test]
fn test_replayer_reads_artlog_and_provides_events() {
    let trace_path = "test_replayer.artlog";
    let _ = fs::remove_file(trace_path);

    let src = r#"
 let t = time_now()
 let r = rand_next()
 "#;

    let interp = run_and_interpret_with_tracer(src, trace_path);
    let t_val = interp.get_global("t").expect("t deve existir");
    let r_val = interp.get_global("r").expect("r deve existir");

    let mut replayer =
        interpreter::replayer::Replayer::new(trace_path).expect("replayer deve abrir arquivo");

    // Primeiro evento: time_now no tick 1
    let event = replayer.consume_intercept("time_now", 1);
    assert!(event.is_ok(), "consume_intercept deve retornar Ok");
    let payload = event.unwrap();
    assert!(payload.is_some(), "deve ter payload para time_now");
    if let core::ast::ArtValue::Int(recorded) = payload.unwrap() {
        if let core::ast::ArtValue::Int(original) = t_val {
            assert_eq!(
                recorded, original,
                "valor gravado deve ser igual ao executado"
            );
        }
    } else {
        panic!("payload deveria ser Int");
    }

    // Segundo evento: rand_next no tick 2
    let event2 = replayer.consume_intercept("rand_next", 2);
    assert!(event2.is_ok());
    assert!(event2.unwrap().is_some());

    let _ = fs::remove_file(trace_path);
}

#[test]
fn test_tracer_writes_checkpoint_event() {
    let trace_path = "test_trace_checkpoint.artlog";
    let _ = fs::remove_file(trace_path);

    let src = r#"
 let t = time_now()
 let u = time_now()
 let v = time_now()
 let w = time_now()
 let x = time_now()
 let y = time_now()
 let z = time_now()
 let a = time_now()
 let b = time_now()
 let c = time_now()
"#;

    let _ = run_and_interpret_with_tracer(src, trace_path);
    let replayer = interpreter::replayer::Replayer::new(trace_path).expect("replayer");
    let checkpoint = replayer.find_checkpoint_before(10);
    assert!(checkpoint.is_some(), "deve encontrar checkpoint no trace");
    let (tick, payload) = checkpoint.unwrap();
    assert!(tick <= 10);
    if let core::ast::ArtValue::Map(m) = payload {
        let map = m.0.lock().unwrap();
        assert!(
            map.get("rng_state").is_some(),
            "checkpoint deve armazenar rng_state"
        );
    } else {
        panic!("payload checkpoint deve ser Map");
    }

    let _ = fs::remove_file(trace_path);
}

#[test]
fn test_replayer_skips_checkpoint_event() {
    let trace_path = "test_replayer_checkpoint_skip.artlog";
    let _ = fs::remove_file(trace_path);

    let src = r#"
 let t = time_now()
 let u = time_now()
 let v = time_now()
 let w = time_now()
 let x = time_now()
 let y = time_now()
 let z = time_now()
 let a = time_now()
 let b = time_now()
 let c = time_now()
"#;

    let _ = run_and_interpret_with_tracer(src, trace_path);
    let mut replayer = interpreter::replayer::Replayer::new(trace_path).expect("replayer");

    let first = replayer
        .consume_intercept("time_now", 1)
        .expect("should work");
    assert!(first.is_some());
    let second = replayer
        .consume_intercept("time_now", 2)
        .expect("should work");
    assert!(second.is_some());

    let _ = fs::remove_file(trace_path);
}

#[test]
fn test_replayer_returns_none_for_wrong_tick() {
    let trace_path = "test_replayer_wrong_tick.artlog";
    let _ = fs::remove_file(trace_path);

    let src = r#"
 let t = time_now()
 "#;
    let _interp = run_and_interpret_with_tracer(src, trace_path);

    let mut replayer = interpreter::replayer::Replayer::new(trace_path).expect("replayer");

    // Tick errado: time_now foi gravado no tick 1, pedimos no tick 99
    let event = replayer.consume_intercept("time_now", 99);
    assert!(event.is_ok());
    assert!(event.unwrap().is_none(), "tick errado deve retornar None");

    let _ = fs::remove_file(trace_path);
}
