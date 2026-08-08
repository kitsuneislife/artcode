use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Dollar,
    PipeGreater,
    Comma,
    Semicolon,
    Colon,
    ColonColon,
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
    Import,
    Func,
    Performant,
    Spawn,
    Actor,
    Return,
    Yield,
    While,
    For,
    In,
    Try,
    Catch,
    Impl,
    Weak,      // keyword 'weak' (açúcar)
    Unowned,   // keyword 'unowned' (açúcar)
    Component, // keyword 'component'
    View,      // keyword 'view'
    State,     // binding qualifier 'state'
    Prop,      // binding qualifier 'prop'
    Memo,      // binding qualifier 'memo'
    Ref,       // binding qualifier 'ref'
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
    pub col: usize,
    pub start: usize,
    pub end: usize,
}

impl Token {
    pub fn new(
        token_type: TokenType,
        lexeme: String,
        line: usize,
        col: usize,
        start: usize,
        end: usize,
    ) -> Self {
        Token {
            token_type,
            lexeme,
            line,
            col,
            start,
            end,
        }
    }

    pub fn dummy(lexeme: &str) -> Self {
        Token {
            token_type: TokenType::Identifier,
            lexeme: lexeme.to_string(),
            line: 0,
            col: 0,
            start: 0,
            end: 0,
        }
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:?} {} @{}:{}",
            self.token_type, self.lexeme, self.line, self.col
        )
    }
}
