use core::{TokenType};
use core::ast::{Stmt, MatchPattern, ArtValue, Expr};
use crate::parser::Parser;

pub fn statement(parser: &mut Parser) -> Stmt {
    if parser.check(&TokenType::Match) {
        return match_statement(parser);
    }
    if parser.check(&TokenType::If) {
        return if_statement(parser);
    }
    if parser.check(&TokenType::Return) {
        return return_statement(parser);
    }
    if parser.check(&TokenType::LeftBrace) {
        parser.advance();
        return Stmt::Block { statements: block(parser) };
    }

    let expr = parser.expression();

    if parser.match_token(TokenType::LeftBrace) {
        if let Expr::Variable { name } = expr {
            let mut fields = Vec::new();
            while !parser.check(&TokenType::RightBrace) {
                if parser.check(&TokenType::Identifier) {
                    let field_name = parser.advance();
                    parser.consume(TokenType::Colon, "Expect ':' after field name.");
                    let value = parser.expression();
                    fields.push((field_name, value));
                    if !parser.check(&TokenType::RightBrace) {
                        parser.match_token(TokenType::Comma);
                    }
                } else {
                    let p = parser.peek();
                    parser.diagnostics.push(diagnostics::Diagnostic::new(
                        diagnostics::DiagnosticKind::Parse,
                        format!("Expected field name, got {:?}", p.token_type),
                        diagnostics::Span::new(p.start, p.end, p.line, p.col)));
                    break;
                }
            }
            parser.consume(TokenType::RightBrace, "Expect '}' after struct fields.");
            let struct_init_expr = Expr::StructInit { name, fields };
            return Stmt::Expression(struct_init_expr);
        } else {
            let p = parser.peek();
            parser.diagnostics.push(diagnostics::Diagnostic::new(
                diagnostics::DiagnosticKind::Parse,
                "Invalid expression before '{' for struct initialization.".to_string(),
                diagnostics::Span::new(p.start, p.end, p.line, p.col)));
        }
    }

    parser.match_token(TokenType::Semicolon);
    Stmt::Expression(expr)
}

pub fn let_declaration(parser: &mut Parser) -> Stmt {
    let name = parser.consume(TokenType::Identifier, "Expect variable name.");

    let ty = if parser.match_token(TokenType::Colon) {
        Some(parser.parse_type())
    } else {
        None
    };

    parser.consume(TokenType::Equal, "Expect '=' after variable name or type.");
    let initializer = parser.expression();
    parser.match_token(TokenType::Semicolon);
    Stmt::Let { name, ty, initializer }
}

pub fn if_statement(parser: &mut Parser) -> Stmt {
    parser.consume(TokenType::If, "Expect 'if'.");
    let condition = parser.expression();

    let then_branch = Box::new(statement(parser));
    let else_branch = if parser.match_token(TokenType::Else) {
        Some(Box::new(statement(parser)))
    } else {
        None
    };

    Stmt::If { condition, then_branch, else_branch }
}

pub fn block(parser: &mut Parser) -> Vec<Stmt> {
    let mut statements = Vec::new();
    while !parser.check(&TokenType::RightBrace) && !parser.is_at_end() {
        statements.push(parser.declaration());
    }
    if !parser.is_at_end() {
        parser.consume(TokenType::RightBrace, "Expect '}' after block.");
    }
    statements
}

pub fn match_statement(parser: &mut Parser) -> Stmt {
    parser.consume(TokenType::Match, "Expect 'match'.");
    let expr = parser.expression();
    parser.consume(TokenType::LeftBrace, "Expect '{' after match expression.");
    let mut cases = Vec::new();
    while !parser.check(&TokenType::RightBrace) && !parser.is_at_end() {
        parser.consume(TokenType::Case, "Expect 'case' in match statement.");
        let pattern = parse_pattern(parser);
        parser.consume(TokenType::Colon, "Expect ':' after case pattern.");
        let stmt = statement(parser);
        cases.push((pattern, stmt));
    }
    parser.consume(TokenType::RightBrace, "Expect '}' after match cases.");
    Stmt::Match { expr, cases }
}

pub fn parse_pattern(parser: &mut Parser) -> MatchPattern {
    if parser.match_token(TokenType::Dot) {
        let variant = parser.consume(TokenType::Identifier, "Expect variant name after '.'");
        let mut params = None;
        if parser.match_token(TokenType::LeftParen) {
            let mut param_list = Vec::new();
            if !parser.check(&TokenType::RightParen) {
                loop {
                    param_list.push(parse_pattern(parser));
                    if !parser.match_token(TokenType::Comma) {
                        break;
                    }
                }
            }
            parser.consume(TokenType::RightParen, "Expect ')' after parameters.");
            params = Some(param_list);
        }
        MatchPattern::EnumVariant { variant, params }
    } else if parser.match_token(TokenType::Let) {
        let name = parser.consume(TokenType::Identifier, "Expect variable name after 'let'.");
        MatchPattern::Binding(name)
    } else if parser.match_token(TokenType::Underscore) {
        MatchPattern::Wildcard
    } else if is_literal_token(&parser.peek().token_type) {
        let token = parser.advance();
        match token.token_type {
            TokenType::Number(n) => {
                if n.fract() == 0.0 {
                    MatchPattern::Literal(ArtValue::Int(n as i64))
                } else {
                    MatchPattern::Literal(ArtValue::Float(n))
                }
            },
            TokenType::String(s) => MatchPattern::Literal(ArtValue::String(std::sync::Arc::from(s))),
            TokenType::True => MatchPattern::Literal(ArtValue::Bool(true)),
            TokenType::False => MatchPattern::Literal(ArtValue::Bool(false)),
            TokenType::None => MatchPattern::Literal(ArtValue::none()),
            _ => {
                let p = parser.peek();
                parser.diagnostics.push(diagnostics::Diagnostic::new(
                    diagnostics::DiagnosticKind::Parse,
                    "Unexpected token in pattern".to_string(),
                    diagnostics::Span::new(p.start, p.end, p.line, p.col)));
                MatchPattern::Wildcard
            },
        }
    } else if parser.check(&TokenType::Identifier) {
        let name = parser.consume(TokenType::Identifier, "Expect pattern.");
        MatchPattern::Variable(name)
    } else {
        let p = parser.peek();
        parser.diagnostics.push(diagnostics::Diagnostic::new(
            diagnostics::DiagnosticKind::Parse,
            "Expected pattern after 'case'".to_string(),
            diagnostics::Span::new(p.start, p.end, p.line, p.col)));
        MatchPattern::Wildcard
    }
}

fn is_literal_token(token_type: &TokenType) -> bool {
    matches!(token_type, TokenType::Number(_) | TokenType::String(_) | TokenType::True | TokenType::False | TokenType::None)
}

pub fn return_statement(parser: &mut Parser) -> Stmt {
    parser.consume(TokenType::Return, "Expect 'return'.");
    let value = if parser.check(&TokenType::Semicolon) {
        None
    } else {
        Some(parser.expression())
    };
    parser.match_token(TokenType::Semicolon);
    Stmt::Return { value }
}