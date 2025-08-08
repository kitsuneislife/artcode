use interpreter::interpreter::Interpreter;
use lexer::lexer::Lexer;
use parser::parser::Parser;
use diagnostics::format_diagnostic;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::process;

fn run_file(path: &str) {
    match fs::read_to_string(path) {
        Ok(source) => run_with_source(path, source),
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            process::exit(74);
        }
    }
}

fn run_prompt() {
    loop {
        print!("> ");
        io::stdout().flush().ok();
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() || line.trim().is_empty() { break; }
    run_with_source("<repl>", line);
    }
}

fn run_with_source(_name: &str, source: String) {
    let mut lexer = Lexer::new(source.clone());
    let tokens = match lexer.scan_tokens() {
        Ok(t) => t,
        Err(d) => { eprintln!("{}", format_diagnostic(&source, &d)); return; }
    };
    let mut parser = Parser::new(tokens);
    let (program, diags) = parser.parse();
    if !diags.is_empty() {
        for d in &diags { eprintln!("{}", format_diagnostic(&source, d)); }
        return;
    }
    let mut interpreter = Interpreter::with_prelude();
    if let Err(e) = interpreter.interpret(program) {
        eprintln!("Erro de execução: {}", e);
    }
    for d in interpreter.take_diagnostics() { eprintln!("{}", format_diagnostic(&source, &d)); }
    if !source.trim().ends_with(";") { // heurística simples: se não termina com ';' mostrar valor
        if let Some(val) = interpreter.last_value {
            println!("=> {}", val);
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.len() {
        1 => run_prompt(),
        2 => run_file(&args[1]),
        3 => {
            if args[1] == "run" { run_file(&args[2]); } else {
                println!("Usage: art [run] <script>"); process::exit(64);
            }
        }
        _ => { println!("Usage: art [run] <script>"); process::exit(64); }
    }
}