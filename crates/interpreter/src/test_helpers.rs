use crate::interpreter::Interpreter;
use core::ast::{ArtValue, Function, Stmt};
use std::rc::Rc;

/// Test helpers that expose minimal wrappers around internal interpreter helpers.
/// These are intended for tests only and keep the production API private.
pub mod test_helpers {
    use super::*;

    pub fn heap_create_atomic(interp: &mut Interpreter, initial: ArtValue) -> ArtValue {
        // forward to internal implementation
        interp.heap_create_atomic(initial)
    }

    pub fn heap_create_mutex(interp: &mut Interpreter, initial: ArtValue) -> ArtValue {
        interp.heap_create_mutex(initial)
    }

    pub fn heap_atomic_add(interp: &mut Interpreter, h: crate::interpreter::ObjHandle, delta: i64) -> Option<i64> {
        interp.heap_atomic_add(h, delta)
    }

    pub fn insert_finalizer(interp: &mut Interpreter, id: u64, func: Function) {
        interp.finalizers.insert(id, Rc::new(func));
    }

    pub fn force_heap_strong_to_one(interp: &mut Interpreter, id: u64) {
        interp.force_heap_strong_to_one(id);
    }

    pub fn dec_object_strong_recursive(interp: &mut Interpreter, id: u64) {
        interp.dec_object_strong_recursive(id);
    }
}
