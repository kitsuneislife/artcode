use core::{Token, TokenType};
use core::ast::{Expr, Program, Stmt};
use crate::statements;
use crate::expressions;
use crate::precedence::Precedence;

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
            statements.push(self.declaration());
        }
        statements
    }

    pub fn declaration(&mut self) -> Stmt {
        if self.match_token(TokenType::Struct) {
            self.struct_declaration()
        } else if self.match_token(TokenType::Enum) {
            self.enum_declaration()
        } else if self.match_token(TokenType::Let) {
            self.let_declaration()
        } else if self.match_token(TokenType::Func) {
            self.function_declaration()
        } else {
            self.statement()
        }
    }

    pub fn struct_declaration(&mut self) -> Stmt {
        let name = self.consume(TokenType::Identifier, "Expect struct name.");
        self.consume(TokenType::LeftBrace, "Expect '{' after struct name.");
        let mut fields = Vec::new();
        if !self.check(&TokenType::RightBrace) {
            loop {
                let field_name = self.consume(TokenType::Identifier, "Expect field name.");
                self.consume(TokenType::Colon, "Expect ':' after field name.");
                let ty = self.parse_type();
                fields.push((field_name, ty));
                if !self.match_token(TokenType::Comma) || self.check(&TokenType::RightBrace) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightBrace, "Expect '}' after struct fields.");
        Stmt::StructDecl { name, fields }
    }

    pub fn enum_declaration(&mut self) -> Stmt {
        let name = self.consume(TokenType::Identifier, "Expect enum name.");
        self.consume(TokenType::LeftBrace, "Expect '{' after enum name.");
        let mut variants = Vec::new();
        while !self.check(&TokenType::RightBrace) && !self.is_at_end() {
            let variant_name = self.consume(TokenType::Identifier, "Expect variant name.");
            let params = if self.match_token(TokenType::LeftParen) {
                let mut param_types = Vec::new();
                if !self.check(&TokenType::RightParen) {
                    loop {
                        param_types.push(self.parse_type());
                        if !self.match_token(TokenType::Comma) {
                            break;
                        }
                    }
                }
                self.consume(TokenType::RightParen, "Expect ')' after variant types.");
                Some(param_types)
            } else {
                None
            };
            variants.push((variant_name, params));
            if !self.match_token(TokenType::Comma) || self.check(&TokenType::RightBrace) {
                break;
            }
        }
        self.consume(TokenType::RightBrace, "Expect '}' after enum variants.");
        Stmt::EnumDecl { name, variants }
    }

    pub fn statement(&mut self) -> Stmt {
        statements::statement(self)
    }

    pub fn let_declaration(&mut self) -> Stmt {
        statements::let_declaration(self)
    }

    pub fn if_statement(&mut self) -> Stmt {
        statements::if_statement(self)
    }

    pub fn block(&mut self) -> Vec<Stmt> {
        statements::block(self)
    }

    pub fn expression(&mut self) -> Expr {
        expressions::expression(self)
    }

    pub fn parse_precedence(&mut self, precedence: u8) -> Expr {
        expressions::parse_precedence(self, precedence)
    }

    pub fn parse_prefix(&mut self) -> Expr {
        expressions::parse_prefix(self)
    }

    pub fn parse_infix(&mut self, left: Expr, operator: Token) -> Expr {
        expressions::parse_infix(self, left, operator)
    }

    pub fn finish_call(&mut self, callee: Expr) -> Expr {
        expressions::finish_call(self, callee)
    }

    pub fn peek_precedence(&self) -> u8 {
        self.token_precedence(&self.peek().token_type)
    }

    pub fn token_precedence(&self, token_type: &TokenType) -> u8 {
        match token_type {
            TokenType::And => Precedence::And as u8,
            TokenType::Or => Precedence::Or as u8,
            TokenType::EqualEqual | TokenType::BangEqual => Precedence::Equality as u8,
            TokenType::Greater | TokenType::GreaterEqual | TokenType::Less | TokenType::LessEqual => Precedence::Comparison as u8,
            TokenType::Plus | TokenType::Minus => Precedence::Term as u8,
            TokenType::Star | TokenType::Slash => Precedence::Factor as u8,
            TokenType::Dot => Precedence::Call as u8,
            TokenType::As => Precedence::Call as u8,
            TokenType::Question => Precedence::Try as u8,
            _ => Precedence::None as u8,
        }
    }

    pub fn match_token(&mut self, tt: TokenType) -> bool {
        if self.check(&tt) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn consume(&mut self, tt: TokenType, message: &str) -> Token {
        if self.check(&tt) {
            return self.advance();
        }
        panic!("{}: Expected {:?}, got {:?}", message, tt, self.peek().token_type);
    }

    pub fn check(&self, tt: &TokenType) -> bool {
        if self.is_at_end() { return false; }
        std::mem::discriminant(&self.peek().token_type) == std::mem::discriminant(tt)
    }

    pub fn advance(&mut self) -> Token {
        if !self.is_at_end() { self.current += 1; }
        self.previous()
    }

    pub fn is_at_end(&self) -> bool {
        matches!(self.peek().token_type, TokenType::Eof)
    }

    pub fn peek(&self) -> Token {
        self.tokens[self.current].clone()
    }

    pub fn previous(&self) -> Token {
        self.tokens[self.current - 1].clone()
    }

    pub fn parse_type(&mut self) -> String {
        let mut type_str = String::new();
        if self.match_token(TokenType::LeftBracket) {
            type_str.push('[');
            type_str.push_str(&self.parse_type());
            self.consume(TokenType::RightBracket, "Expect ']' after array element type.");
            type_str.push(']');
        } else {
            let type_name = self.consume(TokenType::Identifier, "Expect type name.");
            type_str.push_str(&type_name.lexeme);
            if self.match_token(TokenType::Less) {
                type_str.push('<');
                loop {
                    type_str.push_str(&self.parse_type());
                    if self.match_token(TokenType::Comma) {
                        type_str.push_str(", ");
                    } else if self.match_token(TokenType::Greater) {
                        type_str.push('>');
                        break;
                    } else {
                        panic!("Expect ',' or '>' in generic type parameters.");
                    }
                }
            }
        }
        type_str
    }

    pub fn function_declaration(&mut self) -> Stmt {
        let name = self.consume(TokenType::Identifier, "Expect function name.");
        self.consume(TokenType::LeftParen, "Expect '(' after function name.");
        let mut params = Vec::new();
        if !self.check(&TokenType::RightParen) {
            loop {
                let param_name = self.consume(TokenType::Identifier, "Expect parameter name.");
                let param_type = if self.match_token(TokenType::Colon) {
                    Some(self.parse_type())
                } else {
                    None
                };
                params.push(core::ast::FunctionParam {
                    name: param_name,
                    ty: param_type,
                });
                if !self.match_token(TokenType::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightParen, "Expect ')' after parameters.");
        let return_type = if self.match_token(TokenType::Arrow) {
            Some(self.parse_type())
        } else {
            None
        };
        self.consume(TokenType::LeftBrace, "Expect '{' before function body.");
        let body = Box::new(Stmt::Block { statements: self.block() });
        Stmt::Function {
            name,
            params,
            return_type,
            body,
        }
    }
}