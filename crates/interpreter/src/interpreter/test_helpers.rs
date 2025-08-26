use crate::interpreter::Interpreter;
use core::ast::{ArtValue, Function, ObjHandle};
use std::rc::Rc;

/// Test helpers dentro do módulo `interpreter` para acessar helpers privados em testes.

pub fn heap_create_atomic(interp: &mut Interpreter, initial: ArtValue) -> ArtValue {
    interp.heap_create_atomic(initial)
}

pub fn heap_create_mutex(interp: &mut Interpreter, initial: ArtValue) -> ArtValue {
    interp.heap_create_mutex(initial)
}

pub fn heap_atomic_add(interp: &mut Interpreter, h: ObjHandle, delta: i64) -> Option<i64> {
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

pub fn heap_object_ids(interp: &Interpreter) -> Vec<u64> {
    interp.heap_objects.keys().cloned().collect()
}
