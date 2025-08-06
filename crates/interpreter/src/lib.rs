
use parser::ast::{Expr, LiteralValue, Program, Stmt};
use parser::TokenType;

pub struct Interpreter {}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {}
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
        }
    }

    fn evaluate(&mut self, expr: Expr) -> LiteralValue {
        match expr {
            Expr::Literal(value) => value,
            Expr::Grouping { expression } => self.evaluate(*expression),
            Expr::Binary { left, operator, right } => {
                let left_val = self.evaluate(*left);
                let right_val = self.evaluate(*right);

                if let (LiteralValue::Number(left_num), LiteralValue::Number(right_num)) = (left_val, right_val) {
                    match operator.token_type {
                        TokenType::Plus => LiteralValue::Number(left_num + right_num),
                        TokenType::Minus => LiteralValue::Number(left_num - right_num),
                        TokenType::Star => LiteralValue::Number(left_num * right_num),
                        TokenType::Slash => {
                            LiteralValue::Number(left_num / right_num)
                        }
                        _ => panic!("Operador binário inválido: {:?}", operator),
                    }
                } else {
                    panic!("Operandos devem ser números para operações aritméticas.");
                }
            }
            Expr::Variable { .. } => {
                LiteralValue::String("dummy".to_string())
            }
            Expr::Call { callee, arguments } => {
                if let Expr::Variable { name } = *callee {
                    if name.lexeme == "println" {
                        if arguments.is_empty() { return LiteralValue::String("".to_string()); }
                        let value_to_print = self.evaluate(arguments[0].clone());

                        match value_to_print {
                            LiteralValue::String(s) => println!("{}", s),
                            LiteralValue::Number(n) => println!("{}", n),
                        }
                    }
                }
                LiteralValue::String("".to_string())
            }
        }
    }
}
