use interpreter::interpreter::Interpreter;
use lexer::lexer::Lexer;
use parser::parser::Parser;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::process;

fn run_file(path: &str) {
    match fs::read_to_string(path) {
        Ok(source) => {
            run(source);
        }
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            process::exit(74);
        }
    }
}

fn run_prompt() {
    loop {
        print!("> ");
        io::stdout().flush().unwrap();
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() || line.trim().is_empty() {
            break;
        }
        run(line);
    }
}

fn run(source: String) {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.scan_tokens();

    let mut parser = Parser::new(tokens);
    let program = parser.parse();

    let mut interpreter = Interpreter::with_prelude();
    if let Err(e) = interpreter.interpret(program) {
        eprintln!("Erro de execução: {}", e);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    match args.len() {

        1 => {
            run_prompt();
        }

        2 => {
            run_file(&args[1]);
        }

        3 => {
            if args[1] == "run" {
                run_file(&args[2]);
            } else {
                println!("Usage: art [run] <script>");
                process::exit(64);
            }
        }
        _ => {
            println!("Usage: art [run] <script>");
            process::exit(64);
        }
    }
}