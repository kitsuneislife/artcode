use core::ast::ArtValue;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeError {
    UndefinedVariable(String),
    InvalidOperator,
    TypeMismatch,
    DivisionByZero,
    MissingField(String),
    InvalidEnumVariant(String),
    WrongNumberOfArguments,
    Other(String),
    Return(ArtValue),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::UndefinedVariable(name) => write!(f, "Undefined variable '{}'.", name),
            RuntimeError::InvalidOperator => write!(f, "Invalid operator."),
            RuntimeError::TypeMismatch => write!(f, "Type mismatch."),
            RuntimeError::DivisionByZero => write!(f, "Division by zero."),
            RuntimeError::MissingField(field) => write!(f, "Missing field '{}'.", field),
            RuntimeError::InvalidEnumVariant(variant) => write!(f, "Invalid enum variant '{}'.", variant),
            RuntimeError::WrongNumberOfArguments => write!(f, "Wrong number of arguments."),
            RuntimeError::Other(msg) => write!(f, "{}", msg),
            RuntimeError::Return(val) => write!(f, "Function returned a value (this should not be seen by user): {}", val),
        }
    }
}

pub type Result<T> = std::result::Result<T, RuntimeError>;