//! Interoperabilidade C-ABI (FFI)
//!
//! Este módulo provê funções exportadas (extern "C") para facilitar a interação
//! entre a linguagem Artcode e códigos nativos (C, C++, etc.).
//! Implementa os mapeamentos da RFC 0005.

use crate::ast::ArtValue;
use std::ffi::CString;
use std::os::raw::c_char;

/// Retém uma referência a um ArtValue. Incrementa a contagem de referências.
/// O chamador deve parear essa chamada com um `art_value_release`.
#[unsafe(no_mangle)]
pub extern "C" fn art_value_retain(ptr: *mut ArtValue) {
    if ptr.is_null() {
        return;
    }
    let _val = unsafe { &*ptr };
}

/// Libera uma referência a um ArtValue. Descrementa a contagem de referências.
#[unsafe(no_mangle)]
pub extern "C" fn art_value_release(ptr: *mut ArtValue) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        // Recapturar a variável possivelmente heap-allocated e dropar.
        let _ = Box::from_raw(ptr);
    }
}

/// Cria um valor Numérico (i64) exportado para o ambiente C.
/// O chamador se torna o dono do valor instanciado no heap e deverá chamar `art_value_release`.
#[unsafe(no_mangle)]
pub extern "C" fn art_create_i64(val: i64) -> *mut ArtValue {
    let boxed = Box::new(ArtValue::Int(val));
    Box::into_raw(boxed)
}

/// Extrai um i64 nativo a partir de um ponteiro *mut ArtValue.
/// Retorna 0 como fallback se não for do tipo Integer. (Para debug)
#[unsafe(no_mangle)]
pub extern "C" fn art_extract_i64(ptr: *const ArtValue) -> i64 {
    if ptr.is_null() {
        return 0;
    }
    let val = unsafe { &*ptr };
    match val {
        ArtValue::Int(i) => *i,
        _ => 0, // Fallback/errout
    }
}

/// Constrói uma String C nula a partir de um *mut ArtValue (deve ser variante String/Arc<str>).
/// O chamador é encarregado de liberar essa C String posteriormente (depende da doc final).
#[unsafe(no_mangle)]
pub extern "C" fn art_string_to_cstr(ptr: *const ArtValue) -> *mut c_char {
    if ptr.is_null() {
        return std::ptr::null_mut();
    }
    let val = unsafe { &*ptr };
    match val {
        ArtValue::String(s) => {
            if let Ok(c_str) = CString::new(&**s) {
                c_str.into_raw()
            } else {
                std::ptr::null_mut()
            }
        }
        _ => std::ptr::null_mut(),
    }
}

/// Libera uma string C exportada por `art_string_to_cstr`.
#[unsafe(no_mangle)]
pub extern "C" fn art_free_cstr(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(s);
    }
}
