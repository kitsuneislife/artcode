use core::ast::{Expr, InterpolatedPart, MatchPattern, Stmt};
use core::Token;
use diagnostics::{Diagnostic, DiagnosticKind, Span};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RefKind {
    Weak,
    Unowned,
}

pub fn lint_ast(program: &[Stmt]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut scope_stack = ScopeStack::new();

    for stmt in program {
        lint_stmt(stmt, &mut scope_stack, &mut diagnostics, false);
    }

    diagnostics
}

struct ScopeStack {
    scopes: Vec<HashSet<String>>,
    ref_kinds: Vec<HashMap<String, RefKind>>,
}

impl ScopeStack {
    fn new() -> Self {
        Self {
            scopes: vec![HashSet::new()],
            ref_kinds: vec![HashMap::new()],
        }
    }

    fn push(&mut self) {
        self.scopes.push(HashSet::new());
        self.ref_kinds.push(HashMap::new());
    }

    fn pop(&mut self) {
        self.scopes.pop();
        self.ref_kinds.pop();
    }

    fn declare(&mut self, name: &str, token: &Token, diagnostics: &mut Vec<Diagnostic>) {
        // Shadowing check: Is it declared in a *parent* scope?
        if self.scopes.len() > 1 {
            for i in (0..self.scopes.len() - 1).rev() {
                if self.scopes[i].contains(name) {
                    let span = Span::new(token.start, token.end, token.line, token.col);
                    diagnostics.push(Diagnostic::new(
                        DiagnosticKind::Lint,
                        format!("Suspicious shadowing: variable '{}' is already declared in an outer scope.", name),
                        span,
                    ).note("Shadowing can lead to logic errors or unintended bugs. Consider renaming this variable."));
                    break;
                }
            }
        }

        // Add to current scope
        if let Some(current) = self.scopes.last_mut() {
            current.insert(name.to_string());
        }
    }

    fn set_ref_kind(&mut self, name: &str, kind: RefKind) {
        if let Some(current) = self.ref_kinds.last_mut() {
            current.insert(name.to_string(), kind);
        }
    }

    fn get_ref_kind(&self, name: &str) -> Option<RefKind> {
        for scope in self.ref_kinds.iter().rev() {
            if let Some(kind) = scope.get(name) {
                return Some(*kind);
            }
        }
        None
    }
}

fn infer_ref_kind(expr: &Expr, scopes: &ScopeStack) -> Option<RefKind> {
    match expr {
        Expr::Weak(_) => Some(RefKind::Weak),
        Expr::Unowned(_) => Some(RefKind::Unowned),
        Expr::Grouping { expression } => infer_ref_kind(expression, scopes),
        Expr::Variable { name } => scopes.get_ref_kind(&name.lexeme),
        Expr::Call { callee, .. } => {
            if let Expr::Variable { name } = &**callee {
                match name.lexeme.as_str() {
                    "weak" => Some(RefKind::Weak),
                    "unowned" => Some(RefKind::Unowned),
                    _ => None,
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

fn is_scalar_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Literal(v) => matches!(
            v,
            core::ast::ArtValue::Int(_)
                | core::ast::ArtValue::Float(_)
                | core::ast::ArtValue::Bool(_)
                | core::ast::ArtValue::Optional(_)
        ),
        Expr::Grouping { expression } => is_scalar_literal(expression),
        _ => false,
    }
}

fn bind_ref_kind_from_pattern(pattern: &MatchPattern, kind: Option<RefKind>, scopes: &mut ScopeStack) {
    if let Some(kind) = kind {
        match pattern {
            MatchPattern::Variable(tok) | MatchPattern::Binding(tok) => {
                scopes.set_ref_kind(&tok.lexeme, kind);
            }
            _ => {}
        }
    }
}

fn lint_stmt(
    stmt: &Stmt,
    scopes: &mut ScopeStack,
    diagnostics: &mut Vec<Diagnostic>,
    in_performant: bool,
) {
    match stmt {
        Stmt::Let {
            pattern,
            initializer,
            ..
        } => {
            lint_expr(initializer, scopes, diagnostics);
            let ref_kind = infer_ref_kind(initializer, scopes);
            declare_pattern_bindings(pattern, scopes, diagnostics);
            bind_ref_kind_from_pattern(pattern, ref_kind, scopes);
        }
        Stmt::Function {
            name, params, body, ..
        } => {
            scopes.declare(&name.lexeme, name, diagnostics);
            scopes.push();
            for param in params {
                scopes.declare(&param.name.lexeme, &param.name, diagnostics);
            }
            lint_stmt(body, scopes, diagnostics, in_performant);
            scopes.pop();
        }
        Stmt::Block { statements } | Stmt::SpawnActor { body: statements } => {
            scopes.push();
            for s in statements {
                lint_stmt(s, scopes, diagnostics, in_performant);
            }
            scopes.pop();
        }
        Stmt::Performant { statements } => {
            scopes.push();
            for s in statements {
                lint_stmt(s, scopes, diagnostics, true);
            }
            scopes.pop();
        }
        Stmt::If {
            condition,
            then_branch,
            else_branch,
        } => {
            lint_expr(condition, scopes, diagnostics);
            lint_stmt(then_branch, scopes, diagnostics, in_performant);
            if let Some(els) = else_branch {
                lint_stmt(els, scopes, diagnostics, in_performant);
            }
        }
        Stmt::IfLet {
            pattern,
            value,
            then_branch,
            else_branch,
        } => {
            lint_expr(value, scopes, diagnostics);
            scopes.push();
            declare_pattern_bindings(pattern, scopes, diagnostics);
            lint_stmt(then_branch, scopes, diagnostics, in_performant);
            scopes.pop();
            if let Some(els) = else_branch {
                lint_stmt(els, scopes, diagnostics, in_performant);
            }
        }
        Stmt::TryCatch {
            try_branch,
            catch_name,
            catch_branch,
        } => {
            lint_stmt(try_branch, scopes, diagnostics, in_performant);
            scopes.push();
            scopes.declare(&catch_name.lexeme, catch_name, diagnostics);
            lint_stmt(catch_branch, scopes, diagnostics, in_performant);
            scopes.pop();
        }
        Stmt::Expression(expr) | Stmt::Return { value: Some(expr) } => {
            lint_expr(expr, scopes, diagnostics);
        }
        Stmt::Return { value: None }
        | Stmt::Import { .. }
        | Stmt::ShellCommand { .. }
        | Stmt::StructDecl { .. }
        | Stmt::EnumDecl { .. } => {}
        Stmt::Match { expr, cases } => {
            lint_expr(expr, scopes, diagnostics);
            let mut irrefutable_found = false;

            for (pattern, guard, body) in cases {
                if irrefutable_found {
                    diagnostics.push(Diagnostic::new(
                          DiagnosticKind::Lint,
                          "Dead code: this match arm is unreachable because a previous arm catches all remaining cases.",
                          Span::dummy(),
                      ));
                }

                scopes.push();
                declare_pattern_bindings(pattern, scopes, diagnostics);

                match pattern {
                    MatchPattern::Wildcard
                    | MatchPattern::Binding(_)
                    | MatchPattern::Variable(_) => {
                        if guard.is_none() {
                            irrefutable_found = true;
                        }
                    }
                    _ => {}
                }

                if let Some(g) = guard {
                    lint_expr(g, scopes, diagnostics);
                }
                lint_stmt(body, scopes, diagnostics, in_performant);
                scopes.pop();
            }
        }
        Stmt::While { condition, body } => {
            lint_expr(condition, scopes, diagnostics);
            if !in_performant && stmt_contains_allocation(body) {
                diagnostics.push(
                    Diagnostic::new(
                        DiagnosticKind::Lint,
                        "Potential allocation hotspot in loop body; consider wrapping this block in `performant {}` or reducing heap graph retention with `weak`/`unowned` where safe.",
                        Span::dummy(),
                    )
                    .note("This is a heuristic hint focused on memory-sensitive loops."),
                );
            }
            lint_stmt(body, scopes, diagnostics, in_performant);
        }
        Stmt::For {
            element,
            iterator,
            body,
        } => {
            lint_expr(iterator, scopes, diagnostics);
            if !in_performant && stmt_contains_allocation(body) {
                diagnostics.push(
                    Diagnostic::new(
                        DiagnosticKind::Lint,
                        "Potential allocation hotspot in loop body; consider wrapping this block in `performant {}` or reducing heap graph retention with `weak`/`unowned` where safe.",
                        Span::dummy(),
                    )
                    .note("This is a heuristic hint focused on memory-sensitive loops."),
                );
            }
            scopes.push();
            scopes.declare(&element.lexeme, element, diagnostics);
            lint_stmt(body, scopes, diagnostics, in_performant);
            scopes.pop();
        }
    }
}

fn stmt_contains_allocation(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Expression(expr) => expr_contains_allocation(expr),
        Stmt::Let { initializer, .. } => expr_contains_allocation(initializer),
        Stmt::Block { statements }
        | Stmt::Performant { statements }
        | Stmt::SpawnActor { body: statements } => {
            statements.iter().any(stmt_contains_allocation)
        }
        Stmt::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expr_contains_allocation(condition)
                || stmt_contains_allocation(then_branch)
                || else_branch
                    .as_deref()
                    .map(stmt_contains_allocation)
                    .unwrap_or(false)
        }
        Stmt::IfLet {
            value,
            then_branch,
            else_branch,
            ..
        } => {
            expr_contains_allocation(value)
                || stmt_contains_allocation(then_branch)
                || else_branch
                    .as_deref()
                    .map(stmt_contains_allocation)
                    .unwrap_or(false)
        }
        Stmt::TryCatch {
            try_branch,
            catch_branch,
            ..
        } => stmt_contains_allocation(try_branch) || stmt_contains_allocation(catch_branch),
        Stmt::Match { expr, cases } => {
            expr_contains_allocation(expr)
                || cases
                    .iter()
                    .any(|(_, guard, body)| {
                        guard
                            .as_ref()
                            .map(expr_contains_allocation)
                            .unwrap_or(false)
                            || stmt_contains_allocation(body)
                    })
        }
        Stmt::While { condition, body } => {
            expr_contains_allocation(condition) || stmt_contains_allocation(body)
        }
        Stmt::For { iterator, body, .. } => {
            expr_contains_allocation(iterator) || stmt_contains_allocation(body)
        }
        Stmt::Return { value } => value.as_ref().map(expr_contains_allocation).unwrap_or(false),
        Stmt::StructDecl { .. }
        | Stmt::EnumDecl { .. }
        | Stmt::Function { .. }
        | Stmt::Import { .. }
        | Stmt::ShellCommand { .. } => false,
    }
}

fn expr_contains_allocation(expr: &Expr) -> bool {
    match expr {
        Expr::Array(_) | Expr::Tuple(_) | Expr::StructInit { .. } | Expr::EnumInit { .. } => true,
        Expr::Call {
            callee, arguments, ..
        } => {
            let call_alloc = matches!(
                &**callee,
                Expr::Variable { name }
                    if matches!(name.lexeme.as_str(), "map_new" | "set_new" | "weak" | "unowned")
            );
            call_alloc
                || expr_contains_allocation(callee)
                || arguments.iter().any(expr_contains_allocation)
        }
        Expr::Binary { left, right, .. } | Expr::Logical { left, right, .. } => {
            expr_contains_allocation(left) || expr_contains_allocation(right)
        }
        Expr::Unary { right, .. }
        | Expr::Grouping { expression: right }
        | Expr::Try(right)
        | Expr::Weak(right)
        | Expr::Unowned(right)
        | Expr::WeakUpgrade(right)
        | Expr::UnownedAccess(right) => expr_contains_allocation(right),
        Expr::FieldAccess { object, .. } | Expr::Cast { object, .. } => expr_contains_allocation(object),
        Expr::InterpolatedString(parts) => parts.iter().any(|p| match p {
            InterpolatedPart::Literal(_) => false,
            InterpolatedPart::Expr { expr, .. } => expr_contains_allocation(expr),
        }),
        Expr::SpawnActor { body } => body.iter().any(stmt_contains_allocation),
        Expr::Literal(_) | Expr::Variable { .. } => false,
    }
}

fn declare_pattern_bindings(
    pattern: &MatchPattern,
    scopes: &mut ScopeStack,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match pattern {
        MatchPattern::Variable(token) | MatchPattern::Binding(token) => {
            scopes.declare(&token.lexeme, token, diagnostics);
        }
        MatchPattern::Tuple(parts) => {
            for part in parts {
                declare_pattern_bindings(part, scopes, diagnostics);
            }
        }
        MatchPattern::EnumVariant { params, .. } => {
            if let Some(params) = params {
                for param in params {
                    declare_pattern_bindings(param, scopes, diagnostics);
                }
            }
        }
        MatchPattern::Literal(_) | MatchPattern::Wildcard => {}
    }
}

fn lint_expr(expr: &Expr, scopes: &mut ScopeStack, diagnostics: &mut Vec<Diagnostic>) {
    match expr {
        Expr::Binary { left, right, .. } | Expr::Logical { left, right, .. } => {
            lint_expr(left, scopes, diagnostics);
            lint_expr(right, scopes, diagnostics);
        }
        Expr::Unary { right, .. } | Expr::Grouping { expression: right } | Expr::Try(right) => {
            lint_expr(right, scopes, diagnostics);
        }
        Expr::Weak(right) => {
            lint_expr(right, scopes, diagnostics);
            if is_scalar_literal(right) {
                diagnostics.push(
                    Diagnostic::new(
                        DiagnosticKind::Lint,
                        "Suspicious weak target: applying `weak` to scalar literals usually has no ownership semantics.",
                        Span::dummy(),
                    )
                    .note("Use `weak` with heap-backed values (arrays, structs, enums, maps, sets or object handles)."),
                );
            }
        }
        Expr::Unowned(right) => {
            lint_expr(right, scopes, diagnostics);
            if is_scalar_literal(right) {
                diagnostics.push(
                    Diagnostic::new(
                        DiagnosticKind::Lint,
                        "Suspicious unowned target: applying `unowned` to scalar literals usually has no ownership semantics.",
                        Span::dummy(),
                    )
                    .note("Use `unowned` only when the referenced heap object lifetime is externally guaranteed."),
                );
            }
        }
        Expr::WeakUpgrade(right) => {
            lint_expr(right, scopes, diagnostics);
            let ok = infer_ref_kind(right, scopes) == Some(RefKind::Weak) || matches!(&**right, Expr::Weak(_));
            if !ok {
                diagnostics.push(
                    Diagnostic::new(
                        DiagnosticKind::Lint,
                        "Weak upgrade misuse: postfix `?` expects a weak reference expression.",
                        Span::dummy(),
                    )
                    .note("Assign `weak expr` (or `weak(...)`) to a variable and apply `?` on that reference."),
                );
            }
        }
        Expr::UnownedAccess(right) => {
            lint_expr(right, scopes, diagnostics);
            let ok =
                infer_ref_kind(right, scopes) == Some(RefKind::Unowned) || matches!(&**right, Expr::Unowned(_));
            if !ok {
                diagnostics.push(
                    Diagnostic::new(
                        DiagnosticKind::Lint,
                        "Unowned access misuse: postfix `!` expects an unowned reference expression.",
                        Span::dummy(),
                    )
                    .note("Create an unowned reference with `unowned expr` (or `unowned(...)`) before using `!`."),
                );
            }
        }
        Expr::Call {
            callee, arguments, ..
        } => {
            lint_expr(callee, scopes, diagnostics);
            for arg in arguments {
                lint_expr(arg, scopes, diagnostics);
            }
        }
        Expr::FieldAccess { object, .. } | Expr::Cast { object, .. } => {
            lint_expr(object, scopes, diagnostics);
        }
        Expr::Array(elements) => {
            for el in elements {
                lint_expr(el, scopes, diagnostics);
            }
        }
        Expr::Tuple(elements) => {
            for el in elements {
                lint_expr(el, scopes, diagnostics);
            }
        }
        Expr::StructInit { fields, .. } => {
            for (_, val) in fields {
                lint_expr(val, scopes, diagnostics);
            }
        }
        Expr::EnumInit { values, .. } => {
            for val in values {
                lint_expr(val, scopes, diagnostics);
            }
        }
        Expr::InterpolatedString(parts) => {
            for part in parts {
                if let InterpolatedPart::Expr { expr: e, .. } = part {
                    lint_expr(e, scopes, diagnostics);
                }
            }
        }
        Expr::SpawnActor { body } => {
            scopes.push();
            for s in body {
                lint_stmt(s, scopes, diagnostics, false);
            }
            scopes.pop();
        }
        Expr::Literal(_) | Expr::Variable { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lexer::lexer::Lexer;
    use parser::parser::Parser;

    fn lint_messages(src: &str) -> Vec<String> {
        let mut lexer = Lexer::new(src.to_string());
        let tokens = lexer.scan_tokens().expect("scan tokens");
        let mut parser = Parser::new(tokens);
        let (program, parse_diags) = parser.parse();
        assert!(parse_diags.is_empty(), "source should parse in lint tests");
        lint_ast(&program)
            .into_iter()
            .map(|d| d.message.to_string())
            .collect()
    }

    #[test]
    fn warns_when_weak_upgrade_is_applied_to_non_weak_expr() {
        let msgs = lint_messages("let arr = [1];\nlet v = arr?;\n");
        assert!(msgs
            .iter()
            .any(|m| m.contains("Weak upgrade misuse: postfix `?` expects a weak reference")));
    }

    #[test]
    fn warns_when_unowned_access_is_applied_to_non_unowned_expr() {
        let msgs = lint_messages("let arr = [1];\nlet v = arr!;\n");
        assert!(msgs
            .iter()
            .any(|m| m.contains("Unowned access misuse: postfix `!` expects an unowned reference")));
    }

    #[test]
    fn accepts_valid_weak_unowned_flow_without_semantic_warnings() {
        let msgs = lint_messages(
            "let arr = [1];\nlet w = weak arr;\nlet x = w?;\nlet u = unowned arr;\nlet y = u!;\nprintln(x);\nprintln(y);\n",
        );
        assert!(!msgs
            .iter()
            .any(|m| m.contains("Weak upgrade misuse") || m.contains("Unowned access misuse")));
    }
}
