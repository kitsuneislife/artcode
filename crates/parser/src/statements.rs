use core::{TokenType, Token}; // Adicionando o import de Token
use core::ast::{Stmt, MatchPattern, ArtValue};
use crate::parser::Parser;

pub fn statement(parser: &mut Parser) -> Stmt {
    if parser.match_token(TokenType::If) {
        return if_statement(parser);
    }
    if parser.match_token(TokenType::Match) {
        return match_statement(parser);
    }
    if parser.match_token(TokenType::Return) {
        return return_statement(parser);
    }
    if parser.match_token(TokenType::LeftBrace) {
        return Stmt::Block { statements: block(parser) };
    }
    let expr = parser.expression();
    parser.match_token(TokenType::Semicolon);
    Stmt::Expression(expr
    )
}

pub fn let_declaration(parser: &mut Parser) -> Stmt {
    let name = parser.consume(TokenType::Identifier, "Expect variable name.");
    parser.consume(TokenType::Equal, "Expect '=' after variable name.");
    let initializer = parser.expression();
    parser.match_token(TokenType::Semicolon);
    Stmt::Let { name, initializer }
}

pub fn if_statement(parser: &mut Parser) -> Stmt {
    // Make parentheses optional - check if there's a left paren
    let condition = if parser.match_token(TokenType::LeftParen) {
        let expr = parser.expression();
        parser.consume(TokenType::RightParen, "Expect ')' after if condition.");
        expr
    } else {
        // No parentheses, just parse the expression directly
        parser.expression()
    };

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
    let expr;
    if parser.check(&TokenType::Identifier) {
        let expr_token = parser.consume(TokenType::Identifier, "Expect variable name after 'match'.");
        expr = core::ast::Expr::Variable { name: expr_token.clone() };
    } else {
        expr = parser.expression();
    }
    parser.consume(TokenType::LeftBrace, "Expect '{' after match expression.");
    let mut cases = Vec::new();
    while !parser.check(&TokenType::RightBrace) && !parser.is_at_end() {
        parser.consume(TokenType::Case, "Expect 'case' in match statement.");
        let pattern = parse_pattern(parser);
        parser.consume(TokenType::Colon, "Expect ':' after case pattern.");
        let stmt = if parser.check(&TokenType::LeftBrace) {
            parser.advance();
            let stmts = block(parser);
            Stmt::Block { statements: stmts }
        } else {
            statement(parser)
        };
        cases.push((pattern, stmt));
    }
    parser.consume(TokenType::RightBrace, "Expect '}' after match cases.");
    Stmt::Match { expr, cases }
}

fn parse_pattern(parser: &mut Parser) -> MatchPattern {
    if parser.match_token(TokenType::Dot) {
        // Tratar tanto Identifier quanto None como nomes de variante válidos
        let variant;
        if parser.check(&TokenType::Identifier) {
            variant = parser.advance();
        } else if parser.match_token(TokenType::None) {
            variant = Token {
                token_type: TokenType::Identifier,
                lexeme: "None".to_string(),
                line: parser.previous().line,
            };
        } else {
            panic!("Expect variant name after '.', got {:?}", parser.peek().token_type);
        }

        let mut params = None;
        if parser.match_token(TokenType::LeftParen) {
            let mut param_list = Vec::new();
            if !parser.check(&TokenType::RightParen) {
                loop {
                    // Handle 'let' bindings in patterns like 'case .Ok(let valor):'
                    if parser.match_token(TokenType::Let) {
                        let param = parser.consume(TokenType::Identifier, "Expect parameter name after 'let'.");
                        param_list.push(param);
                    } else {
                        let param = parser.consume(TokenType::Identifier, "Expect parameter name.");
                        param_list.push(param);
                    }
                    if !parser.match_token(TokenType::Comma) {
                        break;
                    }
                }
            }
            parser.consume(TokenType::RightParen, "Expect ')' after parameters.");
            params = Some(param_list);
        }

        MatchPattern::EnumVariant { variant, params }
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
            TokenType::String(s) => MatchPattern::Literal(ArtValue::String(s)),
            TokenType::True => MatchPattern::Literal(ArtValue::Bool(true)),
            TokenType::False => MatchPattern::Literal(ArtValue::Bool(false)),
            TokenType::None => MatchPattern::Literal(ArtValue::Optional(Box::new(None))),
            _ => panic!("Unexpected token in pattern"),
        }
    } else if parser.check(&TokenType::Identifier) {
        let name = parser.consume(TokenType::Identifier, "Expect pattern.");
        MatchPattern::Variable(name)
    } else {
        panic!("Expected pattern after 'case'");
    }
}

fn is_literal_token(token_type: &TokenType) -> bool {
    match token_type {
        TokenType::Number(_) | TokenType::String(_) | TokenType::True | TokenType::False | TokenType::None => true,
        _ => false,
    }
}

pub fn return_statement(parser: &mut Parser) -> Stmt {
    let value = if parser.check(&TokenType::Semicolon) {
        None
    } else {
        Some(parser.expression())
    };

    parser.match_token(TokenType::Semicolon); // Optional semicolon
    Stmt::Return { value }
}
