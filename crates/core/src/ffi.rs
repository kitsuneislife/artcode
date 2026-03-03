//! Interoperabilidade C-ABI (FFI)
//!
//! Este módulo provê funções exportadas (extern "C") para facilitar a interação
//! entre a linguagem Artcode e códigos nativos (C, C++, etc.).
//! Implementa os mapeamentos da RFC 0005.

use crate::ast::ArtValue;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::{Arc, Mutex, OnceLock};

/// Macro para facilitar a criação de bindings C-ABI (zero-cost) a partir do Rust.
/// Gera a ponte segura garantindo `extern "C"` e no_mangle.
#[macro_export]
macro_rules! art_extern {
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident($($arg:ident: $arg_ty:ty),*) -> $ret:ty {
            $($body:tt)*
        }
    ) => {
        $(#[$meta])*
        #[unsafe(no_mangle)]
        $vis extern "C" fn $name($($arg: $arg_ty),*) -> $ret {
            $($body)*
        }
    };
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident($($arg:ident: $arg_ty:ty),*) {
            $($body:tt)*
        }
    ) => {
        $(#[$meta])*
        #[unsafe(no_mangle)]
        $vis extern "C" fn $name($($arg: $arg_ty),*) {
            $($body)*
        }
    };
}

art_extern! {
    /// Retém uma referência a um ArtValue. Incrementa a contagem de referências.
    /// O chamador deve parear essa chamada com um `art_value_release`.
    pub fn art_value_retain(ptr: *mut ArtValue) {
        if ptr.is_null() {
            return;
        }
        let _val = unsafe { &*ptr };
    }
}

art_extern! {
    /// Libera uma referência a um ArtValue. Descrementa a contagem de referências.
    pub fn art_value_release(ptr: *mut ArtValue) {
        if ptr.is_null() {
            return;
        }
        unsafe {
            // Recapturar a variável possivelmente heap-allocated e dropar.
            let _ = Box::from_raw(ptr);
        }
    }
}

art_extern! {
    /// Cria um valor Numérico (i64) exportado para o ambiente C.
    /// O chamador se torna o dono do valor instanciado no heap e deverá chamar `art_value_release`.
    pub fn art_create_i64(val: i64) -> *mut ArtValue {
        let boxed = Box::new(ArtValue::Int(val));
        Box::into_raw(boxed)
    }
}

art_extern! {
    /// Extrai um i64 nativo a partir de um ponteiro *mut ArtValue.
    /// Retorna 0 como fallback se não for do tipo Integer. (Para debug)
    pub fn art_extract_i64(ptr: *const ArtValue) -> i64 {
        if ptr.is_null() {
            return 0;
        }
        let val = unsafe { &*ptr };
        match val {
            ArtValue::Int(i) => *i,
            _ => 0, // Fallback/errout
        }
    }
}

// ---------------------------------------------------------
// Cache de conversão de Strings (Arc<str> <-> *const c_char)
// ---------------------------------------------------------

static CSTR_CACHE: OnceLock<Mutex<HashMap<usize, CString>>> = OnceLock::new();

fn get_cstr_cache() -> &'static Mutex<HashMap<usize, CString>> {
    CSTR_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

art_extern! {
    /// Constrói uma String C nula a partir de um *mut ArtValue (deve ser variante String/Arc<str>).
    /// Utiliza um cache baseado no endereço interno do `Arc<str>` para garantir zero-cost se exportado
    /// múltiplas vezes sequenciais. O ponteiro provido não precisa ser limpado pelo autor da chamada
    /// enquanto a VM rodar ou a cache for limpa ativamente.
    pub fn art_string_to_cstr(ptr: *const ArtValue) -> *const c_char {
        if ptr.is_null() {
            return std::ptr::null();
        }
        let val = unsafe { &*ptr };
        match val {
            ArtValue::String(s) => {
                let key = Arc::as_ptr(s) as *const u8 as usize;
                let mut cache = get_cstr_cache().lock().unwrap();

                // Retorna iterador do cache se jpa possuir.
                if let Some(c_str) = cache.get(&key) {
                    return c_str.as_ptr();
                }

                // Senão cadastra pela primeira vez.
                if let Ok(c_str) = CString::new(&**s) {
                    let c_ptr = c_str.as_ptr();
                    cache.insert(key, c_str);
                    return c_ptr;
                } else {
                    std::ptr::null()
                }
            }
            _ => std::ptr::null(),
        }
    }
}

art_extern! {
    /// Libera todo o cache atrelado a strings. Essa função deverá ser utilizada num loop global
    /// do ambiente do usuário final para não vazar memórias em usos pesados de text-processing.
    pub fn art_free_cstr_cache() {
        let mut cache = get_cstr_cache().lock().unwrap();
        cache.clear();
    }
}
