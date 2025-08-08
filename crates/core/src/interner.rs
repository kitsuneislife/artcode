use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

fn pool() -> &'static Mutex<HashSet<&'static str>> {
    static POOL: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Interna uma &str retornando um ponteiro &'static str único para seu conteúdo.
/// Implementação simples: mantém HashSet global e faz leak de Box<str>. Isto é aceitável
/// porque o conjunto de símbolos cresce monotonicamente e é pequeno comparado ao tempo de vida do processo.
pub fn intern(s: &str) -> &'static str {
    let mut set = pool().lock().unwrap();
    if let Some(&existing) = set.get(s) { return existing; }
    let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
    set.insert(leaked);
    leaked
}
