use crate::parser::Parser;
use crate::precedence::Precedence;
use core::ast::Expr;
use core::{Token, TokenType};

pub fn expression(parser: &mut Parser) -> Expr {
    parse_precedence(parser, Precedence::Assignment as u8)
}

pub fn parse_precedence(parser: &mut Parser, precedence: u8) -> Expr {
    // Universal recursion depth guard — prevents stack overflow from deeply nested
    // fuzz inputs like `((((...` or `a.b.b.b.b....` chained operators.
    if !parser.push_depth(None) {
        return Expr::Literal(core::ast::ArtValue::none());
    }
    let mut left = parse_prefix(parser);

    while precedence < parser.peek_precedence() {
        let operator = parser.advance();
        left = parse_infix(parser, left, operator);
    }
    parser.pop_depth();
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
        TokenType::String(s) => Expr::Literal(core::ast::ArtValue::String(core::intern_arc(&s))),
        TokenType::InterpolatedString(s) => parser.parse_interpolated_string(s),
        TokenType::True => Expr::Literal(core::ast::ArtValue::Bool(true)),
        TokenType::False => Expr::Literal(core::ast::ArtValue::Bool(false)),
        TokenType::None => Expr::Literal(core::ast::ArtValue::none()),
        TokenType::LeftBracket => {
            let mut elements = Vec::new();
            if !parser.check(&TokenType::RightBracket) {
                while !parser.is_at_end() && !parser.check(&TokenType::RightBracket) {
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
            // Guard against excessively deep nesting (e.g., from fuzz inputs like `((((`).
            if parser.is_at_end() || !parser.push_depth(Some(&token)) {
                parser.diagnostics.push(diagnostics::Diagnostic::new(
                    diagnostics::DiagnosticKind::Parse,
                    "Unclosed '(' — reached end of input or nesting limit.".to_string(),
                    diagnostics::Span::new(token.start, token.end, token.line, token.col),
                ));
                return Expr::Literal(core::ast::ArtValue::none());
            }

            if parser.check(&TokenType::RightParen) {
                // Empty tuple ()
                parser.advance();
                parser.pop_depth();
                return Expr::Tuple(Vec::new());
            }

            let expr = expression(parser);

            if parser.match_token(TokenType::Comma) {
                // It's a tuple with at least 1 element
                let mut elements = vec![expr];

                if !parser.check(&TokenType::RightParen) {
                    while !parser.is_at_end() && !parser.check(&TokenType::RightParen) {
                        elements.push(expression(parser));
                        if !parser.match_token(TokenType::Comma) {
                            break;
                        }
                    }
                }

                parser.consume(TokenType::RightParen, "Expect ')' after tuple elements.");
                parser.pop_depth();
                return Expr::Tuple(elements);
            }

            // Just a grouping expression (expr)
            parser.pop_depth();
            parser.consume(TokenType::RightParen, "Expect ')' after expression.");
            Expr::Grouping {
                expression: Box::new(expr),
            }
        }
        TokenType::Identifier => Expr::Variable { name: token },
        TokenType::Spawn => {
            // parse spawn actor { ... } as an expression returning an actor id
            // consume 'actor' and the block
            // Note: parser.advance() already consumed the 'spawn' token
            if parser.check(&TokenType::Actor) {
                // consume 'actor'
                parser.advance();
                parser.consume(TokenType::LeftBrace, "Expect '{' to start actor body.");
                let body = crate::statements::block(parser);
                Expr::SpawnActor { body }
            } else {
                parser.diagnostics.push(diagnostics::Diagnostic::new(
                    diagnostics::DiagnosticKind::Parse,
                    "Expect 'actor' after 'spawn'".to_string(),
                    diagnostics::Span::new(token.start, token.end, token.line, token.col),
                ));
                Expr::Literal(core::ast::ArtValue::none())
            }
        }
        TokenType::Weak => {
            // próximo é expressão de menor precedência que unary
            let inner = parse_precedence(parser, Precedence::Unary as u8);
            Expr::Weak(Box::new(inner))
        }
        TokenType::Unowned => {
            let inner = parse_precedence(parser, Precedence::Unary as u8);
            Expr::Unowned(Box::new(inner))
        }
        TokenType::Bang | TokenType::Minus => {
            let right = parse_precedence(parser, Precedence::Unary as u8);
            Expr::Unary {
                operator: token,
                right: Box::new(right),
            }
        }
        TokenType::Dot => {
            let variant_name =
                parser.consume(TokenType::Identifier, "Expect enum variant name after '.'");
            if parser.match_token(TokenType::LeftParen) {
                let mut values = Vec::new();
                if !parser.check(&TokenType::RightParen) {
                    while !parser.is_at_end() && !parser.check(&TokenType::RightParen) {
                        values.push(expression(parser));
                        if !parser.match_token(TokenType::Comma) {
                            break;
                        }
                    }
                }
                parser.consume(
                    TokenType::RightParen,
                    "Expect ')' after enum variant values.",
                );
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
        _ => {
            parser.diagnostics.push(diagnostics::Diagnostic::new(
                diagnostics::DiagnosticKind::Parse,
                format!("Unexpected token in expression: {:?}", token.token_type),
                diagnostics::Span::new(token.start, token.end, token.line, token.col),
            ));
            Expr::Literal(core::ast::ArtValue::none())
        }
    }
}

pub fn parse_infix(parser: &mut Parser, left: Expr, operator: Token) -> Expr {
    let precedence = parser.token_precedence(&operator.token_type);
    match operator.token_type {
        TokenType::ColonColon => {
            parser.consume(
                TokenType::Less,
                "Expect '<' after '::' for generic arguments.",
            );
            let mut type_args = Vec::new();
            if !parser.check(&TokenType::Greater) {
                while !parser.is_at_end() && !parser.check(&TokenType::Greater) {
                    type_args.push(parser.parse_type());
                    if !parser.match_token(TokenType::Comma) {
                        break;
                    }
                }
            }
            parser.consume(
                TokenType::Greater,
                "Expect '>' after generic type arguments.",
            );
            parser.consume(
                TokenType::LeftParen,
                "Expect '(' after generic arguments to call the function.",
            );
            let call_expr = finish_call(parser, left);
            match call_expr {
                Expr::Call {
                    callee,
                    type_args: _,
                    arguments,
                } => Expr::Call {
                    callee,
                    type_args: Some(type_args),
                    arguments,
                },
                other => other,
            }
        }
        TokenType::LeftParen => finish_call(parser, left),
        TokenType::Dot => {
            let ident = parser.consume(TokenType::Identifier, "Expect identifier after '.'");
            // Se left é Variable e próximo é '(' trata como EnumInit nomeado
            if let Expr::Variable {
                name: enum_name_tok,
            } = left.clone()
            {
                let is_type_like = enum_name_tok
                    .lexeme
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false);
                if is_type_like {
                    if parser.check(&TokenType::LeftParen) {
                        parser.advance(); // consume '('
                        let mut values = Vec::new();
                        if !parser.check(&TokenType::RightParen) {
                            while !parser.is_at_end() && !parser.check(&TokenType::RightParen) {
                                values.push(expression(parser));
                                if !parser.match_token(TokenType::Comma) {
                                    break;
                                }
                            }
                        }
                        parser.consume(
                            TokenType::RightParen,
                            "Expect ')' after enum variant values.",
                        );
                        return Expr::EnumInit {
                            name: Some(enum_name_tok),
                            variant: ident,
                            values,
                        };
                    } else {
                        // Variant sem payload
                        return Expr::EnumInit {
                            name: Some(enum_name_tok),
                            variant: ident,
                            values: Vec::new(),
                        };
                    }
                } else {
                    return Expr::FieldAccess {
                        object: Box::new(left),
                        field: ident,
                    };
                }
            }
            Expr::FieldAccess {
                object: Box::new(left),
                field: ident,
            }
        }
        TokenType::Question => {
            // Se left é Weak(...) ou já produziu algo que deve virar WeakUpgrade
            Expr::WeakUpgrade(Box::new(left))
        }
        TokenType::Bang => {
            // Postfix unowned access
            Expr::UnownedAccess(Box::new(left))
        }
        TokenType::As => {
            let type_name = parser.parse_type();
            Expr::Cast {
                object: Box::new(left),
                target_type: type_name,
            }
        }
        TokenType::And | TokenType::Or => {
            let right = parse_precedence(parser, precedence);
            Expr::Logical {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            }
        }
        TokenType::PipeGreater => {
            let right = parse_precedence(parser, precedence);
            match right {
                Expr::Call {
                    callee,
                    type_args,
                    mut arguments,
                } => {
                    arguments.insert(0, left);
                    Expr::Call {
                        callee,
                        type_args,
                        arguments,
                    }
                }
                other => Expr::Call {
                    callee: Box::new(other),
                    type_args: None,
                    arguments: vec![left],
                },
            }
        }
        _ => {
            let right = parse_precedence(parser, precedence);
            Expr::Binary {
                left: Box::new(left),
                operator,
                right: Box::new(right),
            }
        }
    }
}

pub fn finish_call(parser: &mut Parser, callee: Expr) -> Expr {
    let mut arguments = Vec::new();
    if !parser.check(&TokenType::RightParen) {
        while !parser.is_at_end() && !parser.check(&TokenType::RightParen) {
            arguments.push(expression(parser));
            if !parser.match_token(TokenType::Comma) {
                break;
            }
        }
    }
    parser.consume(TokenType::RightParen, "Expect ')' after arguments.");
    Expr::Call {
        callee: Box::new(callee),
        type_args: None,
        arguments,
    }
}
