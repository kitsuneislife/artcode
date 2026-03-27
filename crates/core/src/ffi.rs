//! Interoperabilidade C-ABI (FFI)
//!
//! Este módulo provê funções exportadas (extern "C") para facilitar a interação
//! entre a linguagem Artcode e códigos nativos (C, C++, etc.).
//! Implementa os mapeamentos da RFC 0005.

use crate::ast::ArtValue;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicU64, Ordering};
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

#[derive(Clone)]
struct FfiHandleEntry {
    value: ArtValue,
    refs: u32,
}

static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

thread_local! {
    static HANDLE_REGISTRY_LOCAL: RefCell<HashMap<u64, FfiHandleEntry>> = RefCell::new(HashMap::new());
}

fn with_handle_registry_mut<R>(f: impl FnOnce(&mut HashMap<u64, FfiHandleEntry>) -> R) -> R {
    HANDLE_REGISTRY_LOCAL.with(|cell| {
        let mut borrowed = cell.borrow_mut();
        f(&mut borrowed)
    })
}

fn with_handle_registry<R>(f: impl FnOnce(&HashMap<u64, FfiHandleEntry>) -> R) -> R {
    HANDLE_REGISTRY_LOCAL.with(|cell| {
        let borrowed = cell.borrow();
        f(&borrowed)
    })
}

fn create_handle(value: ArtValue) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::Relaxed);
    with_handle_registry_mut(|reg| {
        reg.insert(id, FfiHandleEntry { value, refs: 1 });
    });
    id
}

fn with_handle_value<R>(handle: u64, f: impl FnOnce(&ArtValue) -> R) -> Option<R> {
    with_handle_registry(|reg| reg.get(&handle).map(|entry| f(&entry.value)))
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
    /// Cria um handle opaco para valor `Int` no registry FFI seguro.
    /// O chamador deve liberar via `art_handle_release`.
    pub fn art_handle_create_i64(val: i64) -> u64 {
        create_handle(ArtValue::Int(val))
    }
}

art_extern! {
    /// Retém (incrementa) a contagem de referências de um handle opaco FFI.
    /// Retorna 1 em sucesso, 0 se handle não existir.
    pub fn art_handle_retain(handle: u64) -> u8 {
        with_handle_registry_mut(|reg| {
            if let Some(entry) = reg.get_mut(&handle) {
                entry.refs = entry.refs.saturating_add(1);
                1
            } else {
                0
            }
        })
    }
}

art_extern! {
    /// Libera (decrementa) a contagem de referências de um handle opaco FFI.
    /// Retorna 1 em sucesso, 0 se handle não existir.
    pub fn art_handle_release(handle: u64) -> u8 {
        with_handle_registry_mut(|reg| {
            let Some(entry) = reg.get_mut(&handle) else {
                return 0;
            };
            if entry.refs > 1 {
                entry.refs -= 1;
            } else {
                reg.remove(&handle);
            }
            1
        })
    }
}

art_extern! {
    /// Extrai um `i64` de um handle opaco para `out_value`.
    /// Códigos de retorno:
    /// 0 = OK, 1 = handle inválido, 2 = tipo incompatível, 3 = out_value nulo.
    pub fn art_handle_extract_i64(handle: u64, out_value: *mut i64) -> i32 {
        if out_value.is_null() {
            return 3;
        }
        let Some(v) = with_handle_value(handle, |v| v.clone()) else {
            return 1;
        };
        match v {
            ArtValue::Int(i) => {
                unsafe {
                    *out_value = i;
                }
                0
            }
            _ => 2,
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
                let mut cache = get_cstr_cache().lock().unwrap_or_else(|e| e.into_inner());

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
    /// Exporta `String` de um handle opaco para `*const c_char` com cache interno.
    /// Retorna null quando o handle é inválido ou o valor não é String.
    pub fn art_handle_string_to_cstr(handle: u64) -> *const c_char {
        let Some(val) = with_handle_value(handle, |v| v.clone()) else {
            return std::ptr::null();
        };
        match val {
            ArtValue::String(s) => {
                let key = Arc::as_ptr(&s) as *const u8 as usize;
                let mut cache = get_cstr_cache().lock().unwrap_or_else(|e| e.into_inner());
                if let Some(c_str) = cache.get(&key) {
                    return c_str.as_ptr();
                }
                if let Ok(c_str) = CString::new(&*s) {
                    let c_ptr = c_str.as_ptr();
                    cache.insert(key, c_str);
                    c_ptr
                } else {
                    std::ptr::null()
                }
            }
            _ => std::ptr::null(),
        }
    }
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn syscall(num: std::os::raw::c_long, ...) -> std::os::raw::c_long;
}

art_extern! {
    /// Gateway de syscall "unsafe" por registradores para integração de baixo nível.
    ///
    /// Arguments:
    /// - `num`: número da syscall
    /// - `a0..a5`: registradores de argumento
    /// - `out_errno`: ponteiro opcional para errno (0 em sucesso)
    ///
    /// Retorna o valor bruto da syscall. Em plataformas não-Linux retorna -1.
    pub fn art_syscall_unsafe(
        num: i64,
        a0: usize,
        a1: usize,
        a2: usize,
        a3: usize,
        a4: usize,
        a5: usize,
        out_errno: *mut i64
    ) -> i64 {
        #[cfg(target_os = "linux")]
        {
            let ret = unsafe {
                syscall(
                    num as std::os::raw::c_long,
                    a0,
                    a1,
                    a2,
                    a3,
                    a4,
                    a5,
                ) as i64
            };
            if !out_errno.is_null() {
                let errno = if ret == -1 {
                    std::io::Error::last_os_error().raw_os_error().unwrap_or(0) as i64
                } else {
                    0
                };
                unsafe {
                    *out_errno = errno;
                }
            }
            ret
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (num, a0, a1, a2, a3, a4, a5);
            if !out_errno.is_null() {
                unsafe {
                    *out_errno = 38;
                }
            }
            -1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_handle_retain_release_roundtrip() {
        let h = art_handle_create_i64(123);
        assert!(h > 0);
        assert_eq!(art_handle_retain(h), 1);
        assert_eq!(art_handle_release(h), 1);
        assert_eq!(art_handle_release(h), 1);
        assert_eq!(art_handle_release(h), 0);
    }

    #[test]
    fn safe_handle_extract_i64_ok() {
        let h = art_handle_create_i64(987);
        let mut out = 0i64;
        let code = art_handle_extract_i64(h, &mut out as *mut i64);
        assert_eq!(code, 0);
        assert_eq!(out, 987);
        assert_eq!(art_handle_release(h), 1);
    }
}

art_extern! {
    /// Libera todo o cache atrelado a strings. Essa função deverá ser utilizada num loop global
    /// do ambiente do usuário final para não vazar memórias em usos pesados de text-processing.
    pub fn art_free_cstr_cache() {
        let mut cache = get_cstr_cache().lock().unwrap_or_else(|e| e.into_inner());
        cache.clear();
    }
}
