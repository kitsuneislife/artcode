use core::ast::ArtValue;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeError {
    Return(ArtValue),
    TypeError(String),
    DebugStepBack,
    DebugQuit,
    DebugJumpTo(usize),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::Return(val) => write!(f, "Function returned: {}", val),
            RuntimeError::TypeError(msg) => write!(f, "Type error: {}", msg),
            RuntimeError::DebugStepBack => write!(f, "Debug step back requested"),
            RuntimeError::DebugQuit => write!(f, "Debug quit"),
            RuntimeError::DebugJumpTo(tick) => write!(f, "Debug jump to tick {}", tick),
        }
    }
}

pub type Result<T> = std::result::Result<T, RuntimeError>;
