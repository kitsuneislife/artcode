use interpreter::Interpreter;
use lexer::Lexer;
use parser::Parser;

fn main() {
    let s = "]}=v\n}r_L\t)DP0)-A__*]:X[\t{xWr=}XOQ((";
    let mut l = Lexer::new(s.to_string());
    if let Ok(t) = l.scan_tokens() {
        let mut p = Parser::new(t);
        let (ast, _) = p.parse();
        println!("AST parsed successfully with length {}", ast.len());
        println!("{:#?}", ast);
        println!("Starting interpretation...");
        let mut i = Interpreter::with_prelude();
        let _ = i.interpret(ast);
    }
}
