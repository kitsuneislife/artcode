
pub use core::ast;
pub use core::TokenType;

use core::Token;
use ast::{Expr, LiteralValue, Program, Stmt};

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    fn precedence(token_type: &TokenType) -> Option<u8> {
        match token_type {
            TokenType::Plus | TokenType::Minus => Some(1),
            TokenType::Star | TokenType::Slash => Some(2),
            _ => None,
        }
    }

    fn parse_precedence(&mut self, precedence: u8) -> Expr {
        let mut left = self.parse_prefix();

        while let Some(op_precedence) = Self::precedence(&self.peek().token_type) {
            if precedence > op_precedence {
                break;
            }

            let operator = self.advance();
            let right = self.parse_precedence(op_precedence + 1);
            left = Expr::Binary {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            };
        }

        left
    }

    fn parse_prefix(&mut self) -> Expr {
        let token = self.advance();
        match token.token_type {
            TokenType::Number(n) => Expr::Literal(LiteralValue::Number(n)),
            TokenType::String(s) => Expr::Literal(LiteralValue::String(s)),
            TokenType::LeftParen => {
                let expr = self.parse_precedence(1);
                self.consume(TokenType::RightParen, "Esperava ')' após a expressão.");
                Expr::Grouping { expression: Box::new(expr) }
            }
            TokenType::Identifier => {
                if self.match_token(TokenType::LeftParen) {
                    self.finish_call(Expr::Variable { name: token })
                } else {
                    Expr::Variable { name: token }
                }
            }
            _ => panic!("Token inesperado: {:?}", token),
        }
    }

    fn expression(&mut self) -> Expr {
        self.parse_precedence(1)
    }
}


impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, current: 0 }
    }

    pub fn parse(&mut self) -> Program {
        let mut statements = Vec::new();
        while !self.is_at_end() {
            statements.push(self.statement());
        }
        statements
    }

    fn statement(&mut self) -> Stmt {
        let expr = self.expression();
        Stmt::Expression(expr)
    }

    fn finish_call(&mut self, callee: Expr) -> Expr {
        let mut arguments = Vec::new();
        if !self.check(&TokenType::RightParen) {
            arguments.push(self.expression());
        }
        self.consume(TokenType::RightParen, "Esperava ')' após os argumentos.");
        Expr::Call {
            callee: Box::new(callee),
            arguments,
        }
    }

    fn match_token(&mut self, tt: TokenType) -> bool {
        if self.check(&tt) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume(&mut self, tt: TokenType, message: &str) -> Token {
        if self.check(&tt) {
            return self.advance();
        }
        panic!("{}", message);
    }

    fn check(&self, tt: &TokenType) -> bool {
        if self.is_at_end() { return false; }
        std::mem::discriminant(&self.peek().token_type) == std::mem::discriminant(tt)
    }

    fn advance(&mut self) -> Token {
        if !self.is_at_end() { self.current += 1; }
        self.previous()
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek().token_type, TokenType::Eof)
    }

    fn peek(&self) -> Token {
        self.tokens[self.current].clone()
    }

    fn previous(&self) -> Token {
        self.tokens[self.current - 1].clone()
    }
}
