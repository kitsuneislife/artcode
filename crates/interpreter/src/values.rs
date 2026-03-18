use core::ast::ArtValue;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeError {
    Return(ArtValue),
    TypeError(String),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::Return(val) => write!(f, "Function returned: {}", val),
            RuntimeError::TypeError(msg) => write!(f, "Type error: {}", msg),
        }
    }
}

pub type Result<T> = std::result::Result<T, RuntimeError>;
