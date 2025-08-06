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
                let ty = self.consume(TokenType::Identifier, "Expect type after ':'.");
                fields.push((field_name, ty.lexeme));
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
            let variant_name;
            if self.check(&TokenType::Identifier) {
                variant_name = self.advance();
            } else if self.match_token(TokenType::None) {
                variant_name = Token {
                    token_type: TokenType::Identifier,
                    lexeme: "None".to_string(),
                    line: self.previous().line,
                };
            } else {
                panic!("Expect variant name, got {:?}", self.peek().token_type);
            }

            let params = if self.match_token(TokenType::LeftParen) {
                let mut param_types = Vec::new();

                if !self.check(&TokenType::RightParen) {
                    loop {
                        let ty = self.consume(TokenType::Identifier, "Expect type in variant.");
                        param_types.push(ty.lexeme);

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
            if self.check(&TokenType::RightBrace) {
                break;
            }
            self.consume(TokenType::Comma, "Expect ',' between enum variants.");
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
            let token = self.advance();
            return token;
        }
        panic!("{}", message);
    }

    pub fn check(&self, tt: &TokenType) -> bool {
        if self.is_at_end() { return false; }
        self.tokens_match(&self.peek().token_type, tt)
    }

    fn tokens_match(&self, a: &TokenType, b: &TokenType) -> bool {
        match (a, b) {
            (TokenType::LeftParen, TokenType::LeftParen) => true,
            (TokenType::RightParen, TokenType::RightParen) => true,
            (TokenType::LeftBrace, TokenType::LeftBrace) => true,
            (TokenType::RightBrace, TokenType::RightBrace) => true,
            (TokenType::LeftBracket, TokenType::LeftBracket) => true,
            (TokenType::RightBracket, TokenType::RightBracket) => true,
            (TokenType::Comma, TokenType::Comma) => true,
            (TokenType::None, TokenType::None) => true,
            (TokenType::Minus, TokenType::Minus) => true,
            (TokenType::Plus, TokenType::Plus) => true,
            (TokenType::Slash, TokenType::Slash) => true,
            (TokenType::Star, TokenType::Star) => true,
            (TokenType::Equal, TokenType::Equal) => true,
            (TokenType::Semicolon, TokenType::Semicolon) => true,
            (TokenType::Colon, TokenType::Colon) => true,
            (TokenType::Dot, TokenType::Dot) => true,
            (TokenType::Arrow, TokenType::Arrow) => true,
            (TokenType::Bang, TokenType::Bang) => true,
            (TokenType::BangEqual, TokenType::BangEqual) => true,
            (TokenType::EqualEqual, TokenType::EqualEqual) => true,
            (TokenType::Greater, TokenType::Greater) => true,
            (TokenType::GreaterEqual, TokenType::GreaterEqual) => true,
            (TokenType::Less, TokenType::Less) => true,
            (TokenType::LessEqual, TokenType::LessEqual) => true,
            (TokenType::Let, TokenType::Let) => true,
            (TokenType::If, TokenType::If) => true,
            (TokenType::Else, TokenType::Else) => true,
            (TokenType::True, TokenType::True) => true,
            (TokenType::False, TokenType::False) => true,
            (TokenType::Struct, TokenType::Struct) => true,
            (TokenType::Enum, TokenType::Enum) => true,
            (TokenType::And, TokenType::And) => true,
            (TokenType::Or, TokenType::Or) => true,
            (TokenType::Match, TokenType::Match) => true,
            (TokenType::Case, TokenType::Case) => true,
            (TokenType::Underscore, TokenType::Underscore) => true,
            (TokenType::Func, TokenType::Func) => true,
            (TokenType::Return, TokenType::Return) => true,
            (TokenType::Identifier, TokenType::Identifier) => true,
            (TokenType::String(_), TokenType::String(_)) => true,
            (TokenType::Number(_), TokenType::Number(_)) => true,
            (TokenType::Question, TokenType::Question) => true,
            (TokenType::Eof, TokenType::Eof) => true,
            _ => false,
        }
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

    pub fn function_declaration(&mut self) -> Stmt {
        let name = self.consume(TokenType::Identifier, "Expect function name.");

        self.consume(TokenType::LeftParen, "Expect '(' after function name.");
        let mut params = Vec::new();

        // Parse parameters
        if !self.check(&TokenType::RightParen) {
            loop {
                let param_name = self.consume(TokenType::Identifier, "Expect parameter name.");

                // Check for type annotation
                let param_type = if self.match_token(TokenType::Colon) {
                    // Handle more complex types like [Int] (array types)
                    let mut type_str = String::new();

                    if self.match_token(TokenType::LeftBracket) {
                        // Array type like [Int]
                        type_str.push('[');
                        let element_type = self.consume(TokenType::Identifier, "Expect element type in array.").lexeme;
                        type_str.push_str(&element_type);
                        self.consume(TokenType::RightBracket, "Expect ']' after array element type.");
                        type_str.push(']');
                    } else {
                        // Simple type like Int or String
                        let type_name = self.consume(TokenType::Identifier, "Expect parameter type after ':'.");
                        type_str.push_str(&type_name.lexeme);

                        // Check for generic type like Result<T, E>
                        if self.match_token(TokenType::Less) {
                            type_str.push('<');

                            // Parse type parameters
                            loop {
                                let type_param = self.consume(TokenType::Identifier, "Expect type parameter.").lexeme;
                                type_str.push_str(&type_param);

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

                    Some(type_str)
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

        // Parse optional return type
        let return_type = if self.match_token(TokenType::Arrow) {
            // Handle complex return types like Result<Int, ErroDeDivisao>
            // For now, we'll just collect the full return type as a string
            let mut return_type_str = String::new();

            // Get the base type (like "Result")
            let base_type = self.consume(TokenType::Identifier, "Expect return type after '->'.").lexeme;
            return_type_str.push_str(&base_type);

            // Check for generic parameters
            if self.match_token(TokenType::Less) {
                return_type_str.push('<');

                // Parse the generic parameters
                loop {
                    let type_param = self.consume(TokenType::Identifier, "Expect type parameter.").lexeme;
                    return_type_str.push_str(&type_param);

                    if self.match_token(TokenType::Comma) {
                        return_type_str.push_str(", ");
                    } else if self.match_token(TokenType::Greater) {
                        return_type_str.push('>');
                        break;
                    } else {
                        panic!("Expect ',' or '>' in generic type parameters.");
                    }
                }
            }

            Some(return_type_str)
        } else {
            None
        };

        // Parse function body
        let body = if self.check(&TokenType::LeftBrace) {
            Box::new(Stmt::Block { statements: self.block() })
        } else {
            panic!("Expect '{{' before function body.");
        };

        Stmt::Function {
            name,
            params,
            return_type,
            body,
        }
    }
}