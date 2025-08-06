use core::{Token, TokenType};
use std::collections::HashMap;

pub struct Lexer {
    source: Vec<char>,
    tokens: Vec<Token>,
    start: usize,
    current: usize,
    line: usize,
    keywords: HashMap<String, TokenType>,
}

impl Lexer {
    pub fn new(source: String) -> Self {
        let mut keywords = HashMap::new();
        keywords.insert("let".to_string(), TokenType::Let);
        keywords.insert("if".to_string(), TokenType::If);
        keywords.insert("else".to_string(), TokenType::Else);
        keywords.insert("true".to_string(), TokenType::True);
        keywords.insert("false".to_string(), TokenType::False);
        keywords.insert("struct".to_string(), TokenType::Struct);
        keywords.insert("enum".to_string(), TokenType::Enum);
        keywords.insert("and".to_string(), TokenType::And);
        keywords.insert("or".to_string(), TokenType::Or);
        keywords.insert("match".to_string(), TokenType::Match);
        keywords.insert("case".to_string(), TokenType::Case);
        keywords.insert("func".to_string(), TokenType::Func);
        keywords.insert("return".to_string(), TokenType::Return);
        keywords.insert("none".to_string(), TokenType::None);
        keywords.insert("as".to_string(), TokenType::As);

        Lexer {
            source: source.chars().collect(),
            tokens: Vec::new(),
            start: 0,
            current: 0,
            line: 1,
            keywords,
        }
    }

    pub fn scan_tokens(&mut self) -> Vec<Token> {
        while !self.is_at_end() {
            self.start = self.current;
            self.scan_token();
        }
        self.tokens
            .push(Token::new(TokenType::Eof, "".to_string(), self.line));
        self.tokens.clone()
    }

    fn scan_token(&mut self) {
        let c = self.advance();
        match c {
            '(' => self.add_token(TokenType::LeftParen),
            ')' => self.add_token(TokenType::RightParen),
            '{' => self.add_token(TokenType::LeftBrace),
            '}' => self.add_token(TokenType::RightBrace),
            '[' => self.add_token(TokenType::LeftBracket),
            ']' => self.add_token(TokenType::RightBracket),
            ',' => self.add_token(TokenType::Comma),
            '.' => self.add_token(TokenType::Dot),
            '-' => {
                if self.match_char('>') {
                    self.add_token(TokenType::Arrow);
                } else {
                    self.add_token(TokenType::Minus);
                }
            }
            '+' => self.add_token(TokenType::Plus),
            ';' => self.add_token(TokenType::Semicolon),
            ':' => self.add_token(TokenType::Colon),
            '*' => self.add_token(TokenType::Star),
            '?' => self.add_token(TokenType::Question),
            '_' => self.add_token(TokenType::Underscore),
            '!' => {
                let token = if self.match_char('=') {
                    TokenType::BangEqual
                } else {
                    TokenType::Bang
                };
                self.add_token(token);
            }
            '=' => {
                let token = if self.match_char('=') {
                    TokenType::EqualEqual
                } else {
                    TokenType::Equal
                };
                self.add_token(token);
            }
            '<' => {
                let token = if self.match_char('=') {
                    TokenType::LessEqual
                } else {
                    TokenType::Less
                };
                self.add_token(token);
            }
            '>' => {
                let token = if self.match_char('=') {
                    TokenType::GreaterEqual
                } else {
                    TokenType::Greater
                };
                self.add_token(token);
            }
            '/' => {
                if self.match_char('/') {
                    while self.peek() != '\n' && !self.is_at_end() {
                        self.advance();
                    }
                } else {
                    self.add_token(TokenType::Slash);
                }
            }
            ' ' | '\r' | '\t' => (),
            '\n' => self.line += 1,
            '"' => self.string(),
            'f' => { // <<< LÓGICA PARA 'f'
                if self.peek() == '"' {
                    self.advance(); // Consome o "
                    self.interpolated_string();
                } else {
                    self.identifier();
                }
            }
            c if c.is_ascii_digit() => self.number(),
            c if c.is_alphabetic() || c == '_' => self.identifier(),
            _ => {
                panic!("[line {}] Error: Unexpected character.", self.line);
            }
        }
    }

    fn interpolated_string(&mut self) {
        while self.peek() != '"' && !self.is_at_end() {
            if self.peek() == '\n' {
                self.line += 1;
            }
            self.advance();
        }

        if self.is_at_end() {
            panic!("[line {}] Error: Unterminated string.", self.line);
        }

        self.advance(); // The closing ".

        let value: String = self.source[self.start + 2..self.current - 1].iter().collect();
        self.add_token(TokenType::InterpolatedString(value));
    }

    fn string(&mut self) {
        while self.peek() != '"' && !self.is_at_end() {
            if self.peek() == '\n' {
                self.line += 1;
            }
            self.advance();
        }

        if self.is_at_end() {
            panic!("[line {}] Error: Unterminated string.", self.line);
        }

        self.advance();

        let value: String = self.source[self.start + 1..self.current - 1].iter().collect();
        self.add_token(TokenType::String(value));
    }

    fn number(&mut self) {
        while self.peek().is_ascii_digit() {
            self.advance();
        }

        if self.peek() == '.' && self.peek_next().is_ascii_digit() {
            self.advance();
            while self.peek().is_ascii_digit() {
                self.advance();
            }
        }

        let value: String = self.source[self.start..self.current].iter().collect();
        let num: f64 = value.parse().unwrap();
        self.add_token(TokenType::Number(num));
    }

    fn identifier(&mut self) {
        while self.peek().is_alphanumeric() || self.peek() == '_' {
            self.advance();
        }
        let text: String = self.source[self.start..self.current].iter().collect();
        let token_type = self.keywords.get(&text).cloned().unwrap_or(TokenType::Identifier);
        self.add_token(token_type);
    }

    fn add_token(&mut self, token_type: TokenType) {
        let text: String = self.source[self.start..self.current].iter().collect();
        self.tokens.push(Token::new(token_type, text, self.line));
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.is_at_end() || self.source[self.current] != expected {
            false
        } else {
            self.current += 1;
            true
        }
    }

    fn peek(&self) -> char {
        if self.is_at_end() {
            '\0'
        } else {
            self.source[self.current]
        }
    }

    fn peek_next(&self) -> char {
        if self.current + 1 >= self.source.len() {
            '\0'
        } else {
            self.source[self.current + 1]
        }
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn advance(&mut self) -> char {
        let c = self.source[self.current];
        self.current += 1;
        c
    }
}