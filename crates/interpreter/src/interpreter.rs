use core::ast::{Expr, ArtValue, Program, Stmt, MatchPattern, Function, InterpolatedPart};
use diagnostics::{Diagnostic, DiagnosticKind, Span};
use core::environment::Environment;
use crate::type_registry::TypeRegistry;
use crate::values::{Result, RuntimeError};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Interpreter {
    environment: Rc<RefCell<Environment>>,
    type_registry: TypeRegistry,
    pub diagnostics: Vec<Diagnostic>,
    pub last_value: Option<ArtValue>,
}

impl Interpreter {
    pub fn new() -> Self {
        let global_env = Rc::new(RefCell::new(Environment::new(None)));

    global_env.borrow_mut().define("println", ArtValue::Builtin(core::ast::BuiltinFn::Println));

    Interpreter { environment: global_env, type_registry: TypeRegistry::new(), diagnostics: Vec::new(), last_value: None }
    }

    pub fn with_prelude() -> Self {
        let mut interp = Self::new();
        // Registrar enum Result simples (não genérica) com Ok, Err aceitando 1 valor
    use core::Token;
        let name = Token::dummy("Result");
        let variants = vec![
            (Token::dummy("Ok"), Some(vec!["T".to_string()])),
            (Token::dummy("Err"), Some(vec!["E".to_string()])),
        ];
        interp.type_registry.register_enum(name, variants);
    interp
    }

    pub fn interpret(&mut self, program: Program) -> Result<()> {
        self.last_value = None;
        for statement in program {
            if let Err(RuntimeError::Return(_)) = self.execute(statement) { break; }
        }
        Ok(())
    }
    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> { std::mem::take(&mut self.diagnostics) }

    fn execute(&mut self, stmt: Stmt) -> Result<()> {
        match stmt {
            Stmt::Expression(expr) => {
                let val = self.evaluate(expr)?;
                self.last_value = Some(val.clone());
                Ok(())
            }
            Stmt::Let { name, ty: _, initializer } => {
                let value = self.evaluate(initializer)?;
                self.environment.borrow_mut().define(&name.lexeme, value);
                Ok(())
            }
            Stmt::Block { statements } => self.execute_block(statements, Some(self.environment.clone())),
            Stmt::If { condition, then_branch, else_branch } => {
                let condition_value = self.evaluate(condition)?;
                if self.is_truthy(&condition_value) {
                    self.execute(*then_branch)
                } else if let Some(else_stmt) = else_branch {
                    self.execute(*else_stmt)
                } else {
                    Ok(())
                }
            }
            Stmt::StructDecl { name, fields } => {
                self.type_registry.register_struct(name, fields);
                Ok(())
            }
            Stmt::EnumDecl { name, variants } => {
                self.type_registry.register_enum(name, variants);
                Ok(())
            }
            Stmt::Match { expr, cases } => {
                let match_value = self.evaluate(expr)?;
                for (pattern, stmt) in cases {
                    if let Some(bindings) = self.pattern_matches(&pattern, &match_value) {
                        let previous_env = self.environment.clone();
                        let new_env =
                            Rc::new(RefCell::new(Environment::new(Some(previous_env.clone()))));
                        self.environment = new_env;
                        for (name, value) in bindings {
                            self.environment.borrow_mut().define(&name, value);
                        }
                        let result = self.execute(stmt);
                        self.environment = previous_env;
                        return result;
                    }
                }
                Ok(())
            }
            Stmt::Function { name, params, body, .. } => {
                let function = Function {
                    name: Some(name.lexeme.clone()),
                    params,
                    body,
                    closure: self.environment.clone(),
                };
                self.environment.borrow_mut().define(&name.lexeme, ArtValue::Function(Rc::new(function)));
                Ok(())
            }
            Stmt::Return { value } => {
                let return_value = match value {
                    Some(expr) => self.evaluate(expr)?,
                    None => ArtValue::none(),
                };
                Err(RuntimeError::Return(return_value))
            }
        }
    }

    fn pattern_matches(
        &mut self,
        pattern: &MatchPattern,
        value: &ArtValue,
    ) -> Option<Vec<(String, ArtValue)>> {
        match (pattern, value) {
            (MatchPattern::Literal(lit), _) if lit == value => Some(vec![]),
            (MatchPattern::Wildcard, _) => Some(vec![]),
            // Se o binding está dentro de EnumVariant, associe ao valor correto
            (MatchPattern::Binding(name), val) => {
                // Se val for EnumInstance com um valor, associe ao primeiro valor
                if let ArtValue::EnumInstance { values, .. } = val {
                    if values.len() == 1 {
                        Some(vec![(name.lexeme.clone(), values[0].clone())])
                    } else {
                        // Se não, associe ao próprio valor
                        Some(vec![(name.lexeme.clone(), val.clone())])
                    }
                } else {
                    Some(vec![(name.lexeme.clone(), val.clone())])
                }
            },
            (
                MatchPattern::EnumVariant { variant, params },
                ArtValue::EnumInstance { variant: v_name, values, .. },
            ) if &variant.lexeme == v_name => match params {
                Some(param_patterns) => {
                    if param_patterns.len() != values.len() {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("Arity mismatch in pattern: expected {} found {}", values.len(), param_patterns.len()),
                            Span::new(variant.start, variant.end, variant.line, variant.col)));
                        return None;
                    }
                    let mut all_bindings = Vec::new();
                    for (i, p) in param_patterns.iter().enumerate() {
                        if let Some(bindings) = self.pattern_matches(p, &values[i]) {
                            all_bindings.extend(bindings);
                        } else {
                            return None;
                        }
                    }
                    Some(all_bindings)
                }
                None => {
                    if values.is_empty() {
                        Some(vec![])
                    } else {
                        None
                    }
                }
            },
            _ => None,
        }
    }

    fn execute_block(
        &mut self,
        statements: Vec<Stmt>,
        enclosing: Option<Rc<RefCell<Environment>>>,
    ) -> Result<()> {
        let previous = self.environment.clone();
        self.environment = Rc::new(RefCell::new(Environment::new(enclosing)));

        for statement in statements {
            if let Err(e) = self.execute(statement) {
                self.environment = previous;
                return Err(e);
            }
        }

        self.environment = previous;
        Ok(())
    }

    fn evaluate(&mut self, expr: Expr) -> Result<ArtValue> {
        match expr {
        Expr::InterpolatedString(parts) => {
                // Heurística: soma tamanhos literais para reservar.
                let cap: usize = parts.iter().map(|p| match p { InterpolatedPart::Literal(s) => s.len(), _ => 4 }).sum();
                let mut result = String::with_capacity(cap);
                for part in parts {
                    match part {
                        InterpolatedPart::Literal(s) => result.push_str(&s),
                        InterpolatedPart::Expr(e) => {
                            let val = self.evaluate(*e)?;
                            let seg = val.to_string();
                            result.push_str(&seg);
                        }
                    }
                }
                Ok(ArtValue::String(std::sync::Arc::from(result)))
            }
            Expr::Literal(value) => Ok(value),
            Expr::Grouping { expression } => self.evaluate(*expression),
            Expr::Variable { name } => {
                let name_str = name.lexeme.clone();
                match self.environment.borrow().get(&name_str) {
                    Some(v) => Ok(v.clone()),
                    None => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("Undefined variable '{}'.", name_str),
                            Span::new(name.start, name.end, name.line, name.col)));
                        Ok(ArtValue::none())
                    }
                }
            }
            Expr::Unary { operator, right } => {
                let right_val = self.evaluate(*right)?;
                match operator.token_type {
                    core::TokenType::Minus => match right_val {
                        ArtValue::Int(n) => Ok(ArtValue::Int(-n)),
                        ArtValue::Float(f) => Ok(ArtValue::Float(-f)),
                        _ => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Type mismatch.".to_string(),
                                Span::new(operator.start, operator.end, operator.line, operator.col)));
                            Ok(ArtValue::none())
                        }
                    },
                    core::TokenType::Bang => Ok(ArtValue::Bool(!self.is_truthy(&right_val))),
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "Invalid operator.".to_string(),
                            Span::new(operator.start, operator.end, operator.line, operator.col)));
                        Ok(ArtValue::none())
                    }
                }
            }
            Expr::Logical { left, operator, right } => {
                let left_val = self.evaluate(*left)?;
                if operator.token_type == core::TokenType::Or {
                    if self.is_truthy(&left_val) { return Ok(left_val); }
                } else if !self.is_truthy(&left_val) { return Ok(left_val); }
                self.evaluate(*right)
            }
            Expr::Binary { left, operator, right } => {
                let left_val = self.evaluate(*left)?;
                let right_val = self.evaluate(*right)?;
                match operator.token_type {
                    core::TokenType::Plus => match (&left_val, &right_val) {
                        (ArtValue::Int(l), ArtValue::Int(r)) => Ok(ArtValue::Int(l + r)),
                        (ArtValue::Float(l), ArtValue::Float(r)) => Ok(ArtValue::Float(l + r)),
                        (ArtValue::String(l), ArtValue::String(r)) => {
                            Ok(ArtValue::String(std::sync::Arc::from(format!("{}{}", l, r))))
                        }
                        (ArtValue::Int(l), ArtValue::Float(r)) => Ok(ArtValue::Float(*l as f64 + r)),
                        (ArtValue::Float(l), ArtValue::Int(r)) => Ok(ArtValue::Float(l + *r as f64)),
                        _ => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Type mismatch.".to_string(),
                                Span::new(operator.start, operator.end, operator.line, operator.col)));
                            Ok(ArtValue::none())
                        }
                    },
                    core::TokenType::Minus => self.binary_num_op(left_val, right_val, |a, b| a - b),
                    core::TokenType::Star => self.binary_num_op(left_val, right_val, |a, b| a * b),
                    core::TokenType::Slash => {
                        let div_by_zero = matches!(right_val, ArtValue::Int(0))
                            || matches!(right_val, ArtValue::Float(f) if f == 0.0);
                        if div_by_zero {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Division by zero".to_string(),
                                Span::new(operator.start, operator.end, operator.line, operator.col)));
                            Ok(ArtValue::none())
                        } else {
                            self.binary_num_op(left_val, right_val, |a, b| a / b)
                        }
                    },
                    core::TokenType::Greater => self.binary_cmp_op(left_val, right_val, |a, b| a > b),
                    core::TokenType::GreaterEqual => self.binary_cmp_op(left_val, right_val, |a, b| a >= b),
                    core::TokenType::Less => self.binary_cmp_op(left_val, right_val, |a, b| a < b),
                    core::TokenType::LessEqual => self.binary_cmp_op(left_val, right_val, |a, b| a <= b),
                    core::TokenType::BangEqual => Ok(ArtValue::Bool(!self.is_equal(&left_val, &right_val))),
                    core::TokenType::EqualEqual => Ok(ArtValue::Bool(self.is_equal(&left_val, &right_val))),
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "Invalid operator.".to_string(),
                            Span::new(operator.start, operator.end, operator.line, operator.col)));
                        Ok(ArtValue::none())
                    }
                }
            }
            Expr::Call { callee, arguments } => self.handle_call(*callee, arguments),
            Expr::StructInit { name, fields } => {
                let struct_def = match self.type_registry.get_struct(&name.lexeme) {
                    Some(def) => def.clone(),
                    None => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("Undefined struct '{}'.", name.lexeme),
                            Span::new(name.start, name.end, name.line, name.col)));
                        return Ok(ArtValue::none().clone());
                    }
                };
                let mut field_values = HashMap::new();
                for (field_name, field_expr) in fields {
                    let value = self.evaluate(field_expr)?;
                    field_values.insert(field_name.lexeme, value);
                }
                for (field_name, _field_type) in &struct_def.fields {
                    if !field_values.contains_key(field_name) {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("Missing field '{}'.", field_name),
                            Span::new(name.start, name.end, name.line, name.col)));
                        return Ok(ArtValue::none().clone());
                    }
                }
                Ok(ArtValue::StructInstance {
                    struct_name: name.lexeme,
                    fields: field_values,
                })
            }
            Expr::EnumInit { name, variant, values } => {
                let enum_name = match name {
                    Some(n) => n.lexeme,
                    None => {
                        // Inferência: procurar enum que contenha a variant de forma única
                        let mut candidate: Option<String> = None;
                        for (ename, edef) in self.type_registry.enums.iter() {
                            if edef.variants.iter().any(|(v, _)| v == &variant.lexeme) {
                                if candidate.is_some() && candidate.as_ref() != Some(ename) {
                                    self.diagnostics.push(Diagnostic::new(
                                        DiagnosticKind::Runtime,
                                        "Ambiguous enum variant shorthand.".to_string(),
                                        Span::new(variant.start, variant.end, variant.line, variant.col)
                                    ));
                                    return Ok(ArtValue::none());
                                }
                                candidate = Some(ename.clone());
                            }
                        }
                        match candidate {
                            Some(c) => c,
                            None => {
                                self.diagnostics.push(Diagnostic::new(
                                    DiagnosticKind::Runtime,
                                    "Cannot infer enum type for shorthand initialization.".to_string(),
                                    Span::new(variant.start, variant.end, variant.line, variant.col)));
                                return Ok(ArtValue::none());
                            }
                        }
                    }
                };
                let enum_def = match self.type_registry.get_enum(&enum_name) {
                    Some(def) => def.clone(),
                    None => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            format!("Undefined enum '{}'.", enum_name),
                            Span::new(variant.start, variant.end, variant.line, variant.col)));
                        return Ok(ArtValue::none());
                    }
                };
                let variant_def = match enum_def
                    .variants
                    .iter()
                    .find(|(v_name, _)| v_name == &variant.lexeme) {
                        Some(v) => v,
                        None => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                format!("Invalid enum variant '{}'.", variant.lexeme),
                                Span::new(variant.start, variant.end, variant.line, variant.col)));
                            return Ok(ArtValue::none());
                        }
                    };
                let mut evaluated_values = Vec::new();
                for value_expr in values {
                    evaluated_values.push(self.evaluate(value_expr)?);
                }
                match &variant_def.1 {
                    Some(expected_params) => {
                        if evaluated_values.len() != expected_params.len() {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Wrong number of arguments.".to_string(),
                                Span::new(variant.start, variant.end, variant.line, variant.col)));
                            return Ok(ArtValue::none());
                        }
                    }
                    None => {
                        if !evaluated_values.is_empty() {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Wrong number of arguments.".to_string(),
                                Span::new(variant.start, variant.end, variant.line, variant.col)));
                            return Ok(ArtValue::none());
                        }
                    }
                }
                Ok(ArtValue::EnumInstance {
                    enum_name,
                    variant: variant.lexeme,
                    values: evaluated_values,
                })
            }
            Expr::FieldAccess { object, field } => {
                let obj_value = self.evaluate(*object)?;
                match obj_value {
                    ArtValue::Array(arr) => match field.lexeme.as_str() {
                        "sum" => {
                            let mut sum = 0;
                            for val in arr.iter() {
                                if let ArtValue::Int(n) = val {
                                    sum += n;
                                } else {
                                    self.diagnostics.push(Diagnostic::new(
                                        DiagnosticKind::Runtime,
                                        "Type mismatch in sum (expected Int)".to_string(),
                                        Span::new(field.start, field.end, field.line, field.col)));
                                    return Ok(ArtValue::none());
                                }
                            }
                            Ok(ArtValue::Int(sum))
                        }
                        "count" => Ok(ArtValue::Int(arr.len() as i64)),
                        _ => {
                            self.diagnostics.push(Diagnostic::new(
                                DiagnosticKind::Runtime,
                                "Type mismatch.".to_string(),
                                Span::new(field.start, field.end, field.line, field.col)));
                            Ok(ArtValue::none())
                        }
                    },
                            ArtValue::StructInstance { fields, .. } => {
                                match fields.get(&field.lexeme) {
                                    Some(v) => Ok(v.clone()),
                                    None => {
                                        self.diagnostics.push(Diagnostic::new(
                                            DiagnosticKind::Runtime,
                                            format!("Missing field '{}'.", field.lexeme),
                                            Span::new(field.start, field.end, field.line, field.col)));
                                        Ok(ArtValue::none())
                                    }
                                }
                            }
                    _ => {
                        self.diagnostics.push(Diagnostic::new(
                            DiagnosticKind::Runtime,
                            "Type mismatch.".to_string(),
                            Span::new(field.start, field.end, field.line, field.col)));
                        Ok(ArtValue::none())
                    }
                }
            }
            Expr::Try(inner) => {
                let result_val = self.evaluate(*inner)?;
                match result_val {
                    ArtValue::EnumInstance { enum_name, variant, mut values } if enum_name == "Result" => {
                        if variant == "Ok" {
                            Ok(values.pop().unwrap_or(ArtValue::none()))
                        } else {
                            Err(RuntimeError::Return(values.pop().unwrap_or(ArtValue::none())))
                        }
                    },
                    other => Ok(other)
                }
            },
            Expr::Cast { object, .. } => self.evaluate(*object),
            Expr::Array(elements) => {
                let mut evaluated_elements = Vec::new();
                for element in elements {
                    evaluated_elements.push(self.evaluate(element)?);
                }
                Ok(ArtValue::Array(evaluated_elements))
            }
        }
    }

    fn handle_call(&mut self, callee: Expr, arguments: Vec<Expr>) -> Result<ArtValue> {
        let original_expr = callee.clone();
        let value = self.evaluate(callee)?;
        match value {
            ArtValue::Function(func) => self.call_function(func, arguments),
            ArtValue::Builtin(b) => self.call_builtin(b, arguments),
            ArtValue::EnumInstance { enum_name, variant, values } if values.is_empty() => {
                self.construct_enum_variant(enum_name, variant, arguments)
            }
            other => self.call_fallback(original_expr, other, &arguments),
        }
    }

    fn call_function(&mut self, func: Rc<Function>, arguments: Vec<Expr>) -> Result<ArtValue> {
    let argc = arguments.len();
        if func.params.len() != argc {
            self.diagnostics.push(Diagnostic::new(
                DiagnosticKind::Runtime,
                "Wrong number of arguments.".to_string(),
                Span::new(0,0,0,0)));
            return Ok(ArtValue::none());
        }
        // Avalia argumentos uma vez
        let mut evaluated_args = Vec::with_capacity(argc);
        for arg in arguments { evaluated_args.push(self.evaluate(arg)?); }
        let previous_env = self.environment.clone();
        self.environment = Rc::new(RefCell::new(Environment::new(Some(func.closure.clone()))));
        // Inserir valores movendo (sem clone) consumindo o vetor
        for (param, value) in func.params.iter().zip(evaluated_args.into_iter()) {
            self.environment.borrow_mut().define(&param.name.lexeme, value);
        }
        let result = self.execute(Rc::as_ref(&func.body).clone());
        self.environment = previous_env;
    match result { Ok(()) => Ok(ArtValue::none()), Err(RuntimeError::Return(val)) => Ok(val) }
    }

    fn call_builtin(&mut self, b: core::ast::BuiltinFn, arguments: Vec<Expr>) -> Result<ArtValue> {
        match b {
            core::ast::BuiltinFn::Println => {
                if let Some(first) = arguments.into_iter().next() {
                    let val = self.evaluate(first)?; println!("{}", val);
                } else { println!(); }
                Ok(ArtValue::none())
            }
        }
    }

    fn construct_enum_variant(&mut self, enum_name: String, variant: String, arguments: Vec<Expr>) -> Result<ArtValue> {
        let mut evaluated_args = Vec::new();
        for arg in arguments { evaluated_args.push(self.evaluate(arg)?); }
        Ok(ArtValue::EnumInstance { enum_name, variant, values: evaluated_args })
    }

    fn call_fallback(&mut self, original_expr: Expr, value: ArtValue, arguments: &[Expr]) -> Result<ArtValue> {
        if arguments.is_empty() && let Expr::FieldAccess { .. } = original_expr { return Ok(value); }
        self.diagnostics.push(Diagnostic::new(
            DiagnosticKind::Runtime,
            format!("'{}' is not a function.", value),
            Span::new(0,0,0,0)));
    Ok(ArtValue::none())
    }

    fn is_truthy(&self, value: &ArtValue) -> bool {
        match value {
            ArtValue::Bool(b) => *b,
            ArtValue::Optional(opt) => opt.is_some(),
            ArtValue::Int(n) => *n != 0,
            ArtValue::Float(f) => *f != 0.0,
            ArtValue::String(s) => !s.is_empty(),
            ArtValue::Array(arr) => !arr.is_empty(),
            _ => true,
        }
    }

    fn is_equal(&self, a: &ArtValue, b: &ArtValue) -> bool {
        a == b
    }

    fn binary_num_op<F>(&self, left: ArtValue, right: ArtValue, op: F) -> Result<ArtValue>
    where
        F: Fn(f64, f64) -> f64,
    {
        match (left, right) {
            (ArtValue::Int(l), ArtValue::Int(r)) => {
                Ok(ArtValue::Int(op(l as f64, r as f64) as i64))
            }
            (ArtValue::Float(l), ArtValue::Float(r)) => Ok(ArtValue::Float(op(l, r))),
            (ArtValue::Int(l), ArtValue::Float(r)) => Ok(ArtValue::Float(op(l as f64, r))),
            (ArtValue::Float(l), ArtValue::Int(r)) => Ok(ArtValue::Float(op(l, r as f64))),
            _ => {
                // Type mismatch in numeric op
                // Without operator token context here; caller handles some cases explicitly.
                // We fallback to neutral Optional(None).
                // (Future: enrich with span info by passing operator token.)
                Ok(ArtValue::none())
            }
        }
    }

    fn binary_cmp_op<F>(&self, left: ArtValue, right: ArtValue, op: F) -> Result<ArtValue>
    where
        F: Fn(f64, f64) -> bool,
    {
        match (left, right) {
            (ArtValue::Int(l), ArtValue::Int(r)) => Ok(ArtValue::Bool(op(l as f64, r as f64))),
            (ArtValue::Float(l), ArtValue::Float(r)) => Ok(ArtValue::Bool(op(l, r))),
            (ArtValue::Int(l), ArtValue::Float(r)) => Ok(ArtValue::Bool(op(l as f64, r))),
            (ArtValue::Float(l), ArtValue::Int(r)) => Ok(ArtValue::Bool(op(l, r as f64))),
            _ => Ok(ArtValue::none()),
        }
    }
}

// (Removed unused infer_type helper; now handled in dedicated type_infer module)