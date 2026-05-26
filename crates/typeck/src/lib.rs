use core::ast::{ArtValue, Expr, MatchPattern, Stmt};
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
        }
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
}
