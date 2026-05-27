use core::ast::{ArtValue, Expr, MatchPattern, Stmt, TemplateAttrValue, TemplateNode};
use core::types::Type;
use diagnostics::{Diagnostic, DiagnosticKind, Span};
use std::collections::HashMap;

/// Compile-time type checker for Artcode.
///
/// Provides:
/// - Local type inference (`let x = 42` → `x: Int`)
/// - Annotated function verification (call site vs. declared param types)
/// - Parametric type inference (`func f<T>(x: T)` → T inferred from arguments)
pub struct TypeChecker {
    functions: HashMap<String, FuncSig>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Clone)]
struct FuncSig {
    params: Vec<(String, Type)>,
    return_type: Type,
    type_params: Vec<(String, Option<String>)>,
}

struct Env {
    scopes: Vec<HashMap<String, Type>>,
}

impl Env {
    fn new() -> Self {
        Self { scopes: vec![HashMap::new()] }
    }

    fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn set(&mut self, name: &str, ty: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    fn get(&self, name: &str) -> Option<&Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(t) = scope.get(name) {
                return Some(t);
            }
        }
        None
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            diagnostics: Vec::new(),
        }
    }

    /// Run type checking over a program. Returns slice of collected diagnostics.
    pub fn check(&mut self, program: &[Stmt]) -> &[Diagnostic] {
        let mut env = Env::new();
        for stmt in program {
            self.collect_decl(stmt, &mut env);
        }
        for stmt in program {
            self.check_stmt(stmt, &mut env);
        }
        &self.diagnostics
    }

    /// First pass: register all top-level function signatures so call sites can
    /// be verified even when a function is called before its declaration.
    fn collect_decl(&mut self, stmt: &Stmt, env: &mut Env) {
        match stmt {
            Stmt::Function { name, params, return_type, type_params, .. } => {
                let param_types: Vec<(String, Type)> = params
                    .iter()
                    .map(|p| {
                        let ty = p.ty.as_deref()
                            .map(|s| self.parse_type(s))
                            .unwrap_or(Type::Unknown);
                        (p.name.lexeme.clone(), ty)
                    })
                    .collect();
                let ret = return_type.as_deref()
                    .map(|s| self.parse_type(s))
                    .unwrap_or(Type::Unknown);
                let tparams = type_params.clone().unwrap_or_default();
                self.functions.insert(
                    name.lexeme.clone(),
                    FuncSig {
                        params: param_types.clone(),
                        return_type: ret.clone(),
                        type_params: tparams,
                    },
                );
                env.set(
                    &name.lexeme,
                    Type::Function(
                        param_types.iter().map(|(_, t)| t.clone()).collect(),
                        Box::new(ret),
                    ),
                );
            }
            Stmt::ImplBlock { methods, .. } => {
                for m in methods {
                    self.collect_decl(m, env);
                }
            }
            Stmt::ComponentBlock { .. } | Stmt::QualifiedBinding { .. } => {}
            _ => {}
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt, env: &mut Env) {
        match stmt {
            Stmt::Let { pattern, ty, initializer } => {
                let inferred = self.infer_expr(initializer, env);
                let actual_ty = if let Some(ann) = ty {
                    let ann_ty = self.parse_type(ann);
                    if !self.types_compatible(&ann_ty, &inferred)
                        && !matches!(inferred, Type::Unknown)
                    {
                        let span = self.expr_span(initializer);
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Type,
                            format!(
                                "type mismatch: declared as {}, initializer has type {}",
                                ann_ty.name(),
                                inferred.name()
                            ),
                            span,
                        ));
                    }
                    ann_ty
                } else {
                    inferred
                };
                self.bind_pattern(pattern, &actual_ty, env);
            }
            Stmt::Function { params, body, type_params, .. } => {
                env.push();
                for p in params {
                    let ty = p.ty.as_deref()
                        .map(|s| self.parse_type(s))
                        .unwrap_or(Type::Unknown);
                    env.set(&p.name.lexeme, ty);
                }
                for (tp_name, _) in type_params.as_deref().unwrap_or(&[]) {
                    env.set(tp_name, Type::GenericParam(tp_name.clone()));
                }
                self.check_stmt(body, env);
                env.pop();
            }
            Stmt::Block { statements } => {
                env.push();
                for s in statements {
                    self.check_stmt(s, env);
                }
                env.pop();
            }
            Stmt::If { condition, then_branch, else_branch } => {
                self.infer_expr(condition, env);
                self.check_stmt(then_branch, env);
                if let Some(eb) = else_branch {
                    self.check_stmt(eb, env);
                }
            }
            Stmt::IfLet { value, then_branch, else_branch, .. } => {
                self.infer_expr(value, env);
                env.push();
                self.check_stmt(then_branch, env);
                env.pop();
                if let Some(eb) = else_branch {
                    self.check_stmt(eb, env);
                }
            }
            Stmt::While { condition, body } => {
                self.infer_expr(condition, env);
                self.check_stmt(body, env);
            }
            Stmt::For { element, iterator, body } => {
                let iter_ty = self.infer_expr(iterator, env);
                let elem_ty = match iter_ty {
                    Type::Array(inner) => *inner,
                    _ => Type::Unknown,
                };
                env.push();
                env.set(&element.lexeme, elem_ty);
                self.check_stmt(body, env);
                env.pop();
            }
            Stmt::TryCatch { try_branch, catch_name, catch_branch } => {
                self.check_stmt(try_branch, env);
                env.push();
                env.set(&catch_name.lexeme, Type::String);
                self.check_stmt(catch_branch, env);
                env.pop();
            }
            Stmt::Match { expr, cases } => {
                self.infer_expr(expr, env);
                for (_, guard, body) in cases {
                    env.push();
                    if let Some(g) = guard {
                        self.infer_expr(g, env);
                    }
                    self.check_stmt(body, env);
                    env.pop();
                }
            }
            Stmt::ImplBlock { methods, .. } => {
                for m in methods {
                    self.check_stmt(m, env);
                }
            }
            Stmt::Performant { statements } => {
                env.push();
                for s in statements {
                    self.check_stmt(s, env);
                }
                env.pop();
            }
            Stmt::Return { value } => {
                if let Some(v) = value {
                    self.infer_expr(v, env);
                }
            }
            Stmt::Expression(e) => {
                self.infer_expr(e, env);
            }
            Stmt::SpawnActor { body } => {
                env.push();
                for s in body {
                    self.check_stmt(s, env);
                }
                env.pop();
            }
            Stmt::StructDecl { .. }
            | Stmt::EnumDecl { .. }
            | Stmt::Import { .. }
            | Stmt::ShellCommand { .. } => {}
            Stmt::ComponentBlock { bindings, .. } => {
                self.check_component_bindings(bindings, env);
            }
            Stmt::QualifiedBinding { qualifier, name, value, .. } => {
                use core::ast::BindingQualifier;
                // rule: state/memo/ref bindings outside component are an error
                if !matches!(qualifier, BindingQualifier::Prop) {
                    // outside-component check is deferred to component parsing; here just infer
                }
                if let Some(v) = value {
                    let _ty = self.infer_expr(v, env);
                    let _ = name;
                }
            }
        }
    }

    fn check_component_bindings(&mut self, bindings: &[Stmt], env: &mut Env) {
        use core::ast::BindingQualifier;
        let mut state_prop_names: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for b in bindings {
            if let Stmt::QualifiedBinding { qualifier, name, .. } = b {
                if matches!(qualifier, BindingQualifier::State | BindingQualifier::Prop) {
                    state_prop_names.insert(name.lexeme.clone());
                }
            }
        }
        env.push();
        for b in bindings {
            if let Stmt::QualifiedBinding { qualifier, name, value, .. } = b {
                match qualifier {
                    BindingQualifier::Memo => {
                        if let Some(v) = value {
                            let refs = expr_var_refs(v);
                            if !refs.iter().any(|r| state_prop_names.contains(r)) {
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Lint,
                                    format!(
                                        "memo '{}' may be stale — no state or prop found in dep list",
                                        name.lexeme
                                    ),
                                    Span::dummy(),
                                ));
                            }
                        }
                        env.set(&name.lexeme, Type::Unknown);
                    }
                    BindingQualifier::State => {
                        env.set(&name.lexeme, Type::Unknown);
                    }
                    BindingQualifier::Prop | BindingQualifier::Ref => {
                        env.set(&name.lexeme, Type::Unknown);
                    }
                }
            }
        }
        env.pop();
    }

    fn infer_expr(&mut self, expr: &Expr, env: &Env) -> Type {
        match expr {
            Expr::Literal(v) => self.type_of_value(v),
            Expr::Variable { name } => {
                env.get(&name.lexeme).cloned().unwrap_or(Type::Unknown)
            }
            Expr::Array(items) => {
                let inner = items
                    .first()
                    .map(|e| self.infer_expr(e, env))
                    .unwrap_or(Type::Unknown);
                Type::Array(Box::new(inner))
            }
            Expr::Tuple(items) => {
                Type::Tuple(items.iter().map(|e| self.infer_expr(e, env)).collect())
            }
            Expr::InterpolatedString(_) => Type::String,
            Expr::Binary { left, operator, right } => {
                let lt = self.infer_expr(left, env);
                let rt = self.infer_expr(right, env);
                match operator.lexeme.as_str() {
                    "==" | "!=" | "<" | "<=" | ">" | ">=" => Type::Bool,
                    "+" => {
                        if matches!(lt, Type::String) || matches!(rt, Type::String) {
                            Type::String
                        } else {
                            lt
                        }
                    }
                    "-" | "*" | "/" | "%" => {
                        if matches!(lt, Type::Float) || matches!(rt, Type::Float) {
                            Type::Float
                        } else {
                            lt
                        }
                    }
                    _ => Type::Unknown,
                }
            }
            Expr::Logical { .. } => Type::Bool,
            Expr::Unary { operator, right } => {
                if operator.lexeme == "!" {
                    Type::Bool
                } else {
                    self.infer_expr(right, env)
                }
            }
            Expr::Grouping { expression } => self.infer_expr(expression, env),
            Expr::Call { callee, arguments, type_args } => {
                self.infer_call(callee, arguments, type_args, env)
            }
            Expr::FieldAccess { field, .. } => match field.lexeme.as_str() {
                "len" | "size" | "count" => Type::Int,
                _ => Type::Unknown,
            },
            Expr::Cast { target_type, .. } => self.parse_type(target_type),
            Expr::Try(inner)
            | Expr::Weak(inner)
            | Expr::Unowned(inner)
            | Expr::WeakUpgrade(inner)
            | Expr::UnownedAccess(inner) => self.infer_expr(inner, env),
            Expr::StructInit { name, .. } => Type::Struct(name.lexeme.clone()),
            Expr::EnumInit { name, variant, .. } => {
                let n = name
                    .as_ref()
                    .map(|t| t.lexeme.clone())
                    .unwrap_or_else(|| variant.lexeme.clone());
                Type::Enum(n)
            }
            Expr::SpawnActor { .. } => Type::Unknown,
            Expr::Template(nodes) => {
                for node in nodes {
                    self.check_template_node(node, env);
                }
                Type::Unknown
            }
        }
    }

    fn check_template_node(&mut self, node: &TemplateNode, env: &Env) {
        match node {
            TemplateNode::Element { attrs, children, .. }
            | TemplateNode::Component { attrs, children, .. } => {
                for attr in attrs {
                    if let TemplateAttrValue::EventHandler(handler_expr) = &attr.value {
                        let ty = self.infer_expr(handler_expr, env);
                        // Warn only if the type is a known non-callable (literal value type).
                        let is_non_callable = matches!(
                            ty,
                            Type::Int | Type::Float | Type::String | Type::Bool
                        );
                        if is_non_callable {
                            let span = self.expr_span(handler_expr);
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Lint,
                                format!(
                                    "event handler '{}' has type '{}', which is not callable — expected a function",
                                    attr.name,
                                    ty.name()
                                ),
                                span,
                            ));
                        }
                    }
                }
                for child in children {
                    self.check_template_node(child, env);
                }
            }
            TemplateNode::If { cond, then_children, else_children } => {
                self.infer_expr(cond, env);
                for child in then_children {
                    self.check_template_node(child, env);
                }
                for child in else_children {
                    self.check_template_node(child, env);
                }
            }
            TemplateNode::For { items, children, .. } => {
                self.infer_expr(items, env);
                for child in children {
                    self.check_template_node(child, env);
                }
            }
            TemplateNode::Slot { children, .. } => {
                for child in children {
                    self.check_template_node(child, env);
                }
            }
            TemplateNode::Expr(e) => {
                self.infer_expr(e, env);
            }
            TemplateNode::Text(_) => {}
        }
    }

    fn infer_call(
        &mut self,
        callee: &Expr,
        args: &[Expr],
        type_args: &Option<Vec<String>>,
        env: &Env,
    ) -> Type {
        let func_name = match callee {
            Expr::Variable { name } => Some(name.lexeme.clone()),
            Expr::FieldAccess { field, .. } => Some(field.lexeme.clone()),
            _ => None,
        };

        let arg_types: Vec<Type> = args.iter().map(|a| self.infer_expr(a, env)).collect();

        if let Some(name) = &func_name {
            if let Some(sig) = self.functions.get(name).cloned() {
                let span = self.callee_span(callee);
                let bindings = self.resolve_generic_bindings(&sig, type_args, &arg_types);

                let non_self_params: Vec<_> = sig.params
                    .iter()
                    .filter(|(n, _)| n != "self")
                    .collect();

                // Only verify argument count when fully annotated (no Unknown params)
                let all_annotated = non_self_params
                    .iter()
                    .all(|(_, t)| !matches!(t, Type::Unknown));

                if all_annotated && non_self_params.len() == args.len() {
                    for (i, ((param_name, param_ty), arg_ty)) in
                        non_self_params.iter().zip(arg_types.iter()).enumerate()
                    {
                        let resolved = self.substitute(param_ty, &bindings);
                        if !self.types_compatible(&resolved, arg_ty)
                            && !matches!(arg_ty, Type::Unknown)
                        {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Type,
                                format!(
                                    "argument {} ('{}') of '{}': expected {}, got {}",
                                    i + 1,
                                    param_name,
                                    name,
                                    resolved.name(),
                                    arg_ty.name()
                                ),
                                span,
                            ));
                        }
                    }
                }

                return self.substitute(&sig.return_type, &bindings);
            }
        }

        Type::Unknown
    }

    fn resolve_generic_bindings(
        &self,
        sig: &FuncSig,
        explicit: &Option<Vec<String>>,
        arg_types: &[Type],
    ) -> HashMap<String, Type> {
        let mut bindings = HashMap::new();

        if let Some(targs) = explicit {
            for (i, (param_name, _)) in sig.type_params.iter().enumerate() {
                if let Some(ta) = targs.get(i) {
                    bindings.insert(param_name.clone(), self.parse_type(ta));
                }
            }
        }

        let non_self: Vec<_> = sig.params.iter().filter(|(n, _)| n != "self").collect();
        for (i, (_, param_ty)) in non_self.iter().enumerate() {
            if let Some(arg_ty) = arg_types.get(i) {
                self.infer_bindings_rec(param_ty, arg_ty, &mut bindings);
            }
        }

        bindings
    }

    fn infer_bindings_rec(&self, param: &Type, arg: &Type, out: &mut HashMap<String, Type>) {
        match (param, arg) {
            (Type::GenericParam(name), concrete) if !matches!(concrete, Type::Unknown) => {
                out.entry(name.clone()).or_insert_with(|| concrete.clone());
            }
            (Type::Array(p), Type::Array(a)) => self.infer_bindings_rec(p, a, out),
            (Type::Tuple(ps), Type::Tuple(as_)) => {
                for (p, a) in ps.iter().zip(as_.iter()) {
                    self.infer_bindings_rec(p, a, out);
                }
            }
            _ => {}
        }
    }

    fn substitute(&self, ty: &Type, bindings: &HashMap<String, Type>) -> Type {
        match ty {
            Type::GenericParam(name) => {
                bindings.get(name).cloned().unwrap_or(Type::Unknown)
            }
            Type::Array(inner) => {
                Type::Array(Box::new(self.substitute(inner, bindings)))
            }
            Type::Tuple(items) => {
                Type::Tuple(items.iter().map(|t| self.substitute(t, bindings)).collect())
            }
            Type::Function(params, ret) => Type::Function(
                params.iter().map(|p| self.substitute(p, bindings)).collect(),
                Box::new(self.substitute(ret, bindings)),
            ),
            _ => ty.clone(),
        }
    }

    fn types_compatible(&self, expected: &Type, actual: &Type) -> bool {
        if expected == actual {
            return true;
        }
        match (expected, actual) {
            (Type::Unknown, _) | (_, Type::Unknown) => true,
            (Type::GenericParam(_), _) | (_, Type::GenericParam(_)) => true,
            // Int coerces to Float in arithmetic
            (Type::Float, Type::Int) => true,
            (Type::Array(a), Type::Array(b)) => self.types_compatible(a, b),
            (Type::Tuple(a), Type::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| self.types_compatible(x, y))
            }
            _ => false,
        }
    }

    fn bind_pattern(&mut self, pattern: &MatchPattern, ty: &Type, env: &mut Env) {
        match pattern {
            MatchPattern::Variable(tok) | MatchPattern::Binding(tok) => {
                env.set(&tok.lexeme, ty.clone());
            }
            MatchPattern::Tuple(pats) => {
                let types = match ty {
                    Type::Tuple(ts) => ts.clone(),
                    _ => vec![Type::Unknown; pats.len()],
                };
                for (p, t) in pats.iter().zip(types.iter()) {
                    self.bind_pattern(p, t, env);
                }
            }
            MatchPattern::EnumVariant { params: Some(pats), .. } => {
                for p in pats {
                    self.bind_pattern(p, &Type::Unknown, env);
                }
            }
            MatchPattern::Literal(_)
            | MatchPattern::Wildcard
            | MatchPattern::EnumVariant { params: None, .. } => {}
        }
    }

    /// Convert a type annotation string (from the AST) to a `Type`.
    pub fn parse_type(&self, s: &str) -> Type {
        let s = s.trim();
        match s {
            "Int" | "i64" | "int" => Type::Int,
            "Float" | "f64" | "float" => Type::Float,
            "Bool" | "bool" => Type::Bool,
            "String" | "str" | "string" => Type::String,
            "Buffer" | "buffer" => Type::Buffer,
            "None" | "()" => Type::None,
            _ if s.starts_with("Array<") && s.ends_with('>') => {
                Type::Array(Box::new(self.parse_type(&s[6..s.len() - 1])))
            }
            _ if s.starts_with('[') && s.ends_with(']') => {
                Type::Array(Box::new(self.parse_type(&s[1..s.len() - 1])))
            }
            _ if s.starts_with("Result<") && s.ends_with('>') => {
                let inner = &s[7..s.len() - 1];
                let params = inner.splitn(2, ',').map(|p| self.parse_type(p.trim())).collect();
                Type::EnumInstance("Result".to_string(), params)
            }
            _ if s.starts_with("Option<") && s.ends_with('>') => {
                let inner = &s[7..s.len() - 1];
                Type::EnumInstance("Option".to_string(), vec![self.parse_type(inner)])
            }
            // Single uppercase letter = generic type parameter
            _ if s.len() == 1 && s.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false) => {
                Type::GenericParam(s.to_string())
            }
            _ if s.chars().next().map(|c| c.is_ascii_uppercase()).unwrap_or(false) => {
                Type::Struct(s.to_string())
            }
            // Anything else treated as a generic parameter name
            _ => Type::GenericParam(s.to_string()),
        }
    }

    fn type_of_value(&self, v: &ArtValue) -> Type {
        match v {
            ArtValue::Int(_) => Type::Int,
            ArtValue::Float(_) => Type::Float,
            ArtValue::Bool(_) => Type::Bool,
            ArtValue::String(_) => Type::String,
            ArtValue::Optional(inner) if inner.is_none() => Type::None,
            ArtValue::Buffer(_) => Type::Buffer,
            _ => Type::Unknown,
        }
    }

    fn expr_span(&self, expr: &Expr) -> Span {
        match expr {
            Expr::Variable { name } => Span::new(name.start, name.end, name.line, name.col),
            Expr::Call { callee, .. } => self.callee_span(callee),
            Expr::FieldAccess { field, .. } => {
                Span::new(field.start, field.end, field.line, field.col)
            }
            _ => Span::dummy(),
        }
    }

    fn callee_span(&self, callee: &Expr) -> Span {
        match callee {
            Expr::Variable { name } => Span::new(name.start, name.end, name.line, name.col),
            Expr::FieldAccess { field, .. } => {
                Span::new(field.start, field.end, field.line, field.col)
            }
            _ => Span::dummy(),
        }
    }
}

fn expr_var_refs(expr: &Expr) -> Vec<String> {
    let mut out = Vec::new();
    collect_refs(expr, &mut out);
    out
}

fn collect_refs(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Variable { name } => out.push(name.lexeme.clone()),
        Expr::Binary { left, right, .. } | Expr::Logical { left, right, .. } => {
            collect_refs(left, out);
            collect_refs(right, out);
        }
        Expr::Unary { right, .. } => collect_refs(right, out),
        Expr::Call { callee, arguments, .. } => {
            collect_refs(callee, out);
            for a in arguments {
                collect_refs(a, out);
            }
        }
        Expr::Array(items) | Expr::Tuple(items) => {
            for i in items {
                collect_refs(i, out);
            }
        }
        Expr::FieldAccess { object, .. } => collect_refs(object, out),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Vec<Stmt> {
        let tokens = lexer::Lexer::new(src.to_string()).scan_tokens().expect("lex");
        let (stmts, _diags) = parser::Parser::new(tokens).parse();
        stmts
    }

    #[test]
    fn test_local_inference_int() {
        let mut tc = TypeChecker::new();
        let prog = parse("let x = 42\nlet y = x");
        tc.check(&prog);
        assert!(tc.diagnostics.is_empty(), "{:?}", tc.diagnostics);
    }

    #[test]
    fn test_local_inference_string() {
        let mut tc = TypeChecker::new();
        let prog = parse(r#"let s = "hello""#);
        tc.check(&prog);
        assert!(tc.diagnostics.is_empty(), "{:?}", tc.diagnostics);
    }

    #[test]
    fn test_annotated_type_match() {
        let mut tc = TypeChecker::new();
        let prog = parse("let x: Int = 42");
        tc.check(&prog);
        assert!(tc.diagnostics.is_empty(), "{:?}", tc.diagnostics);
    }

    #[test]
    fn test_annotated_type_mismatch() {
        let mut tc = TypeChecker::new();
        let prog = parse(r#"let x: Int = "hello""#);
        tc.check(&prog);
        assert!(!tc.diagnostics.is_empty());
        assert!(tc.diagnostics[0].message.contains("Int"));
    }

    #[test]
    fn test_function_arg_type_ok() {
        let mut tc = TypeChecker::new();
        let prog = parse(
            "func add(a: Int, b: Int) -> Int { return a + b }\nlet r = add(1, 2)",
        );
        tc.check(&prog);
        assert!(tc.diagnostics.is_empty(), "{:?}", tc.diagnostics);
    }

    #[test]
    fn test_function_arg_type_mismatch() {
        let mut tc = TypeChecker::new();
        let prog = parse(
            r#"func greet(name: String) -> String { return name }
greet(42)"#,
        );
        tc.check(&prog);
        assert!(!tc.diagnostics.is_empty());
        assert!(tc.diagnostics[0].message.contains("String"));
    }

    #[test]
    fn test_generic_identity_inference() {
        let mut tc = TypeChecker::new();
        let prog = parse("func identity<T>(x: T) -> T { return x }\nlet r = identity(42)");
        tc.check(&prog);
        assert!(tc.diagnostics.is_empty(), "{:?}", tc.diagnostics);
    }

    #[test]
    fn test_generic_array_inference() {
        let mut tc = TypeChecker::new();
        let prog =
            parse("func first<T>(arr: Array<T>) -> T { return arr[0] }\nlet r = first([1, 2, 3])");
        tc.check(&prog);
        assert!(tc.diagnostics.is_empty(), "{:?}", tc.diagnostics);
    }

    #[test]
    fn test_parse_type_primitives() {
        let tc = TypeChecker::new();
        assert_eq!(tc.parse_type("Int"), Type::Int);
        assert_eq!(tc.parse_type("Float"), Type::Float);
        assert_eq!(tc.parse_type("Bool"), Type::Bool);
        assert_eq!(tc.parse_type("String"), Type::String);
        assert_eq!(tc.parse_type("None"), Type::None);
    }

    #[test]
    fn test_parse_type_array() {
        let tc = TypeChecker::new();
        assert_eq!(tc.parse_type("Array<Int>"), Type::Array(Box::new(Type::Int)));
        assert_eq!(tc.parse_type("[String]"), Type::Array(Box::new(Type::String)));
    }

    #[test]
    fn test_parse_type_generic_param() {
        let tc = TypeChecker::new();
        assert_eq!(tc.parse_type("T"), Type::GenericParam("T".to_string()));
    }

    #[test]
    fn test_for_loop_element_type() {
        let mut tc = TypeChecker::new();
        let prog = parse(
            r#"func process(items: Array<Int>) {
    for item in items {
        let doubled = item + item
    }
}"#,
        );
        tc.check(&prog);
        assert!(tc.diagnostics.is_empty(), "{:?}", tc.diagnostics);
    }

    #[test]
    fn test_template_event_handler_non_callable_warns() {
        let mut tc = TypeChecker::new();
        // x is an Int — using it as an event handler should produce a lint warning
        let prog = parse("let x = 42;\nlet el = <button on:click={x}>ok</button>;");
        tc.check(&prog);
        let has_warn = tc.diagnostics.iter().any(|d| d.message.contains("not callable"));
        assert!(has_warn, "Expected non-callable event handler warning. Got: {:?}", tc.diagnostics);
    }

    #[test]
    fn test_template_event_handler_function_no_warn() {
        let mut tc = TypeChecker::new();
        // handler is a function — no warning expected
        let prog = parse("func handler() { }\nlet el = <button on:click={handler}>ok</button>;");
        tc.check(&prog);
        let has_warn = tc.diagnostics.iter().any(|d| d.message.contains("not callable"));
        assert!(!has_warn, "Should not warn for function event handler. Got: {:?}", tc.diagnostics);
    }

    // ── Bloco B — component binding tests ────────────────────────────────────

    #[test]
    fn component_valid_state_and_view() {
        let mut tc = TypeChecker::new();
        let prog = parse("component Counter {\n  state count: Int = 0\n  view { <p>{count}</p> }\n}");
        tc.check(&prog);
        // no errors expected for a valid component
        let errors: Vec<_> = tc.diagnostics.iter().filter(|d| d.kind == DiagnosticKind::Type).collect();
        assert!(errors.is_empty(), "Unexpected type errors: {:?}", errors);
    }

    #[test]
    fn component_valid_prop_binding() {
        let mut tc = TypeChecker::new();
        let prog = parse("component Greeter {\n  prop name: String\n  view { <p>{name}</p> }\n}");
        tc.check(&prog);
        let errors: Vec<_> = tc.diagnostics.iter().filter(|d| d.kind == DiagnosticKind::Type).collect();
        assert!(errors.is_empty(), "Unexpected type errors: {:?}", errors);
    }

    #[test]
    fn component_memo_with_state_dep_no_warn() {
        let mut tc = TypeChecker::new();
        let prog = parse("component Calc {\n  state x: Int = 1\n  memo doubled: Int = x * 2\n  view { <p>{doubled}</p> }\n}");
        tc.check(&prog);
        let stale_warns: Vec<_> = tc.diagnostics.iter().filter(|d| d.message.contains("may be stale")).collect();
        assert!(stale_warns.is_empty(), "Should not warn when memo refs state: {:?}", stale_warns);
    }

    #[test]
    fn component_memo_without_state_dep_warns() {
        let mut tc = TypeChecker::new();
        let prog = parse("component Isolated {\n  memo val: Int = 42\n  view { <p>{val}</p> }\n}");
        tc.check(&prog);
        let stale_warns: Vec<_> = tc.diagnostics.iter().filter(|d| d.message.contains("may be stale")).collect();
        assert!(!stale_warns.is_empty(), "Expected stale-memo warning. Got: {:?}", tc.diagnostics);
    }

    #[test]
    fn parser_component_block_is_ast_node() {
        use core::ast::Stmt;
        let prog = parse("component Btn {\n  state clicked: Bool = false\n  view { <button>{clicked}</button> }\n}");
        assert_eq!(prog.len(), 1);
        assert!(matches!(prog[0], Stmt::ComponentBlock { .. }), "Expected ComponentBlock, got: {:?}", prog[0]);
    }

    #[test]
    fn parser_state_binding_inside_component() {
        use core::ast::{BindingQualifier, Stmt};
        let prog = parse("component X {\n  state n: Int = 0\n  view { <div></div> }\n}");
        if let Stmt::ComponentBlock { bindings, .. } = &prog[0] {
            assert!(!bindings.is_empty(), "Expected at least one binding");
            assert!(matches!(&bindings[0], Stmt::QualifiedBinding { qualifier: BindingQualifier::State, .. }));
        } else {
            panic!("Expected ComponentBlock");
        }
    }

    #[test]
    fn parser_prop_binding_inside_component() {
        use core::ast::{BindingQualifier, Stmt};
        let prog = parse("component Label {\n  prop text: String\n  view { <span>{text}</span> }\n}");
        if let Stmt::ComponentBlock { bindings, .. } = &prog[0] {
            assert!(matches!(&bindings[0], Stmt::QualifiedBinding { qualifier: BindingQualifier::Prop, .. }));
        } else {
            panic!("Expected ComponentBlock");
        }
    }

    #[test]
    fn parser_multiple_qualifiers_in_component() {
        use core::ast::{BindingQualifier, Stmt};
        let prog = parse("component Multi {\n  state x: Int = 0\n  prop y: String\n  memo z: Int = x + 1\n  view { <div></div> }\n}");
        if let Stmt::ComponentBlock { bindings, .. } = &prog[0] {
            assert_eq!(bindings.len(), 3);
            assert!(matches!(&bindings[0], Stmt::QualifiedBinding { qualifier: BindingQualifier::State, .. }));
            assert!(matches!(&bindings[1], Stmt::QualifiedBinding { qualifier: BindingQualifier::Prop, .. }));
            assert!(matches!(&bindings[2], Stmt::QualifiedBinding { qualifier: BindingQualifier::Memo, .. }));
        } else {
            panic!("Expected ComponentBlock");
        }
    }
}
