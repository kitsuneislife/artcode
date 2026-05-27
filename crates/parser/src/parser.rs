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
use core::ast::{Expr, Program, Stmt, TemplateNode};
use core::{Token, TokenType};
use diagnostics::{Diagnostic, DiagnosticKind, Span};
use std::collections::HashSet;
use std::rc::Rc;

const MAX_PARSE_DEPTH: usize = 200;

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
    pub diagnostics: Vec<Diagnostic>,
    depth: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser {
            tokens,
            current: 0,
            diagnostics: Vec::new(),
            depth: 0,
        }
    }

    /// Enter one level of recursive descent. Returns `false` when depth limit is exceeded.
    pub fn push_depth(&mut self, span_token: Option<&Token>) -> bool {
        self.depth += 1;
        if self.depth > MAX_PARSE_DEPTH {
            let (start, end, line, col) = span_token
                .map(|t| (t.start, t.end, t.line, t.col))
                .unwrap_or((0, 0, 0, 0));
            self.diagnostics.push(Diagnostic::new(
                DiagnosticKind::Parse,
                format!("Expression nested too deeply (limit {MAX_PARSE_DEPTH}). Possible unclosed delimiter."),
                Span::new(start, end, line, col),
            ));
            self.depth -= 1;
            return false;
        }
        true
    }

    pub fn pop_depth(&mut self) {
        if self.depth > 0 {
            self.depth -= 1;
        }
    }

    pub fn parse(&mut self) -> (Program, Vec<Diagnostic>) {
        let mut statements = Vec::new();
        while !self.is_at_end() {
            statements.push(self.declaration());
        }
        check_component_imports(&statements, &mut self.diagnostics);
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
        } else if self.match_token(TokenType::Impl) {
            self.impl_block()
        } else if self.match_token(TokenType::Component) {
            self.component_block()
        } else {
            self.statement()
        }
    }

    fn impl_block(&mut self) -> Stmt {
        let name_token = self.consume(TokenType::Identifier, "Expect type name after 'impl'.");
        let type_name = name_token.lexeme.clone();
        self.consume(TokenType::LeftBrace, "Expect '{' after impl type name.");
        let mut methods = Vec::new();
        while !self.is_at_end() && !self.check(&TokenType::RightBrace) {
            if self.match_token(TokenType::Func) {
                let mut method = self.function_declaration();
                // Inject type_name as method_owner if not already set
                if let Stmt::Function { ref mut method_owner, .. } = method {
                    method_owner.get_or_insert_with(|| type_name.clone());
                }
                methods.push(method);
            } else {
                // Skip unknown tokens inside impl to avoid hard parse failure
                self.advance();
            }
        }
        self.consume(TokenType::RightBrace, "Expect '}' after impl block.");
        Stmt::ImplBlock { type_name, methods }
    }

    fn component_block(&mut self) -> Stmt {
        use core::ast::{BindingQualifier, TemplateNode};
        let name_token = self.consume(TokenType::Identifier, "Expect component name after 'component'.");
        let name = name_token.lexeme.clone();
        self.consume(TokenType::LeftBrace, "Expect '{' after component name.");
        let mut bindings: Vec<Stmt> = Vec::new();
        let mut view: Vec<TemplateNode> = Vec::new();
        while !self.is_at_end() && !self.check(&TokenType::RightBrace) {
            let qualifier = if self.match_token(TokenType::State) {
                Some(BindingQualifier::State)
            } else if self.match_token(TokenType::Prop) {
                Some(BindingQualifier::Prop)
            } else if self.match_token(TokenType::Memo) {
                Some(BindingQualifier::Memo)
            } else if self.match_token(TokenType::Ref) {
                Some(BindingQualifier::Ref)
            } else {
                None
            };
            if let Some(qualifier) = qualifier {
                let name_tok = self.consume(TokenType::Identifier, "Expect binding name.");
                let type_ann = if self.match_token(TokenType::Colon) {
                    Some(self.parse_type())
                } else {
                    None
                };
                let value = if self.match_token(TokenType::Equal) {
                    Some(Box::new(self.expression()))
                } else {
                    None
                };
                self.match_token(TokenType::Semicolon);
                bindings.push(Stmt::QualifiedBinding { qualifier, name: name_tok, type_ann, value });
            } else if self.match_token(TokenType::View) {
                self.consume(TokenType::LeftBrace, "Expect '{' after 'view'.");
                let nodes = expressions::parse_template_nodes(self);
                self.consume(TokenType::RightBrace, "Expect '}' after view body.");
                view = nodes;
            } else {
                self.advance();
            }
        }
        self.consume(TokenType::RightBrace, "Expect '}' after component block.");
        Stmt::ComponentBlock { name, bindings, view }
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
            TokenType::PipeGreater => Precedence::Pipeline as u8,
            TokenType::And => Precedence::And as u8,
            TokenType::Or => Precedence::Or as u8,
            TokenType::EqualEqual | TokenType::BangEqual => Precedence::Equality as u8,
            TokenType::Greater
            | TokenType::GreaterEqual
            | TokenType::Less
            | TokenType::LessEqual => Precedence::Comparison as u8,
            TokenType::Plus | TokenType::Minus => Precedence::Term as u8,
            TokenType::Star | TokenType::Slash => Precedence::Factor as u8,
            TokenType::LeftParen | TokenType::Dot | TokenType::ColonColon => Precedence::Call as u8,
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

    pub fn current_pos(&self) -> usize {
        self.current
    }

    pub fn set_current_pos(&mut self, pos: usize) {
        self.current = pos;
    }

    pub fn tokens_ref(&self) -> &[Token] {
        &self.tokens
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
        } else if self.match_token(TokenType::LeftParen) {
            let mut types = Vec::new();
            if !self.check(&TokenType::RightParen) {
                loop {
                    types.push(self.parse_type());
                    if !self.match_token(TokenType::Comma) {
                        break;
                    }
                }
            }
            self.consume(
                TokenType::RightParen,
                "Expect ')' after tuple type elements.",
            );
            type_str.push('(');
            type_str.push_str(&types.join(", "));
            type_str.push(')');
        } else {
            let type_name = self.consume(TokenType::Identifier, "Expect type name.");
            type_str.push_str(&type_name.lexeme);
            if self.match_token(TokenType::LeftBracket) {
                type_str.push('[');
                type_str.push_str(&self.parse_type());
                self.consume(
                    TokenType::RightBracket,
                    "Expect ']' after bracketed type parameter.",
                );
                type_str.push(']');
            }
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

        let mut type_params = None;
        if self.match_token(TokenType::Less) {
            let mut tps = Vec::new();
            if !self.check(&TokenType::Greater) {
                loop {
                    let tp = self.consume(TokenType::Identifier, "Expect type parameter name.");
                    let bound = if self.match_token(TokenType::Colon) {
                        Some(
                            self.consume(TokenType::Identifier, "Expect trait bound name.")
                                .lexeme,
                        )
                    } else {
                        None
                    };
                    tps.push((tp.lexeme, bound));
                    if !self.match_token(TokenType::Comma) {
                        break;
                    }
                }
            }
            self.consume(TokenType::Greater, "Expect '>' after type parameters.");
            type_params = Some(tps);
        }

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
            type_params,
            params,
            return_type,
            body,
            method_owner,
            is_async: false,
        }
    }
}

// ── Post-parse: component import validation ──────────────────────────────────

/// Collect names known at file scope: imported module names, locally defined
/// struct names, and function names that start with an uppercase letter.
fn collect_known_names(stmts: &[Stmt]) -> HashSet<String> {
    let mut known = HashSet::new();
    for stmt in stmts {
        match stmt {
            Stmt::Import { path } => {
                if let Some(last) = path.last() {
                    known.insert(last.lexeme.clone());
                }
            }
            Stmt::StructDecl { name, .. } => {
                known.insert(name.lexeme.clone());
            }
            Stmt::Function { name, .. }
                if name.lexeme.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) =>
            {
                known.insert(name.lexeme.clone());
            }
            Stmt::Function { .. } => {}
            Stmt::Let { pattern: core::ast::MatchPattern::Variable(tok), .. }
                if tok.lexeme.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) =>
            {
                known.insert(tok.lexeme.clone());
            }
            Stmt::Let { .. } => {}
            _ => {}
        }
    }
    known
}

/// Walk a slice of TemplateNodes and collect (component_name, span) for all
/// PascalCase component references.
fn collect_template_components(nodes: &[TemplateNode]) -> Vec<(String, Span)> {
    let mut out = Vec::new();
    for node in nodes {
        match node {
            TemplateNode::Component { name, attrs: _, children } => {
                out.push((name.clone(), Span::new(0, 0, 0, 0)));
                out.extend(collect_template_components(children));
            }
            TemplateNode::Element { children, .. } => {
                out.extend(collect_template_components(children));
            }
            TemplateNode::If { then_children, else_children, .. } => {
                out.extend(collect_template_components(then_children));
                out.extend(collect_template_components(else_children));
            }
            TemplateNode::For { children, .. } => {
                out.extend(collect_template_components(children));
            }
            TemplateNode::Slot { children, .. } => {
                out.extend(collect_template_components(children));
            }
            TemplateNode::Text(_) | TemplateNode::Expr(_) => {}
        }
    }
    out
}

/// Walk all expressions in the program and find template component references.
fn find_template_components_in_stmts(stmts: &[Stmt]) -> Vec<(String, Span)> {
    let mut out = Vec::new();
    for stmt in stmts {
        find_template_components_in_stmt(stmt, &mut out);
    }
    out
}

fn find_template_components_in_stmt(stmt: &Stmt, out: &mut Vec<(String, Span)>) {
    match stmt {
        Stmt::Expression(e) | Stmt::Return { value: Some(e) } => {
            find_template_components_in_expr(e, out);
        }
        Stmt::Let { initializer, .. } => find_template_components_in_expr(initializer, out),
        Stmt::Block { statements } => {
            for s in statements { find_template_components_in_stmt(s, out); }
        }
        Stmt::Function { body, .. } => {
            find_template_components_in_stmt(body, out);
        }
        Stmt::SpawnActor { body } => {
            for s in body { find_template_components_in_stmt(s, out); }
        }
        Stmt::If { condition, then_branch, else_branch } => {
            find_template_components_in_expr(condition, out);
            find_template_components_in_stmt(then_branch, out);
            if let Some(eb) = else_branch { find_template_components_in_stmt(eb, out); }
        }
        Stmt::While { condition, body } => {
            find_template_components_in_expr(condition, out);
            find_template_components_in_stmt(body, out);
        }
        Stmt::For { iterator, body, .. } => {
            find_template_components_in_expr(iterator, out);
            find_template_components_in_stmt(body, out);
        }
        Stmt::Performant { statements } | Stmt::ImplBlock { methods: statements, .. } => {
            for s in statements { find_template_components_in_stmt(s, out); }
        }
        _ => {}
    }
}

fn find_template_components_in_expr(expr: &Expr, out: &mut Vec<(String, Span)>) {
    if let Expr::Template(nodes) = expr {
        out.extend(collect_template_components(nodes));
    }
}

fn check_component_imports(stmts: &[Stmt], diags: &mut Vec<Diagnostic>) {
    let known = collect_known_names(stmts);
    let used_components = find_template_components_in_stmts(stmts);
    let mut reported: HashSet<String> = HashSet::new();
    for (name, span) in used_components {
        if !known.contains(&name) && !reported.contains(&name) {
            reported.insert(name.clone());
            diags.push(Diagnostic::new(
                DiagnosticKind::Parse,
                format!(
                    "Component '<{name}>' is used but not imported or defined — add `import {name};` or define a struct '{name}'"
                ),
                span,
            ));
        }
    }
}
