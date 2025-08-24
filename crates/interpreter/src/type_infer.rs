use core::{ArtValue, Expr, Program, Stmt, Type, InterpolatedPart};
use diagnostics::{Diagnostic, DiagnosticKind, Span};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct TypeEnv {
    types: HashMap<usize, Type>,
    pub vars: HashMap<String, Type>,
}

impl TypeEnv {
    pub fn new() -> Self {
        Self {
            types: HashMap::new(),
            vars: HashMap::new(),
        }
    }
    fn id(expr: &Expr) -> usize {
        expr as *const _ as usize
    }
    fn set(&mut self, expr: &Expr, t: Type) {
        self.types.insert(Self::id(expr), t);
    }
    pub fn set_var(&mut self, name: &str, t: Type) {
        self.vars.insert(name.to_string(), t);
    }
    pub fn get_var(&self, name: &str) -> Option<&Type> {
        self.vars.get(name)
    }
    pub fn get(&self, expr: &Expr) -> Option<&Type> {
        self.types.get(&Self::id(expr))
    }
}

pub struct TypeInfer<'a> {
    pub diags: Vec<Diagnostic>,
    tenv: &'a mut TypeEnv,
    enums: HashMap<String, HashMap<String, Option<usize>>>,
    // lexical scopes stack: each scope maps declared variable names
    scopes: Vec<HashSet<String>>,
}

impl<'a> TypeInfer<'a> {
    pub fn new(tenv: &'a mut TypeEnv) -> Self {
        Self {
            diags: Vec::new(),
            tenv,
            enums: HashMap::new(),
            scopes: vec![HashSet::new()],
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashSet::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare_var(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string());
        }
    }

    fn visible_vars(&self) -> HashSet<String> {
        let mut out = HashSet::new();
        for s in &self.scopes {
            for n in s {
                out.insert(n.clone());
            }
        }
        // include tenv globals as well
        for k in self.tenv.vars.keys() {
            out.insert(k.clone());
        }
        out
    }

    pub fn run(&mut self, program: &Program) -> Result<(), Vec<Diagnostic>> {
        for stmt in program {
            self.visit_stmt(stmt);
        }
        // If any type diagnostics were produced, treat them as errors and return them.
        let type_diags: Vec<Diagnostic> = self
            .diags
            .iter()
            .filter(|d| matches!(d.kind, DiagnosticKind::Type))
            .cloned()
            .collect();
        if !type_diags.is_empty() {
            return Err(type_diags);
        }
        Ok(())
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Expression(e) => {
                self.infer_expr(e);
            }
            Stmt::Let {
                name, initializer, ..
            } => {
                let t = self.infer_expr(initializer);
                self.tenv.set_var(&name.lexeme, t);
                // declare in current lexical scope
                self.declare_var(&name.lexeme);
            }
            Stmt::Block { statements } => {
                self.push_scope();
                for s in statements {
                    self.visit_stmt(s);
                }
                self.pop_scope();
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.infer_expr(condition);
                self.visit_stmt(then_branch);
                if let Some(e) = else_branch {
                    self.visit_stmt(e);
                }
            }
            Stmt::EnumDecl { name, variants } => {
                let mut map = HashMap::new();
                for (v, params) in variants {
                    map.insert(v.lexeme.clone(), params.as_ref().map(|p| p.len()));
                }
                self.enums.insert(name.lexeme.clone(), map);
            }
            Stmt::StructDecl { .. }
            | Stmt::Function { .. }
            | Stmt::Return { .. }
            | Stmt::Match { .. }
            | Stmt::Import { .. } => {}
            Stmt::Performant { statements } => {
                self.check_performant_block(statements);
            }
            Stmt::SpawnActor { body } => {
                // For now, accept top-level actor declarations but don't deeply type-check their body here.
                let _ = body;
            }
        }
    }

    // Minimal static escape analysis: `performant` blocks must not contain `return` statements
    // that would allow arena-allocated composites to escape the block. This is a conservative
    // check implemented early in the pipeline. More checks (assignments to outer scopes,
    // closures capturing arena values) will be added later.
    fn check_performant_block(&mut self, statements: &Vec<Stmt>) {
        // Use lexical symbol table to determine which variables are outer vs local.
        let outer_vars = self.visible_vars();
        // Create a fresh local scope for the performant block so we can track declarations.
        self.push_scope();
        for s in statements {
            self.check_performant_stmt(s, &outer_vars);
        }
        self.pop_scope();
    }
    fn check_performant_stmt(&mut self, stmt: &Stmt, outer_vars: &HashSet<String>) {
        use Stmt::*;
        match stmt {
            Return { value: _ } => {
                // Conservative error: returning from performant may expose arena-allocated composites.
                self.diags.push(Diagnostic::new(
                    DiagnosticKind::Type,
                    "`return` is not allowed inside `performant` blocks: it may allow arena-allocated references to escape".to_string(),
                    Span::new(0,0,0,0),
                ));
            }
            Function { name, .. } => {
                self.diags.push(Diagnostic::new(
                    DiagnosticKind::Type,
                    format!("Function declaration '{}' is not allowed inside `performant` blocks: closures may capture arena values and escape", name.lexeme),
                    Span::new(name.start, name.end, name.line, name.col),
                ));
            }
            Let {
                name, initializer, ..
            } => {
                // If this let shadows a visible outer variable (assignment to outer scope), report a type error.
                if outer_vars.contains(&name.lexeme) {
                    self.diags.push(Diagnostic::new(
                        DiagnosticKind::Type,
                        format!("Assignment to outer-scope variable '{}' inside `performant` is not allowed: may extend lifetime of arena values", name.lexeme),
                        Span::new(name.start, name.end, name.line, name.col),
                    ));
                }
                // Declare this name in the current lexical scope so nested checks know it's local.
                self.declare_var(&name.lexeme);
                // Se inicializador é potencialmente composto, emitir aviso conservador.
                // Suprimir para bindings que começam com '_' (convencionalmente temporários).
                match initializer {
                    Expr::Array(_)
                    | Expr::StructInit { .. }
                    | Expr::EnumInit { .. }
                    | Expr::Call { .. } => {
                        if !name.lexeme.starts_with('_') {
                            self.diags.push(Diagnostic::new(
                                DiagnosticKind::Type,
                                format!("Variable '{}' initialized with a composite value inside `performant` — ensure it does not escape the block", name.lexeme),
                                Span::new(name.start, name.end, name.line, name.col),
                            ));
                        }
                    }
                    _ => {}
                }
                // Conservative capture check: if the initializer expression uses any variables
                // from the outer scope (and they are not local declarations), that's a potential
                // capture of outer arena values and should be rejected.
                let current_locals = self.visible_vars();
                let captures = self.expr_uses_outer_vars(initializer, &current_locals, outer_vars);
                if !captures.is_empty() {
                    for cap in captures {
                        self.diags.push(Diagnostic::new(
                            DiagnosticKind::Type,
                            format!("Initializer for '{}' references outer variable '{}', which may capture/escape arena values", name.lexeme, cap),
                            Span::new(name.start, name.end, name.line, name.col),
                        ));
                    }
                }
                // Conservative extra: if the variable name looks like a non-temporary (doesn't start with _)
                // we warn that assigning to an outer-named variable may escape. This is a heuristic
                // until a full symbol-table/outer-scope analysis is implemented.
                // If the binding starts with '_' it's a temporary by convention; suppress this
                // heuristic diagnostic which otherwise triggers for normal names.
                if !name.lexeme.starts_with('_') {
                    self.diags.push(Diagnostic::new(
                        DiagnosticKind::Type,
                        format!("Binding '{}' inside `performant` may escape if it refers to an outer scope variable. Avoid assigning to outer names or prefix temporaries with '_'", name.lexeme),
                        Span::new(name.start, name.end, name.line, name.col),
                    ));
                }
            }
            Block { statements } => {
                // New block introduces nested lexical scope within performant
                self.push_scope();
                for s in statements {
                    self.check_performant_stmt(s, outer_vars);
                }
                self.pop_scope();
            }
            If {
                condition: _,
                then_branch,
                else_branch,
            } => {
                self.check_performant_stmt(then_branch, outer_vars);
                if let Some(e) = else_branch {
                    self.check_performant_stmt(e, outer_vars);
                }
            }
            Match { expr: _, cases } => {
                for (_pat, _guard, body) in cases {
                    self.check_performant_stmt(body, outer_vars);
                }
            }
            Performant { statements } => {
                // Nested performant: create a fresh scope and recurse
                self.push_scope();
                for s in statements {
                    self.check_performant_stmt(s, outer_vars);
                }
                self.pop_scope();
            }
            SpawnActor { .. } => {
                self.diags.push(Diagnostic::new(
                    DiagnosticKind::Type,
                    "spawn actor is not allowed inside performant blocks".to_string(),
                    Span::new(0,0,0,0),
                ));
            }
            StructDecl { .. } | EnumDecl { .. } | Expression(_) | Import { .. } => { /* allowed */ }
        }
    }

    // Walk an expression and collect any variable names that are from outer scope (i.e. present
    // in `outer_vars`) but not declared in `local_decls`. This is conservative: any such usage
    // may capture an outer value into a local that lives beyond the arena.
    #[allow(clippy::only_used_in_recursion)]
    fn expr_uses_outer_vars(&self, expr: &Expr, current_locals: &HashSet<String>, outer_vars: &HashSet<String>) -> Vec<String> {
        use Expr::*;
        let mut found: Vec<String> = Vec::new();
        match expr {
            Variable { name } => {
                let n = &name.lexeme;
                if outer_vars.contains(n) && !current_locals.contains(n) {
                    found.push(n.clone());
                }
            }
            Grouping { expression } => {
                found.extend(self.expr_uses_outer_vars(expression, current_locals, outer_vars));
            }
            Unary { right, .. } => {
                found.extend(self.expr_uses_outer_vars(right, current_locals, outer_vars));
            }
            Binary { left, right, .. } => {
                found.extend(self.expr_uses_outer_vars(left, current_locals, outer_vars));
                found.extend(self.expr_uses_outer_vars(right, current_locals, outer_vars));
            }
            Logical { left, right, .. } => {
                found.extend(self.expr_uses_outer_vars(left, current_locals, outer_vars));
                found.extend(self.expr_uses_outer_vars(right, current_locals, outer_vars));
            }
            Call { callee, arguments } => {
                found.extend(self.expr_uses_outer_vars(callee, current_locals, outer_vars));
                for a in arguments {
                    found.extend(self.expr_uses_outer_vars(a, current_locals, outer_vars));
                }
            }
            StructInit { fields, .. } => {
                for (_k, v) in fields {
                    found.extend(self.expr_uses_outer_vars(v, current_locals, outer_vars));
                }
            }
            EnumInit { values, .. } => {
                for v in values {
                    found.extend(self.expr_uses_outer_vars(v, current_locals, outer_vars));
                }
            }
            FieldAccess { object, .. } => {
                found.extend(self.expr_uses_outer_vars(object, current_locals, outer_vars));
            }
            Weak(inner)
            | Unowned(inner)
            | WeakUpgrade(inner)
            | UnownedAccess(inner)
            | Try(inner) => {
                found.extend(self.expr_uses_outer_vars(inner, current_locals, outer_vars));
            }
            Array(elements) => {
                for e in elements {
                    found.extend(self.expr_uses_outer_vars(e, current_locals, outer_vars));
                }
            }
            InterpolatedString(parts) => {
                // parts are InterpolatedPart: either Literal(String) or Expr { expr, format }
                for p in parts {
                    match p {
                        InterpolatedPart::Literal(_) => {}
                        InterpolatedPart::Expr { expr, .. } => {
                            found.extend(self.expr_uses_outer_vars(expr, current_locals, outer_vars));
                        }
                    }
                }
            }
            Cast { object, .. } => {
                found.extend(self.expr_uses_outer_vars(object, current_locals, outer_vars));
            }
            Literal(_) => {}
        }
        // Deduplicate
        found.sort();
        found.dedup();
        found
    }

    fn infer_expr(&mut self, expr: &Expr) -> Type {
        use Expr::*;
        let t = match expr {
            Literal(v) => value_type(v),
            Grouping { expression } => self.infer_expr(expression),
            Unary { right, .. } => self.infer_expr(right),
            Binary {
                left,
                operator,
                right,
            } => {
                let lt = self.infer_expr(left);
                let rt = self.infer_expr(right);
                match (lt, rt) {
                    (Type::Int, Type::Int) => Type::Int,
                    (Type::Float, Type::Float) => Type::Float,
                    (Type::Int, Type::Float) | (Type::Float, Type::Int) => Type::Float,
                    (Type::String, Type::String) => Type::String,
                    _ => {
                        self.diags.push(Diagnostic::new(
                            DiagnosticKind::Type,
                            "Invalid types for binary operator".to_string(),
                            Span::new(operator.start, operator.end, operator.line, operator.col),
                        ));
                        Type::Unknown
                    }
                }
            }
            Logical { left, right, .. } => {
                self.infer_expr(left);
                self.infer_expr(right);
                Type::Bool
            }
            Variable { name } => self
                .tenv
                .get_var(&name.lexeme)
                .cloned()
                .unwrap_or(Type::Unknown),
            Call { callee, arguments } => {
                self.infer_expr(callee);
                for a in arguments {
                    self.infer_expr(a);
                }
                Type::Unknown
            }
            StructInit { .. } => Type::Unknown,
            EnumInit {
                name,
                variant,
                values,
            } => {
                let mut val_types = Vec::new();
                for v in values {
                    val_types.push(self.infer_expr(v));
                }
                if let Some(n) = name {
                    if let Some(vmap) = self.enums.get(&n.lexeme) {
                        if let Some(expected) = vmap.get(&variant.lexeme) {
                            let exp = expected.unwrap_or(0);
                            if exp != val_types.len() {
                                self.diags.push(Diagnostic::new(
                                    DiagnosticKind::Type,
                                    format!(
                                        "Enum variant '{}' expects {} arguments, found {}",
                                        variant.lexeme,
                                        exp,
                                        val_types.len()
                                    ),
                                    Span::new(
                                        variant.start,
                                        variant.end,
                                        variant.line,
                                        variant.col,
                                    ),
                                ));
                                Type::Enum(n.lexeme.clone())
                            } else {
                                Type::EnumInstance(n.lexeme.clone(), val_types)
                            }
                        } else {
                            self.diags.push(Diagnostic::new(
                                DiagnosticKind::Type,
                                format!(
                                    "Unknown enum variant '{}' for enum '{}'.",
                                    variant.lexeme, n.lexeme
                                ),
                                Span::new(variant.start, variant.end, variant.line, variant.col),
                            ));
                            Type::Enum(n.lexeme.clone())
                        }
                    } else {
                        Type::Enum(n.lexeme.clone())
                    }
                } else {
                    let mut candidates = Vec::new();
                    for (ename, vmap) in &self.enums {
                        if vmap.contains_key(&variant.lexeme) {
                            candidates.push(ename);
                        }
                    }
                    if candidates.len() == 1 {
                        let ename = candidates[0].clone();
                        if let Some(vmap) = self.enums.get(&ename)
                            && let Some(expected) = vmap.get(&variant.lexeme)
                        {
                            let exp = expected.unwrap_or(0);
                            if exp != val_types.len() {
                                self.diags.push(Diagnostic::new(
                                    DiagnosticKind::Type,
                                    format!(
                                        "Enum variant '{}' expects {} arguments, found {}",
                                        variant.lexeme,
                                        exp,
                                        val_types.len()
                                    ),
                                    Span::new(
                                        variant.start,
                                        variant.end,
                                        variant.line,
                                        variant.col,
                                    ),
                                ));
                            }
                            return Type::EnumInstance(ename.clone(), val_types);
                        }
                        Type::Enum(ename.clone())
                    } else if candidates.len() > 1 {
                        self.diags.push(Diagnostic::new(
                            DiagnosticKind::Type,
                            "Ambiguous enum variant shorthand.".to_string(),
                            Span::new(variant.start, variant.end, variant.line, variant.col),
                        ));
                        Type::Unknown
                    } else {
                        Type::Unknown
                    }
                }
            }
            FieldAccess { object, .. } => self.infer_expr(object),
            Try(inner) => self.infer_expr(inner), // legado
            Weak(inner) => {
                self.infer_expr(inner);
                Type::Unknown
            }
            Unowned(inner) => {
                self.infer_expr(inner);
                Type::Unknown
            }
            WeakUpgrade(inner) => {
                self.infer_expr(inner);
                Type::None
            }
            UnownedAccess(inner) => {
                self.infer_expr(inner);
                Type::Unknown
            }
            Array(elements) => {
                if let Some(first) = elements.first() {
                    Type::Array(Box::new(self.infer_expr(first)))
                } else {
                    Type::Array(Box::new(Type::Unknown))
                }
            }
            Cast { target_type, .. } => Type::Struct(target_type.clone()),
            InterpolatedString(_) => Type::String,
        };
        self.tenv.set(expr, t.clone());
        t
    }
}

fn value_type(v: &ArtValue) -> Type {
    match v {
        ArtValue::Int(_) => Type::Int,
        ArtValue::Float(_) => Type::Float,
        ArtValue::Bool(_) => Type::Bool,
        ArtValue::String(_) => Type::String,
        ArtValue::Optional(_) => Type::None,
        ArtValue::Array(vals) => {
            if let Some(first) = vals.first() {
                Type::Array(Box::new(value_type(first)))
            } else {
                Type::Array(Box::new(Type::Unknown))
            }
        }
        ArtValue::StructInstance { struct_name, .. } => Type::Struct(struct_name.clone()),
        ArtValue::EnumInstance { enum_name, .. } => Type::Enum(enum_name.clone()),
        ArtValue::Function(_) => Type::Function(vec![], Box::new(Type::Unknown)),
        ArtValue::Builtin(_) => Type::Function(vec![], Box::new(Type::Unknown)),
        ArtValue::WeakRef(_) => Type::Unknown,
        ArtValue::UnownedRef(_) => Type::Unknown,
        ArtValue::HeapComposite(_) => Type::Unknown, // resolução ocorre em nível de interpretador; para inferência simplificada tratamos como Unknown
    }
}
