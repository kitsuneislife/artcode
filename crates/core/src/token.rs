use std::fmt;

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
    pub line: usize,
}

impl Token {
    pub fn new(token_type: TokenType, lexeme: String, line: usize) -> Self {
        Token {
            token_type,
            lexeme,
            line,
        }
    }

    pub fn dummy(lexeme: &str) -> Self {
        Token {
            token_type: TokenType::Identifier,
            lexeme: lexeme.to_string(),
            line: 0,
        }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?} {}", self.token_type, self.lexeme)
    }
}