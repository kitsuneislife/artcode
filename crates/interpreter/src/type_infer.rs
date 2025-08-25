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
    // track variable bindings per lexical scope so we can restore previous
    // types when a scope is popped (shadowing must not permanently clobber outer vars)
    var_bindings: Vec<Vec<(String, Option<Type>)>>,
    // store top-level function declarations for simple callsite simulation
    functions: HashMap<String, (Vec<String>, std::rc::Rc<Stmt>)>,
}

impl<'a> TypeInfer<'a> {
    pub fn new(tenv: &'a mut TypeEnv) -> Self {
        Self {
            diags: Vec::new(),
            tenv,
            enums: HashMap::new(),
            scopes: vec![HashSet::new()],
            var_bindings: vec![Vec::new()],
            functions: HashMap::new(),
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashSet::new());
    self.var_bindings.push(Vec::new());
    }

    fn pop_scope(&mut self) {
        // restore variable bindings for the scope being popped
        if let Some(bindings) = self.var_bindings.pop() {
            for (name, prev) in bindings.into_iter().rev() {
                match prev {
                    Some(t) => {
                        self.tenv.set_var(&name, t);
                    }
                    None => {
                        self.tenv.vars.remove(&name);
                    }
                }
            }
        }
        self.scopes.pop();
    }

    fn declare_var(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string());
        }
    }

    fn record_var_binding(&mut self, name: &str) {
        let prev = self.tenv.get_var(name).cloned();
        if let Some(bindings) = self.var_bindings.last_mut() {
            bindings.push((name.to_string(), prev));
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
            Stmt::Let { name, initializer, .. } => {
                // If initializer is a simple variable reference, propagate its known type
                let t = match initializer {
                    Expr::Variable { name: src } => {
                        if let Some(ty) = self.tenv.get_var(&src.lexeme).cloned() {
                            ty
                        } else {
                            self.infer_expr(initializer)
                        }
                    }
                    _ => self.infer_expr(initializer),
                };
                // record previous binding so we can restore on scope pop
                self.record_var_binding(&name.lexeme);
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
            Stmt::If { condition, then_branch, else_branch } => {
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
            | Stmt::Return { .. }
            | Stmt::Match { .. }
            | Stmt::Import { .. } => {}
            Stmt::Function { name, params, return_type: _, body, method_owner: _ } => {
                // record simple top-level function for callsite simulation: store param names and body
                let param_names: Vec<String> = params.iter().map(|p| p.name.lexeme.clone()).collect();
                self.functions.insert(name.lexeme.clone(), (param_names, body.clone()));
            }
            Stmt::Performant { statements } => {
                self.check_performant_block(statements);
            }
            Stmt::SpawnActor { body } => {
                // Conservative check: ensure the actor body does not capture outer variables
                // that might be non-send-safe. We scan statements for uses of outer vars and
                // emit a diagnostic if found.
                let outer_vars = self.visible_vars();
                for s in body {
                    if let Stmt::Expression(e) = s {
                        let captures = self.expr_uses_outer_vars(e, &self.visible_vars(), &outer_vars);
                        if !captures.is_empty() {
                            for cap in captures {
                                self.diags.push(Diagnostic::new(
                                    DiagnosticKind::Type,
                                    format!("Spawned actor body references outer variable '{}' which may not be Send-safe", cap),
                                    Span::new(0,0,0,0),
                                ));
                            }
                        }
                    }
                }
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
            Expr::SpawnActor { body: _ } => {
                // For spawn actor expressions, conservatively treat body as not capturing outer vars
                // (body is a sequence of statements; deeper analysis can be added later)
            }
            Literal(_) => {}
        }
        // Deduplicate
        found.sort();
        found.dedup();
        found
    }

    // Conservative heuristic: decide whether an expression is safe to send/share across actor
    // boundaries. Allowed: Int, Float, Bool, String. Arrays/Structs/Enums are send-safe if
    // all their components are send-safe. HeapComposite is conservatively NOT send-safe.
    fn is_send_safe_expr(&self, expr: &Expr) -> bool {
        use Expr::*;
        match expr {
            Literal(v) => match v {
                ArtValue::Int(_) => true,
                ArtValue::Float(_) => true,
                ArtValue::Bool(_) => true,
                ArtValue::Atomic(_) => true,
                ArtValue::Mutex(_) => true,
                ArtValue::String(_) => true,
                ArtValue::Optional(boxed) => match &**boxed {
                    Some(inner) => matches!(inner, ArtValue::Int(_)),
                    None => true,
                },
                _ => false,
            },
            Array(elements) => elements.iter().all(|e| self.is_send_safe_expr(e)),
            StructInit { fields, .. } => fields.iter().all(|(_n, e)| self.is_send_safe_expr(e)),
            EnumInit { values, .. } => values.iter().all(|e| self.is_send_safe_expr(e)),
            Grouping { expression } => self.is_send_safe_expr(expression),
            Unary { right, .. } => self.is_send_safe_expr(right),
            Binary { left, right, .. } => self.is_send_safe_expr(left) && self.is_send_safe_expr(right),
            Logical { left, right, .. } => self.is_send_safe_expr(left) && self.is_send_safe_expr(right),
            Call { .. } => false,
            Variable { name } => {
                // If the variable has a known type in the TypeEnv, use the type-based
                // send-safety check. Otherwise conservatively assume not send-safe.
                if let Some(t) = self.tenv.get_var(&name.lexeme) {
                    self.is_send_safe_type(t)
                } else {
                    false
                }
            }
            FieldAccess { object, .. } => self.is_send_safe_expr(object),
            InterpolatedString(parts) => parts.iter().all(|p| match p {
                InterpolatedPart::Literal(_) => true,
                InterpolatedPart::Expr { expr, .. } => self.is_send_safe_expr(expr),
            }),
            Cast { object, .. } => self.is_send_safe_expr(object),
            Try(_) | Weak(_) | Unowned(_) | WeakUpgrade(_) | UnownedAccess(_) | SpawnActor { .. } => false,
        }
    }

    fn is_send_safe_type(&self, t: &Type) -> bool {
        match t {
            Type::Int | Type::Float | Type::Bool | Type::String => true,
            Type::Array(inner) => self.is_send_safe_type(inner),
            Type::EnumInstance(_name, types) => types.iter().all(|tt| self.is_send_safe_type(tt)),
            // Struct without field info is conservative: not send-safe unless proven otherwise
            _ => false,
        }
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
                // Infer callee first
                self.infer_expr(callee);
                // If callee is a plain variable named `actor_send` or `make_envelope`,
                // apply a conservative send-safe check on payload argument(s).
                if let Expr::Variable { name } = &**callee {
                    if name.lexeme == "actor_send" {
                        // actor_send(actor, value [, priority])
                        if arguments.len() >= 2 {
                            let payload_expr = &arguments[1];
                            if !self.is_send_safe_expr(payload_expr) {
                                self.diags.push(Diagnostic::new(
                                    DiagnosticKind::Type,
                                    "actor_send: payload expression is not send-safe".to_string(),
                                    Span::new(0,0,0,0),
                                ));
                            }
                        }
                    } else if name.lexeme == "make_envelope" {
                        // make_envelope(payload [, priority])
                        if arguments.len() >= 1 {
                            let payload_expr = &arguments[0];
                            if !self.is_send_safe_expr(payload_expr) {
                                self.diags.push(Diagnostic::new(
                                    DiagnosticKind::Type,
                                    "make_envelope: payload expression is not send-safe".to_string(),
                                    Span::new(0,0,0,0),
                                ));
                            }
                        }
                    }
                }
                // Simple callsite propagation: if the callee is a known top-level function,
                // bind parameter names to argument types (for literal or known-variable args)
                if let Expr::Variable { name } = &**callee {
                    if let Some(entry) = self.functions.get(&name.lexeme).cloned() {
                        let (param_names, body) = entry;
                        // create a temporary scope for params
                        self.push_scope();
                        for (i, p) in param_names.iter().enumerate() {
                            if i < arguments.len() {
                                let arg = &arguments[i];
                                let ty = self.infer_expr(arg);
                                self.record_var_binding(p);
                                self.tenv.set_var(p, ty);
                            }
                        }
                        // Optionally infer the body to propagate types inside function (cheap simulation)
                        // We don't attempt full signature/return inference here.
                        self.visit_stmt(&*body);
                        self.pop_scope();
                    }
                }
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
            Expr::SpawnActor { body } => {
                // spawn actor retorna um handle (Actor). Para inferência simplificada,
                // tratamos como Unknown (não tentamos modelar Actor no sistema de tipos agora).
                let _ = body; // body not deeply type-checked here
                Type::Unknown
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
        ArtValue::Atomic(_) => Type::Unknown,
        ArtValue::Mutex(_) => Type::Unknown,
    ArtValue::Actor(_) => Type::Unknown,
        ArtValue::HeapComposite(_) => Type::Unknown, // resolução ocorre em nível de interpretador; para inferência simplificada tratamos como Unknown
    }
}
