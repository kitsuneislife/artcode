mod lexer;
mod keywords;

use core::{Token, TokenType};
use std::collections::HashMap;

pub struct Lexer<'a> {
    chars: Vec<char>,
    tokens: Vec<Token>,
    start: usize,
    current: usize,
    line: usize,
    keywords: HashMap<String, TokenType>,
    _phantom: std::marker::PhantomData<&'a ()>,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        let mut keywords = HashMap::new();
        keywords.insert("let".to_string(), TokenType::Let);
        keywords.insert("if".to_string(), TokenType::If);
        keywords.insert("else".to_string(), TokenType::Else);
        keywords.insert("true".to_string(), TokenType::True);
        keywords.insert("false".to_string(), TokenType::False);
        keywords.insert("and".to_string(), TokenType::And);
        keywords.insert("or".to_string(), TokenType::Or);

        let chars: Vec<char> = source.chars().collect();

        Lexer {
            chars,
            tokens: Vec::new(),
            start: 0,
            current: 0,
            line: 1,
            keywords,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn scan_tokens(&mut self) -> Vec<Token> {
        while !self.is_at_end() {
            self.start = self.current;
            self.scan_token();
        }

        self.tokens.push(Token {
            token_type: TokenType::Eof,
            lexeme: "".to_string(),
            line: self.line,
        });
        self.tokens.clone()
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.chars.len()
    }

    fn scan_token(&mut self) {
        let c = self.advance();
        match c {
            '(' => self.add_token(TokenType::LeftParen),
            ')' => self.add_token(TokenType::RightParen),
            '{' => self.add_token(TokenType::LeftBrace),
            '}' => self.add_token(TokenType::RightBrace),
            ';' => self.add_token(TokenType::Semicolon),
            '-' => self.add_token(TokenType::Minus),
            '+' => self.add_token(TokenType::Plus),
            '*' => self.add_token(TokenType::Star),
            '/' => self.add_token(TokenType::Slash),
            '!' => {
                let token = if self.match_char('=') { TokenType::BangEqual } else { TokenType::Bang };
                self.add_token(token);
            }
            '=' => {
                let token = if self.match_char('=') { TokenType::EqualEqual } else { TokenType::Equal };
                self.add_token(token);
            }
            '<' => {
                let token = if self.match_char('=') { TokenType::LessEqual } else { TokenType::Less };
                self.add_token(token);
            }
            '>' => {
                let token = if self.match_char('=') { TokenType::GreaterEqual } else { TokenType::Greater };
                self.add_token(token);
            }
            ' ' | '\r' | '\t' => {}
            '\n' => self.line += 1,
            '"' => self.string(),
            _ => {
                if c.is_ascii_digit() {
                    self.number();
                } else if c.is_alphabetic() || c == '_' {
                    self.identifier();
                } else if c != '\0' {
                    panic!("Unexpected character: '{}' at line {}", c, self.line);
                }
            }
        }
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.is_at_end() || self.chars[self.current] != expected {
            return false;
        }
        self.current += 1;
        true
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

        let lexeme: String = self.chars[self.start..self.current].iter().collect();
        match lexeme.parse::<f64>() {
            Ok(value) => self.add_token(TokenType::Number(value)),
            Err(_) => panic!("Invalid number format: '{}' at line {}", lexeme, self.line),
        }
    }

    fn advance(&mut self) -> char {
        if self.is_at_end() {
            return '\0';
        }
        let current_char = self.chars[self.current];
        self.current += 1;
        current_char
    }

    fn add_token(&mut self, token_type: TokenType) {
        let lexeme: String = self.chars[self.start..self.current].iter().collect();
        self.tokens.push(Token {
            token_type,
            lexeme,
            line: self.line,
        });
    }

    fn string(&mut self) {
        while self.peek() != '"' && !self.is_at_end() {
            if self.peek() == '\n' {
                self.line += 1;
            }
            self.advance();
        }

        if self.is_at_end() {
            panic!("Unterminated string at line {}", self.line);
        }

        self.advance();
        let value: String = self.chars[self.start + 1..self.current - 1].iter().collect();
        self.add_token(TokenType::String(value));
    }

    fn identifier(&mut self) {
        while self.peek().is_alphanumeric() || self.peek() == '_' {
            self.advance();
        }

        let text: String = self.chars[self.start..self.current].iter().collect();
        let token_type = self.keywords.get(&text).cloned().unwrap_or(TokenType::Identifier);
        self.add_token(token_type);
    }

    fn peek(&self) -> char {
        if self.is_at_end() {
            '\0'
        } else {
            self.chars[self.current]
        }
    }

    fn peek_next(&self) -> char {
        if self.current + 1 >= self.chars.len() {
            '\0'
        } else {
            self.chars[self.current + 1]
        }
    }
}
