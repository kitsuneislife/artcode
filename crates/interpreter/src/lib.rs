mod environment;
mod interpreter;
mod values;

use parser::ast::{Expr, LiteralValue, Program, Stmt};
use parser::TokenType;
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

#[derive(Clone)]
struct Environment {
    enclosing: Option<Rc<RefCell<Environment>>>,
    values: HashMap<String, LiteralValue>,
}

impl Environment {
    fn new(enclosing: Option<Rc<RefCell<Environment>>>) -> Self {
        Environment {
            enclosing,
            values: HashMap::new(),
        }
    }

    fn define(&mut self, name: String, value: LiteralValue) {
        self.values.insert(name, value);
    }

    fn get(&self, name: &str) -> Option<LiteralValue> {
        if let Some(value) = self.values.get(name) {
            return Some(value.clone());
        }
        if let Some(enclosing) = &self.enclosing {
            return enclosing.borrow().get(name);
        }
        None
    }
}

pub struct Interpreter {
    environment: Rc<RefCell<Environment>>,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            environment: Rc::new(RefCell::new(Environment::new(None))),
        }
    }

    pub fn interpret(&mut self, program: Program) {
        for statement in program {
            self.execute(statement);
        }
    }

    fn execute(&mut self, stmt: Stmt) {
        match stmt {
            Stmt::Expression(expr) => {
                self.evaluate(expr);
            }
            Stmt::Let { name, initializer } => {
                let value = self.evaluate(initializer);
                self.environment.borrow_mut().define(name.lexeme, value);
            }
            Stmt::Block { statements } => {
                self.execute_block(statements, Some(self.environment.clone()));
            }
            Stmt::If { condition, then_branch, else_branch } => {
                let condition_value = self.evaluate(condition);
                if self.is_truthy(condition_value) {
                    self.execute(*then_branch);
                } else if let Some(else_stmt) = else_branch {
                    self.execute(*else_stmt);
                }
            }
        }
    }

    fn execute_block(&mut self, statements: Vec<Stmt>, enclosing: Option<Rc<RefCell<Environment>>>) {
        let previous = self.environment.clone();
        self.environment = Rc::new(RefCell::new(Environment::new(enclosing)));
        for statement in statements {
            self.execute(statement);
        }
        self.environment = previous;
    }

    fn evaluate(&mut self, expr: Expr) -> LiteralValue {
        match expr {
            Expr::Literal(value) => value,
            Expr::Grouping { expression } => self.evaluate(*expression),
            Expr::Variable { name } => self
                .environment
                .borrow()
                .get(&name.lexeme)
                .unwrap_or_else(|| panic!("Undefined variable '{}'.", name.lexeme)),
            Expr::Unary { operator, right } => {
                let right_val = self.evaluate(*right);
                match operator.token_type {
                    TokenType::Minus => {
                        if let LiteralValue::Number(n) = right_val {
                            LiteralValue::Number(-n)
                        } else {
                            panic!("Operand must be a number.");
                        }
                    }
                    TokenType::Bang => LiteralValue::Bool(!self.is_truthy(right_val)),
                    _ => panic!("Invalid unary operator."),
                }
            }
            Expr::Logical { left, operator, right } => {
                let left_val = self.evaluate(*left);
                if operator.token_type == TokenType::Or {
                    if self.is_truthy(left_val.clone()) { return left_val; }
                } else {
                    if !self.is_truthy(left_val.clone()) { return left_val; }
                }
                self.evaluate(*right)
            }
            Expr::Binary { left, operator, right } => {
                let left_val = self.evaluate(*left);
                let right_val = self.evaluate(*right);

                match operator.token_type {
                    TokenType::Plus => {
                        if let (LiteralValue::Number(l), LiteralValue::Number(r)) = (&left_val, &right_val) {
                            return LiteralValue::Number(l + r);
                        }
                        if let (LiteralValue::String(l), LiteralValue::String(r)) = (left_val, right_val) {
                            return LiteralValue::String(format!("{}{}", l, r));
                        }
                        panic!("Operands must be two numbers or two strings.");
                    }
                    TokenType::Minus => self.binary_num_op(left_val, right_val, |a, b| a - b),
                    TokenType::Star => self.binary_num_op(left_val, right_val, |a, b| a * b),
                    TokenType::Slash => self.binary_num_op(left_val, right_val, |a, b| a / b),
                    TokenType::Greater => self.binary_num_op(left_val, right_val, |a, b| a > b),
                    TokenType::GreaterEqual => self.binary_num_op(left_val, right_val, |a, b| a >= b),
                    TokenType::Less => self.binary_num_op(left_val, right_val, |a, b| a < b),
                    TokenType::LessEqual => self.binary_num_op(left_val, right_val, |a, b| a <= b),
                    TokenType::BangEqual => LiteralValue::Bool(!self.is_equal(left_val, right_val)),
                    TokenType::EqualEqual => LiteralValue::Bool(self.is_equal(left_val, right_val)),
                    _ => panic!("Invalid binary operator."),
                }
            }
            Expr::Call { callee, arguments } => {
                if let Expr::Variable { name } = *callee {
                    if name.lexeme == "println" {
                        if !arguments.is_empty() {
                            let value_to_print = self.evaluate(arguments[0].clone());
                            match value_to_print {
                                LiteralValue::String(s) => println!("{}", s),
                                LiteralValue::Number(n) => println!("{}", n),
                                LiteralValue::Bool(b) => println!("{}", b),
                            }
                        } else {
                            println!();
                        }
                    }
                }
                LiteralValue::Bool(false)
            }
        }
    }

    fn is_truthy(&self, value: LiteralValue) -> bool {
        match value {
            LiteralValue::Bool(b) => b,
            _ => false,
        }
    }

    fn is_equal(&self, a: LiteralValue, b: LiteralValue) -> bool {
        a == b
    }

    fn binary_num_op<F, T>(&self, left: LiteralValue, right: LiteralValue, op: F) -> LiteralValue
    where F: Fn(f64, f64) -> T, T: Into<LiteralValue> {
        if let (LiteralValue::Number(l), LiteralValue::Number(r)) = (left, right) {
            op(l, r).into()
        } else {
            panic!("Operands must be numbers.");
        }
    }
}
