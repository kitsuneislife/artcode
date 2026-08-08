//! String interning bounded by live data.
//!
//! An earlier version kept two permanent global pools and handed out
//! `&'static str` obtained from `Box::leak`. That is defensible for
//! `art run script.art`, which exits promptly, and wrong for every long-lived
//! process: `art lsp` re-lexes the open file on each keystroke, and the fuzzers
//! run thousands of programs without restarting, so memory grew with the input
//! rather than with any program's vocabulary.
//!
//! The pool now holds `Weak<str>`, so an entry lives exactly as long as some
//! value still refers to the string. Dead entries are swept when the map grows
//! past a moving threshold, which keeps the keys from accumulating too.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};

/// Entries tolerated before a sweep is considered, and the floor the threshold
/// resets to. Small enough that a leak cannot hide, large enough that ordinary
/// programs never trigger a sweep.
const SWEEP_FLOOR: usize = 256;

struct Pool {
    entries: HashMap<Box<str>, Weak<str>>,
    /// Size at which the next sweep runs. Doubles after a sweep that frees
    /// little, so a program with a genuinely large vocabulary does not pay for
    /// a sweep on every insert.
    sweep_at: usize,
}

impl Pool {
    fn new() -> Self {
        Pool {
            entries: HashMap::new(),
            sweep_at: SWEEP_FLOOR,
        }
    }

    /// Drops entries whose last strong reference is gone.
    fn sweep(&mut self) {
        self.entries.retain(|_, weak| weak.strong_count() > 0);
        self.sweep_at = self.entries.len().saturating_mul(2).max(SWEEP_FLOOR);
    }
}

fn pool() -> &'static Mutex<Pool> {
    static POOL: OnceLock<Mutex<Pool>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(Pool::new()))
}

/// Locks the pool, recovering from a poisoned mutex.
///
/// A panic in another thread while holding this lock cannot corrupt the pool:
/// the map is only ever left in a consistent state between operations, so
/// taking the inner value is safe and preferable to aborting the process.
fn lock_pool() -> std::sync::MutexGuard<'static, Pool> {
    match pool().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Returns a shared `Arc<str>` for `s`, reusing the existing allocation when
/// one is still alive.
///
/// Interning pays off where the vocabulary is closed and hit repeatedly —
/// `type_of` returning one of a fixed set of type names, enum variant names,
/// string literals reused across a parse. It never retains anything past the
/// last user.
pub fn intern_arc(s: &str) -> Arc<str> {
    let mut pool = lock_pool();

    if let Some(existing) = pool.entries.get(s).and_then(Weak::upgrade) {
        return existing;
    }

    let created: Arc<str> = Arc::from(s);
    pool.entries.insert(Box::from(s), Arc::downgrade(&created));

    if pool.entries.len() >= pool.sweep_at {
        pool.sweep();
    }

    created
}

/// Number of entries currently retained by the pool, live or not yet swept.
///
/// Exposed for the tests that assert the pool tracks the vocabulary in use
/// rather than everything the process has ever seen.
pub fn interned_arc_count() -> usize {
    lock_pool().entries.len()
}

/// Drops every entry whose last strong reference is gone, and reports how many
/// remain. Tests call this to observe steady state without waiting for the
/// growth threshold.
pub fn sweep_interned() -> usize {
    let mut pool = lock_pool();
    pool.sweep();
    pool.entries.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_the_same_allocation_while_alive() {
        let a = intern_arc("shared_symbol");
        let b = intern_arc("shared_symbol");
        assert!(Arc::ptr_eq(&a, &b), "live entries must be reused");
    }

    #[test]
    fn releases_entries_once_nothing_refers_to_them() {
        let unique = format!("transient_{}", std::process::id());
        drop(intern_arc(&unique));

        sweep_interned();

        let pool = lock_pool();
        assert!(
            !pool.entries.contains_key(unique.as_str()),
            "a string nobody holds must not survive a sweep"
        );
        // A later request allocates afresh rather than resurrecting a dead Weak.
        drop(pool);
        let revived = intern_arc(&unique);
        assert_eq!(&*revived, unique.as_str());
    }

    #[test]
    fn sweeping_keeps_strings_that_are_still_held() {
        let held = intern_arc("still_referenced");
        sweep_interned();
        let again = intern_arc("still_referenced");
        assert!(Arc::ptr_eq(&held, &again));
    }
}
