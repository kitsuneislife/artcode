mod precedence;
mod expressions;
mod statements;
mod parser;

pub use core::ast;
pub use core::TokenType;

use core::Token;
use ast::{Expr, LiteralValue, Program, Stmt};

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

enum Precedence {
    None,
    Assignment,
    Or,
    And,
    Equality,
    Comparison,
    Term,
    Factor,
    Unary,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, current: 0 }
    }

    pub fn parse(&mut self) -> Program {
        let mut statements = Vec::new();
        while !self.is_at_end() {
            statements.push(self.declaration());
        }
        statements
    }

    fn declaration(&mut self) -> Stmt {
        if self.match_token(TokenType::Let) {
            self.let_declaration()
        } else {
            self.statement()
        }
    }

    fn let_declaration(&mut self) -> Stmt {
        let name = self.consume(TokenType::Identifier, "Expect variable name.");
        self.consume(TokenType::Equal, "Expect '=' after variable name.");
        let initializer = self.expression();
        self.consume(TokenType::Semicolon, "Expect ';' after variable declaration.");
        Stmt::Let { name, initializer }
    }

    fn statement(&mut self) -> Stmt {
        if self.match_token(TokenType::If) {
            return self.if_statement();
        }
        if self.match_token(TokenType::LeftBrace) {
            return Stmt::Block { statements: self.block() };
        }
        let expr = self.expression();
        self.consume(TokenType::Semicolon, "Expect ';' after expression.");
        Stmt::Expression(expr)
    }

    fn if_statement(&mut self) -> Stmt {
        self.consume(TokenType::LeftParen, "Expect '(' after 'if'.");
        let condition = self.expression();
        self.consume(TokenType::RightParen, "Expect ')' after if condition.");

        let then_branch = Box::new(self.statement());
        let else_branch = if self.match_token(TokenType::Else) {
            Some(Box::new(self.statement()))
        } else {
            None
        };

        Stmt::If { condition, then_branch, else_branch }
    }

    fn block(&mut self) -> Vec<Stmt> {
        let mut statements = Vec::new();
        while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
            statements.push(self.declaration());
        }
        self.consume(TokenType::RightBrace, "Expect '}' after block.");
        statements
    }

    fn expression(&mut self) -> Expr {
        self.parse_precedence(Precedence::Assignment as u8)
    }

    fn parse_precedence(&mut self, precedence: u8) -> Expr {
        let mut left = self.parse_prefix();

        while precedence <= self.peek_precedence() {
            let operator = self.advance();
            left = self.parse_infix(left, operator);
        }
        left
    }

    fn parse_prefix(&mut self) -> Expr {
        let token = self.advance();
        match token.token_type {
            TokenType::Number(n) => Expr::Literal(LiteralValue::Number(n)),
            TokenType::String(s) => Expr::Literal(LiteralValue::String(s)),
            TokenType::True => Expr::Literal(LiteralValue::Bool(true)),
            TokenType::False => Expr::Literal(LiteralValue::Bool(false)),
            TokenType::LeftParen => {
                let expr = self.expression();
                self.consume(TokenType::RightParen, "Expect ')' after expression.");
                Expr::Grouping { expression: Box::new(expr) }
            }
            TokenType::Identifier => {
                if self.match_token(TokenType::LeftParen) {
                    self.finish_call(Expr::Variable { name: token })
                } else {
                    Expr::Variable { name: token }
                }
            }
            TokenType::Bang | TokenType::Minus => {
                let right = self.parse_precedence(Precedence::Unary as u8);
                Expr::Unary { operator: token, right: Box::new(right) }
            }
            _ => panic!("Unexpected token: {:?}", token),
        }
    }

    fn parse_infix(&mut self, left: Expr, operator: Token) -> Expr {
        let precedence = self.token_precedence(&operator.token_type);
        match operator.token_type {
            TokenType::And | TokenType::Or => {
                let right = self.parse_precedence(precedence);
                Expr::Logical { left: Box::new(left), operator, right: Box::new(right) }
            }
            _ => {
                let right = self.parse_precedence(precedence + 1);
                Expr::Binary { left: Box::new(left), operator, right: Box::new(right) }
            }
        }
    }

    fn finish_call(&mut self, callee: Expr) -> Expr {
        let mut arguments = Vec::new();
        if !self.check(&TokenType::RightParen) {
            arguments.push(self.expression());
        }
        self.consume(TokenType::RightParen, "Expect ')' after arguments.");
        Expr::Call { callee: Box::new(callee), arguments }
    }

    fn peek_precedence(&self) -> u8 {
        self.token_precedence(&self.peek().token_type)
    }

    fn token_precedence(&self, token_type: &TokenType) -> u8 {
        match token_type {
            TokenType::And => Precedence::And as u8,
            TokenType::Or => Precedence::Or as u8,
            TokenType::EqualEqual | TokenType::BangEqual => Precedence::Equality as u8,
            TokenType::Greater | TokenType::GreaterEqual | TokenType::Less | TokenType::LessEqual => Precedence::Comparison as u8,
            TokenType::Plus | TokenType::Minus => Precedence::Term as u8,
            TokenType::Star | TokenType::Slash => Precedence::Factor as u8,
            _ => Precedence::None as u8,
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
