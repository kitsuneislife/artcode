use core::ast::{Expr, InterpolatedPart, MatchPattern, Stmt};
use core::Token;
use diagnostics::{Diagnostic, DiagnosticKind, Span};
use std::collections::HashSet;

pub fn lint_ast(program: &[Stmt]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut scope_stack = ScopeStack::new();

    for stmt in program {
        lint_stmt(stmt, &mut scope_stack, &mut diagnostics);
    }

    diagnostics
}

struct ScopeStack {
    scopes: Vec<HashSet<String>>,
}

impl ScopeStack {
    fn new() -> Self {
        Self {
            scopes: vec![HashSet::new()],
        }
    }

    fn push(&mut self) {
        self.scopes.push(HashSet::new());
    }

    fn pop(&mut self) {
        self.scopes.pop();
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
}

fn lint_stmt(stmt: &Stmt, scopes: &mut ScopeStack, diagnostics: &mut Vec<Diagnostic>) {
    match stmt {
        Stmt::Let {
            name, initializer, ..
        } => {
            lint_expr(initializer, scopes, diagnostics);
            scopes.declare(&name.lexeme, name, diagnostics);
        }
        Stmt::Function {
            name, params, body, ..
        } => {
            scopes.declare(&name.lexeme, name, diagnostics);
            scopes.push();
            for param in params {
                scopes.declare(&param.name.lexeme, &param.name, diagnostics);
            }
            lint_stmt(body, scopes, diagnostics);
            scopes.pop();
        }
        Stmt::Block { statements }
        | Stmt::Performant { statements }
        | Stmt::SpawnActor { body: statements } => {
            scopes.push();
            for s in statements {
                lint_stmt(s, scopes, diagnostics);
            }
            scopes.pop();
        }
        Stmt::If {
            condition,
            then_branch,
            else_branch,
        } => {
            lint_expr(condition, scopes, diagnostics);
            lint_stmt(then_branch, scopes, diagnostics);
            if let Some(els) = else_branch {
                lint_stmt(els, scopes, diagnostics);
            }
        }
        Stmt::Expression(expr) | Stmt::Return { value: Some(expr) } => {
            lint_expr(expr, scopes, diagnostics);
        }
        Stmt::Return { value: None }
        | Stmt::Import { .. }
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
                lint_stmt(body, scopes, diagnostics);
                scopes.pop();
            }
        }
    }
}

fn lint_expr(expr: &Expr, scopes: &mut ScopeStack, diagnostics: &mut Vec<Diagnostic>) {
    match expr {
        Expr::Binary { left, right, .. } | Expr::Logical { left, right, .. } => {
            lint_expr(left, scopes, diagnostics);
            lint_expr(right, scopes, diagnostics);
        }
        Expr::Unary { right, .. }
        | Expr::Grouping { expression: right }
        | Expr::Try(right)
        | Expr::Weak(right)
        | Expr::Unowned(right)
        | Expr::WeakUpgrade(right)
        | Expr::UnownedAccess(right) => {
            lint_expr(right, scopes, diagnostics);
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
                lint_stmt(s, scopes, diagnostics);
            }
            scopes.pop();
        }
        Expr::Literal(_) | Expr::Variable { .. } => {}
    }
}
