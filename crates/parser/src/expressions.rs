use core::{Token, TokenType};
use core::ast::Expr;
use crate::parser::Parser;
use crate::precedence::Precedence;

pub fn expression(parser: &mut Parser) -> Expr {
    parse_precedence(parser, Precedence::Assignment as u8)
}

pub fn parse_precedence(parser: &mut Parser, precedence: u8) -> Expr {
    let mut left = parse_prefix(parser);
    loop {
        let peek_prec = parser.peek_precedence();
        if precedence > peek_prec || parser.is_at_end() {
            break;
        }
        let operator = parser.advance();
        if operator.token_type == TokenType::Question {
            left = Expr::Try(Box::new(left));
            continue;
        }
        left = parse_infix(parser, left, operator);
    }
    left
}

pub fn parse_prefix(parser: &mut Parser) -> Expr {
    let token = parser.advance();
    match token.token_type {
        TokenType::Number(n) => {
            let art_val = if n.fract() == 0.0 {
                core::ast::ArtValue::Int(n as i64)
            } else {
                core::ast::ArtValue::Float(n)
            };
            Expr::Literal(art_val)
        }
        TokenType::String(s) => Expr::Literal(core::ast::ArtValue::String(s)),
        TokenType::True => Expr::Literal(core::ast::ArtValue::Bool(true)),
        TokenType::False => Expr::Literal(core::ast::ArtValue::Bool(false)),
        TokenType::None => Expr::Literal(core::ast::ArtValue::Optional(Box::new(None))),
        TokenType::Dot => {
            // Handle dot notation for enum variants like .Ok(value) or .Err(error)
            let variant_name = parser.consume(TokenType::Identifier, "Expect enum variant name after '.'");
            // Check if it's a variant call with arguments
            if parser.match_token(TokenType::LeftParen) {
                let mut values = Vec::new();
                if !parser.check(&TokenType::RightParen) {
                    loop {
                        values.push(parse_precedence(parser, Precedence::Assignment as u8));
                        if !parser.match_token(TokenType::Comma) {
                            break;
                        }
                    }
                }
                parser.consume(TokenType::RightParen, "Expect ')' after enum variant values.");
                let enum_name = Token {
                    token_type: TokenType::Identifier,
                    lexeme: "Result".to_string(),
                    line: variant_name.line,
                };
                Expr::EnumInit {
                    name: enum_name,
                    variant: variant_name,
                    values,
                }
            } else {
                let enum_name = Token {
                    token_type: TokenType::Identifier,
                    lexeme: "Result".to_string(),
                    line: variant_name.line,
                };
                Expr::EnumInit {
                    name: enum_name,
                    variant: variant_name,
                    values: Vec::new(),
                }
            }
        }
        TokenType::LeftBracket => {
            let mut elements = Vec::new();
            if !parser.check(&TokenType::RightBracket) {
                loop {
                    elements.push(expression(parser));
                    if parser.match_token(TokenType::Comma) {
                        continue;
                    }
                    break;
                }
            }
            parser.consume(TokenType::RightBracket, "Expect ']' after array elements.");
            let arr = elements.into_iter().map(|e| match e {
                Expr::Literal(v) => v,
                other => panic!("Only literal values allowed in arrays for now: {:?}", other),
            }).collect();
            Expr::Literal(core::ast::ArtValue::Array(arr))
        }
        TokenType::LeftParen => {
            let expr = expression(parser);
            parser.consume(TokenType::RightParen, "Expect ')' after expression.");
            let mut node = Expr::Grouping { expression: Box::new(expr) };
            while parser.check(&TokenType::Question) {
                parser.advance();
                node = Expr::Try(Box::new(node));
            }
            node
        }
        TokenType::Identifier => {
            let ident_token = token.clone();
            if parser.check(&TokenType::LeftBrace) {
                parser.advance();
                let mut fields = Vec::new();
                while !parser.check(&TokenType::RightBrace) {
                    if parser.check(&TokenType::Identifier) {
                        let field_name = parser.advance();
                        parser.consume(TokenType::Colon, "Expect ':' after field name.");
                        let value = expression(parser);
                        fields.push((field_name, value));
                        if !parser.check(&TokenType::RightBrace) {
                            parser.match_token(TokenType::Comma);
                        }
                    } else {
                        panic!("Expect field name, got {:?}", parser.peek().token_type);
                    }
                }
                parser.consume(TokenType::RightBrace, "Expect '}' after struct fields.");
                Expr::StructInit { name: ident_token, fields }
            } else if parser.check(&TokenType::LeftParen) {
                parser.advance();
                finish_call(parser, Expr::Variable { name: token })
            } else {
                Expr::Variable { name: token }
            }
        }
        TokenType::Bang | TokenType::Minus => {
            let right = parse_precedence(parser, Precedence::Unary as u8);
            Expr::Unary { operator: token, right: Box::new(right) }
        }
        TokenType::Func => {
            Expr::Variable { name: Token { token_type: TokenType::Identifier, lexeme: "func".to_string(), line: token.line } }
        }
        TokenType::Return => {
            Expr::Variable { name: Token { token_type: TokenType::Identifier, lexeme: "return".to_string(), line: token.line } }
        }
        _ => panic!("Unexpected token: {:?}", token),
    }
}

pub fn parse_infix(parser: &mut Parser, left: Expr, operator: Token) -> Expr {
    let precedence = parser.token_precedence(&operator.token_type);
    match operator.token_type {
        TokenType::And | TokenType::Or => {
            let right = parse_precedence(parser, precedence);
            Expr::Logical { left: Box::new(left), operator, right: Box::new(right) }
        }
        TokenType::Dot => {
            let field_name;
            if parser.check(&TokenType::Identifier) {
                field_name = parser.advance();
            } else if parser.match_token(TokenType::None) {
                field_name = Token {
                    token_type: TokenType::Identifier,
                    lexeme: "None".to_string(),
                    line: parser.previous().line,
                };
            } else {
                panic!("Expect field name after '.', got {:?}", parser.peek().token_type);
            }
            if parser.check(&TokenType::LeftParen) {
                parser.advance();
                let mut arguments = Vec::new();
                if !parser.check(&TokenType::RightParen) {
                    arguments.push(expression(parser));
                    while parser.match_token(TokenType::Comma) {
                        arguments.push(expression(parser));
                    }
                }
                parser.consume(TokenType::RightParen, "Expect ')' after method arguments.");
                let method_access = Expr::FieldAccess {
                    object: Box::new(left),
                    field: field_name,
                };
                let mut node = Expr::Call {
                    callee: Box::new(method_access),
                    arguments,
                };
                while parser.check(&TokenType::Question) {
                    parser.advance();
                    node = Expr::Try(Box::new(node));
                }
                node
            } else {
                let mut node = Expr::FieldAccess {
                    object: Box::new(left),
                    field: field_name,
                };
                while parser.check(&TokenType::Question) {
                    parser.advance();
                    node = Expr::Try(Box::new(node));
                }
                node
            }
        }
        _ => {
            let right = parse_precedence(parser, precedence + 1);
            Expr::Binary { left: Box::new(left), operator, right: Box::new(right) }
        }
    }
}

pub fn finish_call(parser: &mut Parser, callee: Expr) -> Expr {
    let mut arguments = Vec::new();
    if let Expr::Variable { name } = &callee {
        if name.lexeme == "println" && parser.check(&TokenType::Identifier) {
            let id = parser.peek();
            if id.lexeme == "f" {
                parser.advance();
                if let TokenType::String(_) = &parser.peek().token_type {
                    let string_token = parser.advance();
                    arguments.push(Expr::Literal(core::ast::ArtValue::String(
                        if let TokenType::String(content) = string_token.token_type {
                            content
                        } else {
                            "".to_string()
                        }
                    )));
                    parser.consume(TokenType::RightParen, "Expect ')' after string interpolation.");
                    let node = Expr::Call { callee: Box::new(callee), arguments };
                    return node;
                }
            }
        }
    }
    if !parser.check(&TokenType::RightParen) {
        arguments.push(expression(parser));
        while parser.match_token(TokenType::Comma) {
            arguments.push(expression(parser));
        }
    }
    parser.consume(TokenType::RightParen, "Expect ')' after arguments.");
    let mut node = Expr::Call { callee: Box::new(callee), arguments };
    while parser.check(&TokenType::Question) {
        parser.advance();
        node = Expr::Try(Box::new(node));
    }

    node
}