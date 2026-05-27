// ArtValue is intentionally non-Send (contains Rc). Arc<Mutex<ArtValue>> is used
// for shared ownership within a single thread; no cross-thread sharing occurs.
#![allow(clippy::arc_with_non_send_sync)]

pub mod field_access;
pub mod fstring;
pub mod heap;
pub mod heap_utils;
pub mod interpreter;
pub mod replayer;
pub mod tracer;
// keep top-level compatibility: re-export interpreter::test_helpers as test_helpers only for tests
#[cfg(test)]
pub use interpreter::test_helpers;
pub mod type_infer;
pub mod type_registry;
pub mod values;

pub use interpreter::Interpreter;
pub use values::RuntimeError;
