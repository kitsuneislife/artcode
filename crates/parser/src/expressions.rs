use crate::parser::Parser;
use crate::precedence::Precedence;
use core::ast::{Expr, TemplateAttr, TemplateAttrValue, TemplateNode};
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
        TokenType::Less => parse_template_expr(parser, token),
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

// ── ArtML template parsing ────────────────────────────────────────────────────

fn parse_template_expr(parser: &mut Parser, lt_token: Token) -> Expr {
    // We already consumed `<`. Parse an opening tag name.
    let tag_name = parse_template_tag_name(parser);
    if tag_name.is_empty() {
        parser.diagnostics.push(diagnostics::Diagnostic::new(
            diagnostics::DiagnosticKind::Parse,
            "Expected tag name after '<'".to_string(),
            diagnostics::Span::new(lt_token.start, lt_token.end, lt_token.line, lt_token.col),
        ));
        return Expr::Literal(core::ast::ArtValue::none());
    }
    let node = parse_element(parser, tag_name, lt_token);
    Expr::Template(vec![node])
}

fn parse_template_tag_name(parser: &mut Parser) -> String {
    match &parser.peek().token_type {
        TokenType::Identifier => {
            let tok = parser.advance();
            tok.lexeme.clone()
        }
        TokenType::If => { parser.advance(); "if".to_string() }
        TokenType::For => { parser.advance(); "for".to_string() }
        TokenType::Else => { parser.advance(); "else".to_string() }
        _ => String::new(),
    }
}

fn parse_element(parser: &mut Parser, tag: String, open_lt: Token) -> TemplateNode {
    // Handle control tags
    if tag == "if" {
        return parse_if_node(parser, open_lt);
    }
    if tag == "for" {
        return parse_for_node(parser, open_lt);
    }
    if tag == "slot" {
        return parse_slot_node(parser, open_lt);
    }

    let attrs = parse_attrs(parser);

    // Self-closing: />
    if parser.match_token(TokenType::Slash) {
        parser.consume(TokenType::Greater, "Expect '>' after '/>' in self-closing tag.");
        return make_element_or_component(tag, attrs, Vec::new());
    }

    parser.consume(TokenType::Greater, "Expect '>' after tag attributes.");
    let children = parse_children(parser);
    consume_closing_tag(parser, &tag, &open_lt);

    make_element_or_component(tag, attrs, children)
}

fn make_element_or_component(tag: String, attrs: Vec<TemplateAttr>, children: Vec<TemplateNode>) -> TemplateNode {
    let is_component = tag.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
    if is_component {
        TemplateNode::Component { name: tag, attrs, children }
    } else {
        TemplateNode::Element { tag, attrs, children }
    }
}

fn parse_attrs(parser: &mut Parser) -> Vec<TemplateAttr> {
    let mut attrs = Vec::new();
    // Attributes end at `>`, `/>`, or EOF
    while !parser.is_at_end()
        && !parser.check(&TokenType::Greater)
        && !parser.check(&TokenType::Slash)
    {
        if !parser.check(&TokenType::Identifier) {
            break;
        }
        let name_tok = parser.advance();
        let attr_name = name_tok.lexeme.clone();

        // on:event handler syntax
        if attr_name == "on" && parser.match_token(TokenType::Colon) {
            let event_tok = parser.consume(TokenType::Identifier, "Expect event name after 'on:'");
            let name = format!("on:{}", event_tok.lexeme);
            let val = if parser.match_token(TokenType::Equal) {
                parser.consume(TokenType::LeftBrace, "Expect '{' after event handler '='");
                let expr = expression(parser);
                parser.consume(TokenType::RightBrace, "Expect '}' after event handler expression");
                TemplateAttrValue::EventHandler(Box::new(expr))
            } else {
                TemplateAttrValue::Flag
            };
            attrs.push(TemplateAttr { name, value: val });
            continue;
        }

        if parser.match_token(TokenType::Equal) {
            if parser.check(&TokenType::LeftBrace) {
                parser.advance(); // consume '{'
                let expr = expression(parser);
                parser.consume(TokenType::RightBrace, "Expect '}' after attribute expression");
                attrs.push(TemplateAttr {
                    name: attr_name,
                    value: TemplateAttrValue::Dynamic(Box::new(expr)),
                });
            } else if let TokenType::String(s) = parser.peek().token_type.clone() {
                parser.advance();
                attrs.push(TemplateAttr {
                    name: attr_name,
                    value: TemplateAttrValue::Static(s),
                });
            } else {
                parser.diagnostics.push(diagnostics::Diagnostic::new(
                    diagnostics::DiagnosticKind::Parse,
                    format!("Expected string or '{{' after '=' in attribute '{}'", attr_name),
                    diagnostics::Span::new(name_tok.start, name_tok.end, name_tok.line, name_tok.col),
                ));
            }
        } else {
            // Boolean flag attribute
            attrs.push(TemplateAttr { name: attr_name, value: TemplateAttrValue::Flag });
        }
    }
    attrs
}

fn parse_children(parser: &mut Parser) -> Vec<TemplateNode> {
    let mut children = Vec::new();
    while !parser.is_at_end() {
        // Check for closing tag: </
        if parser.check(&TokenType::Less) {
            // Peek ahead: if next-next is Slash, it's a closing tag
            let saved = parser.current_pos();
            parser.advance(); // consume '<'
            if parser.check(&TokenType::Slash) {
                // closing tag — restore and return
                parser.set_current_pos(saved);
                break;
            }
            // It's a child element: parse it
            let child_lt = parser.tokens_ref()[saved].clone();
            let child_tag = parse_template_tag_name(parser);
            if child_tag.is_empty() {
                // not a valid element, emit error and stop
                parser.diagnostics.push(diagnostics::Diagnostic::new(
                    diagnostics::DiagnosticKind::Parse,
                    "Expected tag name in child element".to_string(),
                    diagnostics::Span::new(child_lt.start, child_lt.end, child_lt.line, child_lt.col),
                ));
                break;
            }
            children.push(parse_element(parser, child_tag, child_lt));
        } else if parser.check(&TokenType::LeftBrace) {
            // Inline expression: {expr}
            parser.advance(); // consume '{'
            let expr = expression(parser);
            parser.consume(TokenType::RightBrace, "Expect '}' after template expression");
            children.push(TemplateNode::Expr(Box::new(expr)));
        } else {
            // Text content: consume tokens as text until '<' or '{'
            let tok = parser.advance();
            let text = tok.lexeme.clone();
            if !text.trim().is_empty() {
                children.push(TemplateNode::Text(text));
            }
        }
    }
    children
}

fn consume_closing_tag(parser: &mut Parser, tag: &str, open_lt: &Token) {
    // Expect </ tag >
    if !parser.match_token(TokenType::Less) {
        parser.diagnostics.push(diagnostics::Diagnostic::new(
            diagnostics::DiagnosticKind::Parse,
            format!("Unclosed tag '<{tag}>' — expected '</{tag}>'"),
            diagnostics::Span::new(open_lt.start, open_lt.end, open_lt.line, open_lt.col),
        ));
        return;
    }
    if !parser.match_token(TokenType::Slash) {
        parser.diagnostics.push(diagnostics::Diagnostic::new(
            diagnostics::DiagnosticKind::Parse,
            format!("Expected '</' to close tag '<{}>'", tag),
            diagnostics::Span::new(open_lt.start, open_lt.end, open_lt.line, open_lt.col),
        ));
        return;
    }
    let close_tag = parse_template_tag_name(parser);
    if close_tag != tag {
        parser.diagnostics.push(diagnostics::Diagnostic::new(
            diagnostics::DiagnosticKind::Parse,
            format!("Mismatched tags: opened '<{}>' but closed '</{}>'\n  hint: check for missing closing tag", tag, close_tag),
            diagnostics::Span::new(open_lt.start, open_lt.end, open_lt.line, open_lt.col),
        ));
    }
    parser.consume(TokenType::Greater, "Expect '>' after closing tag name.");
}

fn parse_if_node(parser: &mut Parser, open_lt: Token) -> TemplateNode {
    // <if cond={expr}>...</if>
    // consume cond attr
    let mut cond_expr: Option<Expr> = None;
    while !parser.is_at_end() && !parser.check(&TokenType::Greater) && !parser.check(&TokenType::Slash) {
        if let TokenType::Identifier = parser.peek().token_type {
            let name_tok = parser.advance();
            if name_tok.lexeme == "cond" && parser.match_token(TokenType::Equal) {
                parser.consume(TokenType::LeftBrace, "Expect '{' after cond=");
                cond_expr = Some(expression(parser));
                parser.consume(TokenType::RightBrace, "Expect '}' after cond expression");
            }
        } else {
            parser.advance(); // skip unexpected token
        }
    }
    parser.consume(TokenType::Greater, "Expect '>' after <if cond={...}>");
    let then_children = parse_children(parser);
    consume_closing_tag(parser, "if", &open_lt);

    // Optional <else>...</else>
    let mut else_children = Vec::new();
    if parser.check(&TokenType::Less) {
        let saved = parser.current_pos();
        parser.advance(); // consume '<'
        let maybe_else = parse_template_tag_name(parser);
        if maybe_else == "else" {
            parser.consume(TokenType::Greater, "Expect '>' after <else>");
            else_children = parse_children(parser);
            consume_closing_tag(parser, "else", &open_lt);
        } else {
            parser.set_current_pos(saved);
        }
    }

    let cond = cond_expr.unwrap_or(Expr::Literal(core::ast::ArtValue::Bool(true)));
    TemplateNode::If {
        cond: Box::new(cond),
        then_children,
        else_children,
    }
}

fn parse_for_node(parser: &mut Parser, open_lt: Token) -> TemplateNode {
    // <for item in {items} key={item.id}>...</for>
    let mut var = String::new();
    let mut items_expr: Option<Expr> = None;
    let mut key_expr: Option<Box<Expr>> = None;

    while !parser.is_at_end() && !parser.check(&TokenType::Greater) && !parser.check(&TokenType::Slash) {
        match parser.peek().token_type.clone() {
            TokenType::Identifier => {
                let name_tok = parser.advance();
                let attr_name = name_tok.lexeme.clone();
                if attr_name == "key" && parser.match_token(TokenType::Equal) {
                    parser.consume(TokenType::LeftBrace, "Expect '{' after key=");
                    key_expr = Some(Box::new(expression(parser)));
                    parser.consume(TokenType::RightBrace, "Expect '}' after key expression");
                } else if parser.match_token(TokenType::Equal) {
                    // generic attr, skip
                    parser.advance();
                } else if parser.check(&TokenType::In) {
                    // `item in {items}` pattern
                    var = attr_name;
                    parser.advance(); // consume 'in'
                    parser.consume(TokenType::LeftBrace, "Expect '{' after 'in' in <for>");
                    items_expr = Some(expression(parser));
                    parser.consume(TokenType::RightBrace, "Expect '}' after items expression in <for>");
                } else {
                    var = attr_name; // bare identifier = loop var
                }
            }
            TokenType::In => {
                parser.advance(); // skip stray 'in'
            }
            _ => {
                parser.advance(); // skip unknown tokens
            }
        }
    }

    if !parser.check(&TokenType::Slash) {
        parser.consume(TokenType::Greater, "Expect '>' after <for ...>");
    } else {
        parser.advance(); // consume '/'
        parser.consume(TokenType::Greater, "Expect '>' after '/>' in self-closing <for>");
        return TemplateNode::For {
            var,
            items: Box::new(items_expr.unwrap_or(Expr::Literal(core::ast::ArtValue::none()))),
            key: key_expr,
            children: Vec::new(),
        };
    }

    let children = parse_children(parser);
    consume_closing_tag(parser, "for", &open_lt);

    if key_expr.is_none() {
        parser.diagnostics.push(diagnostics::Diagnostic::new(
            diagnostics::DiagnosticKind::Parse,
            "warning: <for> without 'key' may cause incorrect re-renders".to_string(),
            diagnostics::Span::new(open_lt.start, open_lt.end, open_lt.line, open_lt.col),
        ));
    }

    TemplateNode::For {
        var,
        items: Box::new(items_expr.unwrap_or(Expr::Literal(core::ast::ArtValue::none()))),
        key: key_expr,
        children,
    }
}

fn parse_slot_node(parser: &mut Parser, open_lt: Token) -> TemplateNode {
    // <slot name='header'>...</slot>
    let mut slot_name: Option<String> = None;
    while !parser.is_at_end() && !parser.check(&TokenType::Greater) && !parser.check(&TokenType::Slash) {
        if let TokenType::Identifier = parser.peek().token_type {
            let attr_tok = parser.advance();
            if attr_tok.lexeme == "name" && parser.match_token(TokenType::Equal) {
                if let TokenType::String(s) = parser.peek().token_type.clone() {
                    parser.advance();
                    slot_name = Some(s);
                }
            }
        } else {
            parser.advance();
        }
    }
    if parser.match_token(TokenType::Slash) {
        parser.consume(TokenType::Greater, "Expect '>' after '/>' in self-closing <slot>");
        return TemplateNode::Slot { name: slot_name, children: Vec::new() };
    }
    parser.consume(TokenType::Greater, "Expect '>' after <slot ...>");
    let children = parse_children(parser);
    consume_closing_tag(parser, "slot", &open_lt);
    TemplateNode::Slot { name: slot_name, children }
}
