use core::ast::{Expr, ArtValue, Program, Stmt, MatchPattern};
use core::TokenType;
use crate::environment::Environment;
use crate::type_registry::TypeRegistry;
use crate::values::{Result, RuntimeError};
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;

pub struct Interpreter {
    environment: Rc<RefCell<Environment>>,
    type_registry: TypeRegistry,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            environment: Rc::new(RefCell::new(Environment::new(None))),
            type_registry: TypeRegistry::new(),
        }
    }

    pub fn interpret(&mut self, program: Program) -> Result<()> {
        for statement in program {
            match self.execute(statement) {
                Ok(()) => {}
                Err(RuntimeError::Return(_)) => {
                    // In a script context, a return at the top level just stops execution.
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn execute(&mut self, stmt: Stmt) -> Result<()> {
        match stmt {
            Stmt::Expression(expr) => {
                self.evaluate(expr)?;
                Ok(())
            }
            Stmt::Let { name, ty: _, initializer } => {
                let value = self.evaluate(initializer)?;
                self.environment.borrow_mut().define(name.lexeme, value);
                Ok(())
            }
            Stmt::Block { statements } => {
                self.execute_block(statements, Some(self.environment.clone()))
            }
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
                let mut matched = false;
                for (pattern, stmt) in cases {
                    if let Some(bindings) = self.pattern_matches(&pattern, &match_value) {
                        let previous_env = self.environment.clone();
                        let new_env = Rc::new(RefCell::new(Environment::new(Some(previous_env.clone()))));
                        self.environment = new_env;
                        for (name, value) in bindings {
                            self.environment.borrow_mut().define(name, value);
                        }
                        let result = self.execute(stmt);
                        self.environment = previous_env;
                        matched = true;
                        return result;
                    }
                }
                if !matched {
                    // In a real implementation, you might want a runtime error
                    // if a match is not exhaustive. For now, we do nothing.
                }
                Ok(())
            }
            Stmt::Function { name, params: _, return_type: _, body: _ } => {
                self.environment.borrow_mut().define(name.lexeme, ArtValue::Bool(false));
                Ok(())
            }
            Stmt::Return { value } => {
                let return_value = match value {
                    Some(expr) => self.evaluate(expr)?,
                    None => ArtValue::Optional(Box::new(None)),
                };
                Err(RuntimeError::Return(return_value))
            }
        }
    }

    fn pattern_matches(&self, pattern: &MatchPattern, value: &ArtValue) -> Option<Vec<(String, ArtValue)>> {
        match (pattern, value) {
            (MatchPattern::Literal(lit), _) if lit == value => Some(vec![]),
            (MatchPattern::Wildcard, _) => Some(vec![]),
            (MatchPattern::Binding(name), _) => Some(vec![(name.lexeme.clone(), value.clone())]),
            (MatchPattern::EnumVariant { variant, params }, ArtValue::EnumInstance { variant: v_name, values, .. }) if &variant.lexeme == v_name => {
                match params {
                    Some(param_patterns) => {
                        if param_patterns.len() != values.len() {
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
                    },
                    None => {
                        if values.is_empty() { Some(vec![]) } else { None }
                    }
                }
            }
            _ => None,
        }
    }

    fn execute_block(&mut self, statements: Vec<Stmt>, enclosing: Option<Rc<RefCell<Environment>>>) -> Result<()> {
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
            Expr::Literal(value) => Ok(value),
            Expr::Grouping { expression } => self.evaluate(*expression),
            Expr::Variable { name } => {
                let name_str = name.lexeme.clone();
                self.environment.borrow().get(&name_str)
                    .ok_or(RuntimeError::UndefinedVariable(name_str))
            }
            Expr::Unary { operator, right } => {
                let right_val = self.evaluate(*right)?;
                match operator.token_type {
                    TokenType::Minus => match right_val {
                        ArtValue::Int(n) => Ok(ArtValue::Int(-n)),
                        ArtValue::Float(f) => Ok(ArtValue::Float(-f)),
                        _ => Err(RuntimeError::TypeMismatch),
                    },
                    TokenType::Bang => Ok(ArtValue::Bool(!self.is_truthy(&right_val))),
                    _ => Err(RuntimeError::InvalidOperator),
                }
            }
            Expr::Logical { left, operator, right } => {
                let left_val = self.evaluate(*left)?;
                if operator.token_type == TokenType::Or {
                    if self.is_truthy(&left_val) { return Ok(left_val); }
                } else {
                    if !self.is_truthy(&left_val) { return Ok(left_val); }
                }
                self.evaluate(*right)
            }
            Expr::Binary { left, operator, right } => {
                let left_val = self.evaluate(*left)?;
                let right_val = self.evaluate(*right)?;
                match operator.token_type {
                    TokenType::Plus => match (&left_val, &right_val) {
                        (ArtValue::Int(l), ArtValue::Int(r)) => Ok(ArtValue::Int(l + r)),
                        (ArtValue::Float(l), ArtValue::Float(r)) => Ok(ArtValue::Float(l + r)),
                        (ArtValue::String(l), ArtValue::String(r)) => Ok(ArtValue::String(format!("{}{}", l, r))),
                        (ArtValue::Int(l), ArtValue::Float(r)) => Ok(ArtValue::Float(*l as f64 + r)),
                        (ArtValue::Float(l), ArtValue::Int(r)) => Ok(ArtValue::Float(l + *r as f64)),
                        _ => Err(RuntimeError::TypeMismatch),
                    },
                    TokenType::Minus => self.binary_num_op(left_val, right_val, |a, b| a - b),
                    TokenType::Star => self.binary_num_op(left_val, right_val, |a, b| a * b),
                    TokenType::Slash => match (&left_val, &right_val) {
                        (_, ArtValue::Int(0)) => Err(RuntimeError::DivisionByZero),
                        (_, ArtValue::Float(f)) if *f == 0.0 => Err(RuntimeError::DivisionByZero),
                        _ => self.binary_num_op(left_val, right_val, |a, b| a / b),
                    },
                    TokenType::Greater => self.binary_cmp_op(left_val, right_val, |a, b| a > b),
                    TokenType::GreaterEqual => self.binary_cmp_op(left_val, right_val, |a, b| a >= b),
                    TokenType::Less => self.binary_cmp_op(left_val, right_val, |a, b| a < b),
                    TokenType::LessEqual => self.binary_cmp_op(left_val, right_val, |a, b| a <= b),
                    TokenType::BangEqual => Ok(ArtValue::Bool(!self.is_equal(&left_val, &right_val))),
                    TokenType::EqualEqual => Ok(ArtValue::Bool(self.is_equal(&left_val, &right_val))),
                    _ => Err(RuntimeError::InvalidOperator),
                }
            }
            Expr::Call { callee, arguments } => self.handle_call(*callee, arguments),
            Expr::StructInit { name, fields } => {
                let struct_def = self.type_registry.get_struct(&name.lexeme)
                    .ok_or_else(|| RuntimeError::Other(format!("Undefined struct '{}'.", name.lexeme)))?
                    .clone();
                let mut field_values = HashMap::new();
                for (field_name, field_expr) in fields {
                    let value = self.evaluate(field_expr)?;
                    field_values.insert(field_name.lexeme, value);
                }
                for (field_name, _field_type) in &struct_def.fields {
                    if !field_values.contains_key(field_name) {
                        return Err(RuntimeError::MissingField(field_name.clone()));
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
                    None => return Err(RuntimeError::Other("Cannot infer enum type for shorthand initialization.".to_string())),
                };
                let enum_def = self.type_registry.get_enum(&enum_name)
                    .ok_or_else(|| RuntimeError::Other(format!("Undefined enum '{}'.", enum_name)))?
                    .clone();
                let variant_def = enum_def.variants.iter()
                    .find(|(v_name, _)| v_name == &variant.lexeme)
                    .ok_or_else(|| RuntimeError::InvalidEnumVariant(variant.lexeme.clone()))?;
                let mut evaluated_values = Vec::new();
                for value_expr in values {
                    evaluated_values.push(self.evaluate(value_expr)?);
                }
                match &variant_def.1 {
                    Some(expected_params) => {
                        if evaluated_values.len() != expected_params.len() {
                            return Err(RuntimeError::WrongNumberOfArguments);
                        }
                    }
                    None => {
                        if !evaluated_values.is_empty() {
                            return Err(RuntimeError::WrongNumberOfArguments);
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
                    ArtValue::Array(arr) => {
                        match field.lexeme.as_str() {
                            "sum" => {
                                let mut sum = 0;
                                for val in arr.iter() {
                                    if let ArtValue::Int(n) = val {
                                        sum += n;
                                    } else {
                                        return Err(RuntimeError::TypeMismatch);
                                    }
                                }
                                Ok(ArtValue::Int(sum))
                            }
                            "count" => Ok(ArtValue::Int(arr.len() as i64)),
                            _ => Err(RuntimeError::TypeMismatch),
                        }
                    }
                    ArtValue::StructInstance { fields, .. } => {
                        fields.get(&field.lexeme)
                            .cloned()
                            .ok_or_else(|| RuntimeError::MissingField(field.lexeme.clone()))
                    }
                    _ => Err(RuntimeError::TypeMismatch),
                }
            }
            Expr::Try(inner) => {
                match self.evaluate(*inner) {
                    Ok(ArtValue::EnumInstance { enum_name, variant, mut values }) if enum_name == "Result" && variant == "Err" => {
                        Err(RuntimeError::Return(values.pop().unwrap_or(ArtValue::Optional(Box::new(None)))))
                    },
                    other => other,
                }
            }
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
        let callee_val = self.evaluate(callee)?;
        let mut args = Vec::new();
        for arg in arguments {
            args.push(self.evaluate(arg)?);
        }

        match callee_val {
            ArtValue::String(s) if s == "println" => {
                if !args.is_empty() {
                    println!("{}", args[0]);
                } else {
                    println!();
                }
                Ok(ArtValue::Optional(Box::new(None)))
            }
            ArtValue::EnumInstance { enum_name, variant, values } if values.is_empty() => {
                Ok(ArtValue::EnumInstance {
                    enum_name,
                    variant,
                    values: args,
                })
            }
            _ => Err(RuntimeError::Other(format!("'{}' is not a function.", callee_val))),
        }
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
    where F: Fn(f64, f64) -> f64, {
        match (left, right) {
            (ArtValue::Int(l), ArtValue::Int(r)) => Ok(ArtValue::Int(op(l as f64, r as f64) as i64)),
            (ArtValue::Float(l), ArtValue::Float(r)) => Ok(ArtValue::Float(op(l, r))),
            (ArtValue::Int(l), ArtValue::Float(r)) => Ok(ArtValue::Float(op(l as f64, r))),
            (ArtValue::Float(l), ArtValue::Int(r)) => Ok(ArtValue::Float(op(l, r as f64))),
            _ => Err(RuntimeError::TypeMismatch),
        }
    }

    fn binary_cmp_op<F>(&self, left: ArtValue, right: ArtValue, op: F) -> Result<ArtValue>
    where F: Fn(f64, f64) -> bool, {
        match (left, right) {
            (ArtValue::Int(l), ArtValue::Int(r)) => Ok(ArtValue::Bool(op(l as f64, r as f64))),
            (ArtValue::Float(l), ArtValue::Float(r)) => Ok(ArtValue::Bool(op(l, r))),
            (ArtValue::Int(l), ArtValue::Float(r)) => Ok(ArtValue::Bool(op(l as f64, r))),
            (ArtValue::Float(l), ArtValue::Int(r)) => Ok(ArtValue::Bool(op(l, r as f64))),
            _ => Err(RuntimeError::TypeMismatch),
        }
    }
}