use std::fs;
use std::path::PathBuf;

/// Incremental Caching system for JIT/AOT compilation payloads.
pub struct ArtCache {
    base_dir: PathBuf,
}

impl ArtCache {
    pub fn new() -> Self {
        let mut dir = std::env::current_dir().unwrap_or_else(|_| std::env::temp_dir());
        dir.push(".artcache");
        let _ = fs::create_dir_all(&dir);
        Self { base_dir: dir }
    }

    /// Very fast FNV-1a 64-bit hashing for string collision reduction
    pub fn compute_hash(payload: &str) -> String {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in payload.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        format!("{:016x}", hash)
    }

    pub fn get(&self, prefix: &str, hash: &str, ext: &str) -> Option<String> {
        let file_name = format!("{}_{}.{}", prefix, hash, ext);
        let path = self.base_dir.join(&file_name);
        fs::read_to_string(path).ok()
    }

    pub fn set(&self, prefix: &str, hash: &str, ext: &str, content: &str) {
        let file_name = format!("{}_{}.{}", prefix, hash, ext);
        let path = self.base_dir.join(&file_name);
        let _ = fs::write(path, content);
    }

    pub fn check_binary(&self, prefix: &str, hash: &str, ext: &str) -> Option<PathBuf> {
        let file_name = format!("{}_{}.{}", prefix, hash, ext);
        let path = self.base_dir.join(&file_name);
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }
}
