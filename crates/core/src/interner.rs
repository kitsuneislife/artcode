use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::{Mutex, OnceLock};

fn pool() -> &'static Mutex<HashSet<&'static str>> {
    static POOL: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Interna uma &str retornando um ponteiro &'static str único para seu conteúdo.
/// Implementação simples: mantém HashSet global e faz leak de Box<str>. Isto é aceitável
/// porque o conjunto de símbolos cresce monotonicamente e é pequeno comparado ao tempo de vida do processo.
pub fn intern(s: &str) -> &'static str {
    let mut set = match pool().lock() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("interner mutex poisoned: {}", e);
            std::process::exit(1);
        }
    };
    if let Some(&existing) = set.get(s) {
        return existing;
    }
    let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
    set.insert(leaked);
    leaked
}

fn arc_pool() -> &'static Mutex<HashMap<String, Arc<str>>> {
    static ARC_POOL: OnceLock<Mutex<HashMap<String, Arc<str>>>> = OnceLock::new();
    ARC_POOL.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Interna uma string retornando um `Arc<str>` compartilhado.
///
/// Diferente de `intern`, este pool permite reaproveitar diretamente
/// o conteúdo em estruturas que armazenam `Arc<str>` (AST/runtime),
/// evitando alocações repetidas para literais idênticos.
pub fn intern_arc(s: &str) -> Arc<str> {
    let mut map = match arc_pool().lock() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("interner arc mutex poisoned: {}", e);
            std::process::exit(1);
        }
    };
    if let Some(existing) = map.get(s) {
        return existing.clone();
    }
    let created: Arc<str> = Arc::from(s.to_string());
    map.insert(s.to_string(), created.clone());
    created
}
