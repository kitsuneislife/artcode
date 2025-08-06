
use crate::ast::{Expr, LiteralValue, Program, Stmt};
use crate::lexer::{Token, TokenType};

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
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

    fn expression(&mut self) -> Expr {
        self.call()
    }

    fn call(&mut self) -> Expr {
        let mut expr = self.primary();

        if self.match_token(TokenType::LeftParen) {
            expr = self.finish_call(expr);
        }

        expr
    }

    fn finish_call(&mut self, callee: Expr) -> Expr {
        let mut arguments = Vec::new();
        if !self.check(TokenType::RightParen) {
            arguments.push(self.expression());
        }

        self.consume(TokenType::RightParen, "Esperava ')' após os argumentos.");

        Expr::Call {
            callee: Box::new(callee),
            arguments,
        }
    }

    fn primary(&mut self) -> Expr {
        if self.match_token(TokenType::Identifier) {
            return Expr::Variable { name: self.previous() };
        }
        if let TokenType::String(value) = self.peek().token_type {
            self.advance();
            return Expr::Literal(LiteralValue::String(value));
        }

        self.advance();
        Expr::Literal(LiteralValue::String("error".to_string()))
    }

    fn match_token(&mut self, tt: TokenType) -> bool {
        if self.check(tt) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume(&mut self, tt: TokenType, message: &str) -> Token {
        if self.check(tt) {
            return self.advance();
        }
        panic!("{}", message);
    }

    fn check(&self, tt: TokenType) -> bool {
        if self.is_at_end() { return false; }
        std::mem::discriminant(&self.peek().token_type) == std::mem::discriminant(&tt)
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
