
use clap::Parser;
use std::fs;
use std::path::PathBuf;

use lexer::Lexer;
use parser::Parser as ArtParser;
use interpreter::Interpreter;

#[derive(Parser, Debug)]
#[command(name = "art", version, about, long_about = None)]
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
            let source_code = fs::read_to_string(&path).unwrap_or_else(|e| {
                eprintln!("Erro: Não foi possível ler o arquivo {}: {}", path.display(), e);
                std::process::exit(1);
            });
            run(source_code);
        }
    }
}

fn run(source: String) {
    let mut lexer = Lexer::new(&source);
    let tokens = lexer.scan_tokens();

    let mut parser = ArtParser::new(tokens);
    let ast = parser.parse();

    let mut interpreter = Interpreter::new();
    interpreter.interpret(ast);
}
