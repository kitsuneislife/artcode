use core::ast::ArtValue;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeError {
    Return(ArtValue),
    TypeError(String),
    DebugStepBack,
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::Return(val) => write!(f, "Function returned: {}", val),
            RuntimeError::TypeError(msg) => write!(f, "Type error: {}", msg),
            RuntimeError::DebugStepBack => write!(f, "Debug step back requested"),
        }
    }
}

pub type Result<T> = std::result::Result<T, RuntimeError>;
