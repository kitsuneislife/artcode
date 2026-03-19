use core::ast::ArtValue;
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
    // Limpa se existir
    let _ = fs::remove_file(trace_path);

    let src = r#"
        let n = time_now()
        let r = rand_next()
    "#;

    let _interp = run_and_interpret_with_tracer(src, trace_path);

    // Verifica se o arquivo log foi gerado
    assert!(std::path::Path::new(trace_path).exists(), "Tracer devera criar o log");
    
    let content = fs::read(trace_path).expect("read trace");
    // Magic header verification
    assert!(content.starts_with(b"ARTLOG01"), "Deve ter o header ARTLOG01");
    
    // Opcionalmente podemos ler o conteúdo, 
    // mas o mero fato de ser gravado já prova o fluxo
    let _ = fs::remove_file(trace_path); // limpa depois do test
}
