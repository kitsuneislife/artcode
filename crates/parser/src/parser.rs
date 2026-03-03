impl Parser {
    pub fn parse_interpolated_string(&mut self, raw: String) -> Expr {
        use core::ast::{Expr, InterpolatedPart};
        use lexer::Lexer; // usar lexer para sub-expressões

        let mut parts = Vec::new();
        let chars: Vec<char> = raw.chars().collect();
        let mut i = 0usize;
        let mut literal_buf = String::new();

        while i < chars.len() {
            let c = chars[i];
            if c == '{' {
                // Escape '{{' -> '{'
                if i + 1 < chars.len() && chars[i + 1] == '{' {
                    literal_buf.push('{');
                    i += 2;
                    continue;
                }
                // flush literal
                if !literal_buf.is_empty() {
                    parts.push(InterpolatedPart::Literal(literal_buf.clone()));
                    literal_buf.clear();
                }
                i += 1; // consume '{'
                let expr_start = i;
                let mut depth = 1; // allow nested braces
                while i < chars.len() && depth > 0 {
                    match chars[i] {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                if depth != 0 {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Parse,
                        "Unterminated interpolation expression in f-string",
                        Span::new(
                            self.tokens[self.current.min(self.tokens.len() - 1)].start,
                            self.tokens[self.current.min(self.tokens.len() - 1)].end,
                            self.tokens[self.current.min(self.tokens.len() - 1)].line,
                            self.tokens[self.current.min(self.tokens.len() - 1)].col,
                        ),
                    ));
                    break;
                }
                // Content between braces (may contain :fmt at top level)
                let inner: String = chars[expr_start..i].iter().collect();
                // advance past closing '}'
                i += 1;
                // Split on first ':' not inside nested braces (already removed)
                let mut expr_src = inner.as_str();
                let mut fmt_opt: Option<String> = None;
                if let Some(colon_pos) = inner.find(':') {
                    // ensure no other ':' before formats? accept first
                    expr_src = &inner[..colon_pos];
                    let fmt_part = &inner[colon_pos + 1..];
                    if !fmt_part.is_empty() {
                        fmt_opt = Some(fmt_part.to_string());
                    }
                }
                // parse expression source
                let mut sub_lexer = Lexer::new(expr_src.to_string());
                let tokens = match sub_lexer.scan_tokens() {
                    Ok(t) => t,
                    Err(diag) => {
                        // Propagar diagnóstico de lexing da sub-expressão
                        self.diagnostics.push(diag);
                        continue; // pular esta interpolação
                    }
                };
                let mut sub_parser = Parser::new(tokens);
                let expr = sub_parser.expression();
                parts.push(InterpolatedPart::Expr {
                    expr: Box::new(expr),
                    format: fmt_opt,
                });
            } else if c == '}' {
                // stray or escaped '}}'
                if i + 1 < chars.len() && chars[i + 1] == '}' {
                    // escape sequence
                    literal_buf.push('}');
                    i += 2;
                    continue;
                } else {
                    self.diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Parse,
                        "Unmatched '}' in interpolated string",
                        Span::new(
                            self.tokens[self.current.min(self.tokens.len() - 1)].start,
                            self.tokens[self.current.min(self.tokens.len() - 1)].end,
                            self.tokens[self.current.min(self.tokens.len() - 1)].line,
                            self.tokens[self.current.min(self.tokens.len() - 1)].col,
                        ),
                    ));
                    i += 1;
                    continue;
                }
            } else {
                literal_buf.push(c);
                i += 1;
            }
        }
        if !literal_buf.is_empty() {
            parts.push(InterpolatedPart::Literal(literal_buf));
        }
        Expr::InterpolatedString(parts)
    }
}
use crate::expressions;
use crate::precedence::Precedence;
use crate::statements;
use core::ast::{Expr, Program, Stmt};
use core::{Token, TokenType};
use diagnostics::{Diagnostic, DiagnosticKind, Span};
use std::rc::Rc;

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
    pub diagnostics: Vec<Diagnostic>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser {
            tokens,
            current: 0,
            diagnostics: Vec::new(),
        }
    }

    pub fn parse(&mut self) -> (Program, Vec<Diagnostic>) {
        let mut statements = Vec::new();
        while !self.is_at_end() {
            statements.push(self.declaration());
        }
        (statements, std::mem::take(&mut self.diagnostics))
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
        } else if self.match_token(TokenType::Import) {
            // parse dotted path: identifier ( '.' identifier )* ';'
            let mut path = Vec::new();
            // Expect at least one identifier
            let first = self.consume(TokenType::Identifier, "Expect module name after 'import'.");
            path.push(first);
            while self.match_token(TokenType::Dot) {
                let part = self.consume(
                    TokenType::Identifier,
                    "Expect identifier after '.' in import path.",
                );
                path.push(part);
            }
            self.consume(TokenType::Semicolon, "Expect ';' after import path.");
            Stmt::Import { path }
        } else if self.match_token(TokenType::Performant) {
            // performant { ... }
            self.consume(TokenType::LeftBrace, "Expect '{' after performant.");
            let statements = self.block();
            Stmt::Performant { statements }
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
            TokenType::Greater
            | TokenType::GreaterEqual
            | TokenType::Less
            | TokenType::LessEqual => Precedence::Comparison as u8,
            TokenType::Plus | TokenType::Minus => Precedence::Term as u8,
            TokenType::Star | TokenType::Slash => Precedence::Factor as u8,
            TokenType::LeftParen | TokenType::Dot => Precedence::Call as u8,
            TokenType::As => Precedence::Call as u8,
            TokenType::Question => Precedence::Try as u8,
            TokenType::Bang => Precedence::Call as u8, // tratar 'expr!' como postfix acesso unowned
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
        let peek = self.peek();
        self.report(
            peek.start,
            peek.end,
            peek.line,
            peek.col,
            DiagnosticKind::Parse,
            format!("{}: expected {:?}, got {:?}", message, tt, peek.token_type),
        );
        // Recover: return dummy token of expected type
        Token::new(tt, String::new(), peek.line, peek.col, peek.start, peek.end)
    }

    fn report(
        &mut self,
        start: usize,
        end: usize,
        line: usize,
        col: usize,
        kind: DiagnosticKind,
        msg: String,
    ) {
        self.diagnostics
            .push(Diagnostic::new(kind, msg, Span::new(start, end, line, col)));
    }

    pub fn check(&self, tt: &TokenType) -> bool {
        if self.is_at_end() {
            return false;
        }
        std::mem::discriminant(&self.peek().token_type) == std::mem::discriminant(tt)
    }

    pub fn advance(&mut self) -> Token {
        if !self.is_at_end() {
            self.current += 1;
        }
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
            self.consume(
                TokenType::RightBracket,
                "Expect ']' after array element type.",
            );
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
                        let t = self.peek();
                        self.report(
                            t.start,
                            t.end,
                            t.line,
                            t.col,
                            DiagnosticKind::Parse,
                            ", or > expected in generic type parameters".to_string(),
                        );
                        break;
                    }
                }
            }
        }
        type_str
    }

    pub fn function_declaration(&mut self) -> Stmt {
        let first_ident = self.consume(TokenType::Identifier, "Expect function name.");
        let (name, method_owner) = if self.match_token(TokenType::Dot) {
            if self.check(&TokenType::Identifier) {
                let method_ident = self.advance();
                (method_ident, Some(first_ident.lexeme.clone()))
            } else {
                (first_ident, None)
            }
        } else {
            (first_ident, None)
        };
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
        let body = Rc::new(Stmt::Block {
            statements: self.block(),
        });
        Stmt::Function {
            name,
            params,
            return_type,
            body,
            method_owner,
        }
    }
}
