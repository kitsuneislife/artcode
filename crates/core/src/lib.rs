pub mod ast;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    LeftParen, RightParen, LeftBrace, RightBrace,
    Minus, Plus, Slash, Star,
    Equal, Semicolon,

    Bang, BangEqual,
    EqualEqual,
    Greater, GreaterEqual,
    Less, LessEqual,

    Let, If, Else,
    True, False,
    And, Or,

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
