
use clap::Parser;
use std::fs;
use std::path::PathBuf;

mod lexer;
mod parser;
mod interpreter;
mod ast;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    Run {
        #[arg(required = true)]
        path: PathBuf,
    },
}

fn main() {
    let args = Args::parse();

    match args.command {
        Commands::Run { path } => {
            if !path.exists() {
                eprintln!("Erro: Arquivo não encontrado: {}", path.display());
                std::process::exit(1);
            }

            println!("-> Executando o arquivo: {}", path.display());

            let source_code = match fs::read_to_string(&path) {
                Ok(code) => code,
                Err(e) => {
                    eprintln!("Erro: Não foi possível ler o arquivo {}: {}", path.display(), e);
                    std::process::exit(1);
                }
            };

            run(source_code);
        }
    }
}

fn run(source: String) {
    let mut lexer = lexer::Lexer::new(&source);
    let tokens = lexer.scan_tokens();
    println!("-> Tokens: {:?}", tokens); // Debug: mostra os tokens

    let mut parser = parser::Parser::new(tokens);
    let ast = parser.parse();
    println!("-> AST: {:?}", ast); // Debug: mostra a AST

    let mut interpreter = interpreter::Interpreter::new();
    interpreter.interpret(ast);
    println!("-> Execução concluída.");
}
