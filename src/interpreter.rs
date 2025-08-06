
use ast::{Expr, LiteralValue, Program, Stmt};

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
            Expr::Variable { .. } => {
                LiteralValue::String("dummy".to_string())
            }
            Expr::Call { callee, arguments } => {
                if let Expr::Variable { name } = *callee {
                    if name.lexeme == "println" {
                        let value_to_print = self.evaluate(arguments[0].clone());

                        match value_to_print {
                            LiteralValue::String(s) => println!("{}", s),
                        }
                    }
                }
                LiteralValue::String("".to_string())
            }
        }
    }
}
