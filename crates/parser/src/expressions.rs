use core::{Token, TokenType};
use core::ast::Expr;
use crate::parser::Parser;
use crate::precedence::Precedence;

pub fn expression(parser: &mut Parser) -> Expr {
    parse_precedence(parser, Precedence::Assignment as u8)
}

pub fn parse_precedence(parser: &mut Parser, precedence: u8) -> Expr {
    let mut left = parse_prefix(parser);

    while precedence < parser.peek_precedence() {
        let operator = parser.advance();
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
    TokenType::InterpolatedString(s) => parser.parse_interpolated_string(s),
        TokenType::True => Expr::Literal(core::ast::ArtValue::Bool(true)),
        TokenType::False => Expr::Literal(core::ast::ArtValue::Bool(false)),
        TokenType::None => Expr::Literal(core::ast::ArtValue::Optional(Box::new(None))),
        TokenType::LeftBracket => {
            let mut elements = Vec::new();
            if !parser.check(&TokenType::RightBracket) {
                loop {
                    elements.push(expression(parser));
                    if !parser.match_token(TokenType::Comma) {
                        break;
                    }
                }
            }
            parser.consume(TokenType::RightBracket, "Expect ']' after array elements.");
            Expr::Array(elements)
        }
        TokenType::LeftParen => {
            let expr = expression(parser);
            parser.consume(TokenType::RightParen, "Expect ')' after expression.");
            Expr::Grouping { expression: Box::new(expr) }
        }
        TokenType::Identifier => Expr::Variable { name: token },
        TokenType::Bang | TokenType::Minus => {
            let right = parse_precedence(parser, Precedence::Unary as u8);
            Expr::Unary { operator: token, right: Box::new(right) }
        }
        TokenType::Dot => {
            let variant_name = parser.consume(TokenType::Identifier, "Expect enum variant name after '.'");
            if parser.match_token(TokenType::LeftParen) {
                let mut values = Vec::new();
                if !parser.check(&TokenType::RightParen) {
                    loop {
                        values.push(expression(parser));
                        if !parser.match_token(TokenType::Comma) {
                            break;
                        }
                    }
                }
                parser.consume(TokenType::RightParen, "Expect ')' after enum variant values.");
                Expr::EnumInit {
                    name: None,
                    variant: variant_name,
                    values,
                }
            } else {
                Expr::EnumInit {
                    name: None,
                    variant: variant_name,
                    values: Vec::new(),
                }
            }
        }
        _ => panic!("Unexpected token for prefix expression: {:?}", token),
    }
}

pub fn parse_infix(parser: &mut Parser, left: Expr, operator: Token) -> Expr {
    let precedence = parser.token_precedence(&operator.token_type);
    match operator.token_type {
        TokenType::LeftParen => finish_call(parser, left),
        TokenType::Dot => {
            let field_name = parser.consume(TokenType::Identifier, "Expect field name after '.'");
            Expr::FieldAccess { object: Box::new(left), field: field_name }
        }
        TokenType::Question => Expr::Try(Box::new(left)),
        TokenType::As => {
            let type_name = parser.parse_type();
            Expr::Cast { object: Box::new(left), target_type: type_name }
        }
        TokenType::And | TokenType::Or => {
            let right = parse_precedence(parser, precedence);
            Expr::Logical { left: Box::new(left), operator, right: Box::new(right) }
        }
        _ => {
            let right = parse_precedence(parser, precedence);
            Expr::Binary { left: Box::new(left), operator, right: Box::new(right) }
        }
    }
}

pub fn finish_call(parser: &mut Parser, callee: Expr) -> Expr {
    let mut arguments = Vec::new();
    if !parser.check(&TokenType::RightParen) {
        loop {
            arguments.push(expression(parser));
            if !parser.match_token(TokenType::Comma) {
                break;
            }
        }
    }
    parser.consume(TokenType::RightParen, "Expect ')' after arguments.");
    Expr::Call { callee: Box::new(callee), arguments }
}