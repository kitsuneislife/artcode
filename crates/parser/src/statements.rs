use crate::parser::Parser;
use core::TokenType;
use core::ast::{ArtValue, Expr, MatchPattern, Stmt};

pub fn statement(parser: &mut Parser) -> Stmt {
    if parser.check(&TokenType::Dollar) {
        return shell_statement(parser);
    }
    if parser.check(&TokenType::Spawn) {
        // syntax: spawn actor { ... }
        parser.advance();
        parser.consume(TokenType::Actor, "Expect 'actor' after 'spawn'.");
        parser.consume(TokenType::LeftBrace, "Expect '{' to start actor body.");
        let body = block(parser);
        return Stmt::SpawnActor { body };
    }
    if parser.check(&TokenType::Match) {
        return match_statement(parser);
    }
    if parser.check(&TokenType::If) {
        return if_statement(parser);
    }
    if parser.check(&TokenType::Try) {
        return try_catch_statement(parser);
    }
    if parser.check(&TokenType::Return) {
        return return_statement(parser);
    }
    if parser.check(&TokenType::While) {
        return while_statement(parser);
    }
    if parser.check(&TokenType::For) {
        return for_statement(parser);
    }
    if parser.check(&TokenType::LeftBrace) {
        parser.advance();
        return Stmt::Block {
            statements: block(parser),
        };
    }

    let expr = parser.expression();

    if parser.match_token(TokenType::LeftBrace) {
        if let Expr::Variable { name } = expr {
            let mut fields = Vec::new();
            while !parser.is_at_end() && !parser.check(&TokenType::RightBrace) {
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
                        diagnostics::Span::new(p.start, p.end, p.line, p.col),
                    ));
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
                diagnostics::Span::new(p.start, p.end, p.line, p.col),
            ));
        }
    }

    parser.match_token(TokenType::Semicolon);
    Stmt::Expression(expr)
}

fn shell_statement(parser: &mut Parser) -> Stmt {
    let dollar = parser.consume(TokenType::Dollar, "Expect '$' before shell command.");
    let line = dollar.line;
    let mut parts: Vec<String> = Vec::new();
    let mut current_arg: Option<(String, usize)> = None;

    while !parser.is_at_end() {
        if parser.check(&TokenType::Semicolon) {
            parser.advance();
            break;
        }
        if parser.check(&TokenType::RightBrace) {
            break;
        }
        if parser.peek().line != line {
            break;
        }

        let tok = parser.advance();
        let (piece, force_new_arg) = match tok.token_type {
            TokenType::String(s) | TokenType::InterpolatedString(s) => (s, true),
            TokenType::Identifier
            | TokenType::Number(_)
            | TokenType::Dot
            | TokenType::Slash
            | TokenType::Colon
            | TokenType::ColonColon
            | TokenType::Equal
            | TokenType::Less
            | TokenType::Greater
            | TokenType::Bang
            | TokenType::Question
            | TokenType::Underscore
            | TokenType::Minus
            | TokenType::Plus
            | TokenType::Star
            | TokenType::Comma => (tok.lexeme, false),
            _ => {
                parser.diagnostics.push(diagnostics::Diagnostic::new(
                    diagnostics::DiagnosticKind::Parse,
                    format!("Unsupported token in shell command: {:?}", tok.token_type),
                    diagnostics::Span::new(tok.start, tok.end, tok.line, tok.col),
                ));
                continue;
            }
        };

        if force_new_arg {
            if let Some((arg, _)) = current_arg.take() {
                parts.push(arg);
            }
            parts.push(piece);
            continue;
        }

        match current_arg.as_mut() {
            Some((arg, last_end)) if tok.start == *last_end => {
                arg.push_str(&piece);
                *last_end = tok.end;
            }
            Some(_) => {
                if let Some((arg, _)) = current_arg.take() {
                    parts.push(arg);
                }
                current_arg = Some((piece, tok.end));
            }
            None => {
                current_arg = Some((piece, tok.end));
            }
        }
    }

    if let Some((arg, _)) = current_arg {
        parts.push(arg);
    }

    if parts.is_empty() {
        parser.diagnostics.push(diagnostics::Diagnostic::new(
            diagnostics::DiagnosticKind::Parse,
            "Shell command requires at least one program token after '$'.",
            diagnostics::Span::new(dollar.start, dollar.end, dollar.line, dollar.col),
        ));
        return Stmt::Expression(Expr::Literal(ArtValue::none()));
    }

    let program = parts.remove(0);
    Stmt::ShellCommand {
        program,
        args: parts,
    }
}

pub fn let_declaration(parser: &mut Parser) -> Stmt {
    let pattern = parse_pattern(parser);

    let ty = if parser.match_token(TokenType::Colon) {
        Some(parser.parse_type())
    } else {
        None
    };

    parser.consume(TokenType::Equal, "Expect '=' after variable name or type.");
    let mut initializer = parser.expression();
    // Suporte a inicialização de struct dentro de expressão de let (ex: let p = Pessoa { campo: valor } )
    if let Expr::Variable {
        name: struct_name_tok,
    } = &initializer
        && parser.match_token(TokenType::LeftBrace)
    {
        let mut fields = Vec::new();
        while !parser.is_at_end() && !parser.check(&TokenType::RightBrace) {
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
                    diagnostics::Span::new(p.start, p.end, p.line, p.col),
                ));
                break;
            }
        }
        parser.consume(TokenType::RightBrace, "Expect '}' after struct fields.");
        initializer = Expr::StructInit {
            name: struct_name_tok.clone(),
            fields,
        };
    }
    parser.match_token(TokenType::Semicolon);

    // Fallback: If pattern is a simple variable, we use it directly as the name.
    if let MatchPattern::Variable(name) = pattern {
        Stmt::Let {
            pattern: MatchPattern::Variable(name),
            ty,
            initializer,
        }
    } else {
        // If it's a destructuring pattern (e.g. tuple), we need to update `Stmt::Let` to support `MatchPattern` later.
        // For now, if we reach here and Stmt::Let doesn't accept pattern natively, we cheat by using a dummy
        // name and handling it properly over the interpreter. Let's assume we update `Stmt::Let` next.
        // Wait, AST Stmt::Let currently takes `name: Token`. 
        // We will change `Stmt::Let` signature in AST to use `pattern: MatchPattern`.
        // Let's assume `Stmt::Let` actually uses `pattern` in the updated AST.
        Stmt::Let {
            pattern,
            ty,
            initializer,
        }
    }
}

pub fn if_statement(parser: &mut Parser) -> Stmt {
    parser.consume(TokenType::If, "Expect 'if'.");

    // Support for `if let Pattern = Expr { ... }`
    if parser.match_token(TokenType::Let) {
        let pattern = parse_pattern(parser);
        parser.consume(TokenType::Equal, "Expect '=' after pattern in 'if let'.");
        let value = parser.expression();
        let then_branch = Box::new(statement(parser));
        let else_branch = if parser.match_token(TokenType::Else) {
            Some(Box::new(statement(parser)))
        } else {
            None
        };
        return Stmt::IfLet {
            pattern,
            value,
            then_branch,
            else_branch,
        };
    }

    let condition = parser.expression();

    let then_branch = Box::new(statement(parser));
    let else_branch = if parser.match_token(TokenType::Else) {
        Some(Box::new(statement(parser)))
    } else {
        None
    };

    Stmt::If {
        condition,
        then_branch,
        else_branch,
    }
}

pub fn try_catch_statement(parser: &mut Parser) -> Stmt {
    parser.consume(TokenType::Try, "Expect 'try'.");
    let try_branch = Box::new(statement(parser));

    parser.consume(TokenType::Catch, "Expect 'catch' after try branch.");
    let catch_name = parser.consume(TokenType::Identifier, "Expect error binding name after 'catch'.");
    let catch_branch = Box::new(statement(parser));

    Stmt::TryCatch {
        try_branch,
        catch_name,
        catch_branch,
    }
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
        // Guard opcional: 'if' expressão
        let guard = if parser.match_token(TokenType::If) {
            Some(parser.expression())
        } else {
            None
        };
        parser.consume(TokenType::Colon, "Expect ':' after case pattern / guard.");
        let stmt = statement(parser);
        cases.push((pattern, guard, stmt));
    }
    parser.consume(TokenType::RightBrace, "Expect '}' after match cases.");
    Stmt::Match { expr, cases }
}

pub fn parse_pattern(parser: &mut Parser) -> MatchPattern {
    if parser.match_token(TokenType::Dot) {
        // Padrão shorthand: .variant
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
        MatchPattern::EnumVariant {
            enum_name: None,
            variant,
            params,
        }
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
            }
            TokenType::String(s) => {
                MatchPattern::Literal(ArtValue::String(core::intern_arc(&s)))
            }
            TokenType::True => MatchPattern::Literal(ArtValue::Bool(true)),
            TokenType::False => MatchPattern::Literal(ArtValue::Bool(false)),
            TokenType::None => MatchPattern::Literal(ArtValue::none()),
            _ => {
                let p = parser.peek();
                parser.diagnostics.push(diagnostics::Diagnostic::new(
                    diagnostics::DiagnosticKind::Parse,
                    "Unexpected token in pattern".to_string(),
                    diagnostics::Span::new(p.start, p.end, p.line, p.col),
                ));
                MatchPattern::Wildcard
            }
        }
    } else if parser.check(&TokenType::Identifier) {
        let name = parser.consume(TokenType::Identifier, "Expect pattern.");
        // Verificar se é um nome qualificado (Enum.Variant)
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
            MatchPattern::EnumVariant {
                enum_name: Some(name),
                variant,
                params,
            }
        } else if parser.match_token(TokenType::LeftParen) {
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
            MatchPattern::EnumVariant {
                enum_name: None,
                variant: name,
                params: Some(param_list),
            }
        } else {
            MatchPattern::Variable(name)
        }
    } else if parser.match_token(TokenType::LeftParen) {
        let mut items = Vec::new();
        if !parser.check(&TokenType::RightParen) {
            loop {
                items.push(parse_pattern(parser));
                if !parser.match_token(TokenType::Comma) {
                    break;
                }
            }
        }
        parser.consume(TokenType::RightParen, "Expect ')' after tuple pattern.");
        MatchPattern::Tuple(items)
    } else {
        let p = parser.peek();
        parser.diagnostics.push(diagnostics::Diagnostic::new(
            diagnostics::DiagnosticKind::Parse,
            "Expected pattern after 'case'".to_string(),
            diagnostics::Span::new(p.start, p.end, p.line, p.col),
        ));
        MatchPattern::Wildcard
    }
}

fn is_literal_token(token_type: &TokenType) -> bool {
    matches!(
        token_type,
        TokenType::Number(_)
            | TokenType::String(_)
            | TokenType::True
            | TokenType::False
            | TokenType::None
    )
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

pub fn while_statement(parser: &mut Parser) -> Stmt {
    parser.consume(TokenType::While, "Expect 'while'.");
    let condition = parser.expression();
    let body = Box::new(statement(parser));

    Stmt::While { condition, body }
}

pub fn for_statement(parser: &mut Parser) -> Stmt {
    parser.consume(TokenType::For, "Expect 'for'.");
    let element = parser.consume(TokenType::Identifier, "Expect element name after 'for'.");

    parser.consume(TokenType::In, "Expect 'in' after for loop element.");
    let iterator = parser.expression();

    let body = Box::new(statement(parser));

    Stmt::For {
        element,
        iterator,
        body,
    }
}
