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
            self.execute(statement)?;
        }
        Ok(())
    }

    fn execute(&mut self, stmt: Stmt) -> Result<()> {
        match stmt {
            Stmt::Expression(expr) => {
                self.evaluate(expr)?;
            }
            Stmt::Let { name, initializer } => {
                let value = self.evaluate(initializer)?;
                self.environment.borrow_mut().define(name.lexeme, value);
            }
            Stmt::Block { statements } => {
                self.execute_block(statements, Some(self.environment.clone()))?;
            }
            Stmt::If { condition, then_branch, else_branch } => {
                let condition_value = self.evaluate(condition)?;
                if self.is_truthy(&condition_value) {
                    self.execute(*then_branch)?;
                } else if let Some(else_stmt) = else_branch {
                    self.execute(*else_stmt)?;
                }
            }
            Stmt::StructDecl { name, fields } => {
                self.type_registry.register_struct(name, fields);
            }
            Stmt::EnumDecl { name, variants } => {
                self.type_registry.register_enum(name, variants);
            }
            Stmt::Match { expr, cases } => {
                let match_value = self.evaluate(expr)?;
                for (pattern, stmt) in cases {
                    if let Some(bindings) = self.pattern_matches(&pattern, &match_value) {
                        let previous_env = self.environment.clone();
                        let new_env = Rc::new(RefCell::new(Environment::new(Some(previous_env.clone()))));
                        self.environment = new_env;
                        for (name, value) in bindings {
                            self.environment.borrow_mut().define(name, value);
                        }
                        self.execute(stmt)?;
                        self.environment = previous_env;
                        break;
                    }
                }
            }
            Stmt::Function { name, params: _, return_type: _, body: _ } => {
                // Store the function in the environment for future reference
                // For now, we'll just define the function name in the environment
                // with a placeholder value
                self.environment.borrow_mut().define(name.lexeme, ArtValue::Bool(false));
            }
            Stmt::Return { value } => {
                // For now, we'll just handle return statements by evaluating the expression
                // In a full implementation, this would actually return from the current function
                if let Some(expr) = value {
                    self.evaluate(expr)?;
                }
            }
        }
        Ok(())
    }

    fn pattern_matches(&self, pattern: &MatchPattern, value: &ArtValue) -> Option<Vec<(String, ArtValue)>> {
        match pattern {
            MatchPattern::Literal(lit) => {
                if lit == value {
                    Some(vec![])
                } else {
                    None
                }
            },
            MatchPattern::EnumVariant { variant, params } => {
                if let ArtValue::EnumInstance { enum_name: _, variant: v, values } = value {
                    if v != &variant.lexeme {
                        return None;
                    }
                    match params {
                        Some(param_names) => {
                            if param_names.len() != values.len() {
                                return None;
                            }
                            let mut bindings = Vec::new();
                            for (i, param) in param_names.iter().enumerate() {
                                bindings.push((param.lexeme.clone(), values[i].clone()));
                            }
                            Some(bindings)
                        },
                        None => {
                            if !values.is_empty() {
                                None
                            } else {
                                Some(vec![])
                            }
                        }
                    }
                } else {
                    None
                }
            },
            MatchPattern::Variable(name) => {
                Some(vec![(name.lexeme.clone(), value.clone())])
            },
            MatchPattern::Wildcard => Some(vec![]),
        }
    }

    fn execute_block(&mut self, statements: Vec<Stmt>, enclosing: Option<Rc<RefCell<Environment>>>) -> Result<()> {
        let previous = self.environment.clone();
        self.environment = Rc::new(RefCell::new(Environment::new(enclosing)));
        for statement in statements {
            self.execute(statement)?;
        }
        self.environment = previous;
        Ok(())
    }

    fn evaluate(&mut self, expr: Expr) -> Result<ArtValue> {
        match expr {
            Expr::Literal(value) => Ok(value),
            Expr::Grouping { expression } => self.evaluate(*expression),
            Expr::Variable { name } => {
                if self.type_registry.has_enum(&name.lexeme) {
                    return Ok(ArtValue::String(name.lexeme));
                }
                let name_str = name.lexeme.clone();
                self.environment.borrow().get(&name_str)
                    .ok_or(RuntimeError::UndefinedVariable(name_str.clone()))
                    .and_then(|val| {
                        if self.type_registry.has_struct(&name_str) {
                            Err(RuntimeError::TypeMismatch)
                        } else {
                            Ok(val)
                        }
                    })
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
            Expr::Call { callee, arguments } => {
                if let Expr::Variable { name } = *callee {
                    if name.lexeme == "println" {
                        if !arguments.is_empty() {
                            let value_to_print = self.evaluate(arguments[0].clone())?;
                            match value_to_print {
                                ArtValue::String(s) => println!("{}", s),
                                ArtValue::Int(n) => println!("{}", n),
                                ArtValue::Float(f) => println!("{}", f),
                                ArtValue::Bool(b) => println!("{}", b),
                                ArtValue::Optional(opt) => match *opt {
                                    Some(val) => println!("{:?}", val),
                                    None => println!("None"),
                                },
                                ArtValue::Array(arr) => println!("{:?}", arr),
                                ArtValue::StructInstance { struct_name, fields } => {
                                    println!("{} {{ {:?} }}", struct_name, fields);
                                },
                                ArtValue::EnumInstance { enum_name, variant, values } => {
                                    if values.is_empty() {
                                        println!("{}.{}", enum_name, variant);
                                    } else {
                                        println!("{}.{}({:?})", enum_name, variant, values);
                                    }
                                },
                            }
                        } else {
                            println!();
                        }
                        return Ok(ArtValue::Bool(false));
                    }
                }
                Ok(ArtValue::Bool(false))
            }
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
                let enum_def = self.type_registry.get_enum(&name.lexeme)
                    .ok_or_else(|| RuntimeError::Other(format!("Undefined enum '{}'.", name.lexeme)))?
                    .clone();
                let variant_def = enum_def.variants.iter()
                    .find(|(v_name, _)| v_name == &variant.lexeme)
                    .ok_or_else(|| RuntimeError::InvalidEnumVariant(variant.lexeme.clone()))?
                    .clone();
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
                    enum_name: name.lexeme,
                    variant: variant.lexeme,
                    values: evaluated_values,
                })
            }
            Expr::FieldAccess { object, field } => {
                let obj_value = self.evaluate(*object)?;
                match obj_value {
                    ArtValue::String(enum_name) if self.type_registry.has_enum(&enum_name) => {
                        let enum_def = self.type_registry.get_enum(&enum_name).unwrap();
                        let variant = enum_def.variants.iter()
                            .find(|(name, _)| name == &field.lexeme)
                            .ok_or_else(|| RuntimeError::InvalidEnumVariant(field.lexeme.clone()))?;
                        if variant.1.is_none() {
                            Ok(ArtValue::EnumInstance {
                                enum_name,
                                variant: field.lexeme,
                                values: Vec::new(),
                            })
                        } else {
                            Err(RuntimeError::WrongNumberOfArguments)
                        }
                    }
                    ArtValue::StructInstance { struct_name: _, fields } => {
                        fields.get(&field.lexeme)
                            .cloned()
                            .ok_or_else(|| RuntimeError::MissingField(field.lexeme.clone()))
                    }
                    ArtValue::Array(arr) => {
                        // Handle array methods
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
                            "count" => {
                                Ok(ArtValue::Int(arr.len() as i64))
                            }
                            _ => Err(RuntimeError::TypeMismatch),
                        }
                    }
                    _ => Err(RuntimeError::TypeMismatch),
                }
            }
            Expr::Try(inner) => match self.evaluate(*inner) {
                Ok(val) => Ok(val),
                Err(e) => Err(e),
            },
        }
    }

    fn is_truthy(&self, value: &ArtValue) -> bool {
        match value {
            ArtValue::Bool(b) => *b,
            ArtValue::Optional(opt) => match opt.as_ref() {
                Some(v) => self.is_truthy(v),
                None => false,
            },
            ArtValue::Int(n) => *n != 0,
            ArtValue::Float(f) => *f != 0.0,
            ArtValue::String(s) => !s.is_empty(),
            ArtValue::Array(arr) => !arr.is_empty(),
            ArtValue::StructInstance { .. } => true,
            ArtValue::EnumInstance { .. } => true,
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
            (ArtValue::Int(l), ArtValue::Int(r)) => Ok(ArtValue::Int(op(l as f64, r as f64) as i64)),
            (ArtValue::Float(l), ArtValue::Float(r)) => Ok(ArtValue::Float(op(l, r))),
            (ArtValue::Int(l), ArtValue::Float(r)) => Ok(ArtValue::Float(op(l as f64, r))),
            (ArtValue::Float(l), ArtValue::Int(r)) => Ok(ArtValue::Float(op(l, r as f64))),
            _ => Err(RuntimeError::TypeMismatch),
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
            _ => Err(RuntimeError::TypeMismatch),
        }
    }
}