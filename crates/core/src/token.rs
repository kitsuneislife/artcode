use std::fmt;
use crate::intern;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Semicolon,
    Colon,
    Dot,
    Arrow,
    Minus,
    Plus,
    Star,
    Slash,
    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    Let,
    If,
    Else,
    True,
    False,
    Struct,
    Enum,
    And,
    Or,
    Match,
    Case,
    Underscore,
    Func,
    Return,
    Weak,      // keyword 'weak' (açúcar)
    Unowned,   // keyword 'unowned' (açúcar)
    Identifier,
    String(String),
    InterpolatedString(String), // <<< NOSSO NOVO TOKEN
    Number(f64),
    None,
    Question,
    As,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub token_type: TokenType,
    pub lexeme: String,
    pub symbol: Option<&'static str>, // intern para identifiers/keywords
    pub line: usize,
    pub col: usize,
    pub start: usize,
    pub end: usize,
}

impl Token {
    pub fn new(token_type: TokenType, lexeme: String, line: usize, col: usize, start: usize, end: usize) -> Self {
    let symbol = matches!(token_type, TokenType::Identifier | TokenType::Let | TokenType::If | TokenType::Else | TokenType::True | TokenType::False | TokenType::Struct | TokenType::Enum | TokenType::And | TokenType::Or | TokenType::Match | TokenType::Case | TokenType::Func | TokenType::Return | TokenType::None | TokenType::As | TokenType::Weak | TokenType::Unowned)
            .then(|| intern(&lexeme));
        Token { token_type, lexeme, symbol, line, col, start, end }
    }

    pub fn dummy(lexeme: &str) -> Self {
    Token { token_type: TokenType::Identifier, lexeme: lexeme.to_string(), symbol: None, line: 0, col: 0, start: 0, end: 0 }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{:?} {} @{}:{}", self.token_type, self.lexeme, self.line, self.col)
    }
}