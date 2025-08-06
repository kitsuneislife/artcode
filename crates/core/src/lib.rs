
pub mod ast;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    LeftParen, RightParen,
    Minus, Plus, Slash, Star,

    Identifier,
    String(String),
    Number(f64),

    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub token_type: TokenType,
    pub lexeme: String,
    pub line: usize,
}
