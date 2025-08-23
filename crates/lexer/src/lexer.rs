use core::{Token, TokenType};
use diagnostics::{DiagResult, Diagnostic, DiagnosticKind, Span};

pub struct Lexer {
    source: Vec<char>,
    tokens: Vec<Token>,
    start: usize,
    current: usize,
    line: usize,
    line_start: usize,
}

impl Lexer {
    pub fn new(source: String) -> Self {
        Lexer {
            source: source.chars().collect(),
            tokens: Vec::new(),
            start: 0,
            current: 0,
            line: 1,
            line_start: 0,
        }
    }
    pub fn scan_tokens(&mut self) -> DiagResult<Vec<Token>> {
        while !self.is_at_end() {
            self.start = self.current;
            self.scan_token()?;
        }
        self.tokens.push(self.make_token(TokenType::Eof));
        Ok(self.tokens.clone())
    }

    fn scan_token(&mut self) -> DiagResult<()> {
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
            '_' => {
                // If underscore is followed by alphanumeric, treat as identifier (e.g. _tmp).
                if self.peek().is_alphanumeric() {
                    self.identifier();
                } else {
                    self.add_token(TokenType::Underscore);
                }
            }
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
            '\n' => {
                self.line += 1;
                self.line_start = self.current;
            }
            '"' => self.string()?,
            'f' => {
                // <<< LÓGICA PARA 'f'
                if self.peek() == '"' {
                    self.advance(); // Consome o "
                    self.interpolated_string()?;
                } else {
                    self.identifier();
                }
            }
            c if c.is_ascii_digit() => self.number()?,
            c if c.is_alphabetic() || c == '_' => self.identifier(),
            _ => return Err(self.error_current("Unexpected character")),
        }
        Ok(())
    }

    fn interpolated_string(&mut self) -> DiagResult<()> {
        while self.peek() != '"' && !self.is_at_end() {
            if self.peek() == '\n' {
                self.line += 1;
                self.line_start = self.current + 1;
            }
            self.advance();
        }

        if self.is_at_end() {
            return Err(self.error_current("Unterminated string"));
        }

        self.advance(); // The closing ".

        let value: String = self.source[self.start + 2..self.current - 1]
            .iter()
            .collect();
        self.add_token(TokenType::InterpolatedString(value));
        Ok(())
    }

    fn string(&mut self) -> DiagResult<()> {
        while self.peek() != '"' && !self.is_at_end() {
            if self.peek() == '\n' {
                self.line += 1;
                self.line_start = self.current + 1;
            }
            self.advance();
        }

        if self.is_at_end() {
            return Err(self.error_current("Unterminated string"));
        }

        self.advance();

        let value: String = self.source[self.start + 1..self.current - 1]
            .iter()
            .collect();
        self.add_token(TokenType::String(value));
        Ok(())
    }

    fn number(&mut self) -> DiagResult<()> {
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
        match value.parse::<f64>() {
            Ok(num) => {
                self.add_token(TokenType::Number(num));
                Ok(())
            }
            Err(_) => {
                // Valor capturado corresponde ao padrão de dígitos (e opcional ponto + dígitos), então erro aqui é raro;
                // ainda assim, geramos diagnóstico ao invés de panic para robustez.
                let col = if self.start >= self.line_start {
                    self.start - self.line_start + 1
                } else {
                    1
                };
                Err(Diagnostic::new(
                    DiagnosticKind::Lex,
                    "Invalid number literal",
                    Span::new(self.start, self.current, self.line, col),
                ))
            }
        }
    }

    fn identifier(&mut self) {
        while self.peek().is_alphanumeric() || self.peek() == '_' {
            self.advance();
        }
        let text: String = self.source[self.start..self.current].iter().collect();
        let token_type = match text.as_str() {
            "let" => TokenType::Let,
            "if" => TokenType::If,
            "else" => TokenType::Else,
            "true" => TokenType::True,
            "false" => TokenType::False,
            "struct" => TokenType::Struct,
            "enum" => TokenType::Enum,
            "and" => TokenType::And,
            "or" => TokenType::Or,
            "match" => TokenType::Match,
            "case" => TokenType::Case,
            "func" => TokenType::Func,
            "performant" => TokenType::Performant,
            "return" => TokenType::Return,
            "none" => TokenType::None,
            "as" => TokenType::As,
            "weak" => TokenType::Weak,
            "unowned" => TokenType::Unowned,
            _ => TokenType::Identifier,
        };
        self.add_token(token_type);
    }

    fn add_token(&mut self, token_type: TokenType) {
        self.tokens.push(self.make_token(token_type));
    }

    fn make_token(&self, token_type: TokenType) -> Token {
        let text: String = self.source[self.start..self.current].iter().collect();
        let col = if self.start >= self.line_start {
            self.start - self.line_start + 1
        } else {
            1
        }; // 1-based, proteção contra overflow
        // Token::new já internará se aplicável.
        Token::new(token_type, text, self.line, col, self.start, self.current)
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

    fn error_current(&self, msg: &str) -> Diagnostic {
        let col = self.current - self.line_start + 1;
        Diagnostic::new(
            DiagnosticKind::Lex,
            msg,
            Span::new(self.current, self.current + 1, self.line, col),
        )
    }
}
