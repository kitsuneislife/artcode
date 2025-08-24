use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use lexer::lexer::Lexer;
use parser::parser::Parser;

/// Resolve imports starting from `entry` file. Returns Ok((program, main_source)) on success.
/// On error returns a vector of (source_string, Diagnostic) for diagnostics produced while
/// lexing/parsing any file.
pub fn resolve(entry: &str) -> Result<(core::Program, String), Vec<(String, diagnostics::Diagnostic)>> {
    let mut visited: HashSet<PathBuf> = HashSet::new();
    let entry_path = PathBuf::from(entry);

    // Read entry source early to return it for formatting runtime diagnostics
    let main_source = match fs::read_to_string(&entry_path) {
        Ok(s) => s,
        Err(e) => {
            let diag = diagnostics::Diagnostic::new(diagnostics::DiagnosticKind::Parse, format!("Failed to read {}: {}", entry, e), diagnostics::Span::new(0,0,0,0));
            return Err(vec![(String::new(), diag)]);
        }
    };

    let mut out_program: core::Program = Vec::new();
    let mut errors: Vec<(String, diagnostics::Diagnostic)> = Vec::new();

    // If a .art-lock exists in the entry's directory, parse it and build a map of locked names to paths
    let mut lock_map: std::collections::HashMap<String, PathBuf> = std::collections::HashMap::new();
    if let Some(parent) = entry_path.parent() {
        let lock = parent.join(".art-lock");
        if lock.exists() {
            if let Ok(s) = std::fs::read_to_string(&lock) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
                    if let Some(name) = v.get("name").and_then(|n| n.as_str()) {
                        if let Some(path) = v.get("path").and_then(|p| p.as_str()) {
                            lock_map.insert(name.to_string(), PathBuf::from(path));
                        }
                    }
                }
            }
        }
    }

    fn resolve_candidate(base: &Path, rel: &str) -> Option<PathBuf> {
        // If rel is absolute, try directly
        let rel_path = PathBuf::from(rel);
        if rel_path.is_absolute() {
            if rel_path.exists() {
                return Some(rel_path);
            }
            let mut with_ext = rel_path.clone();
            with_ext.set_extension("art");
            if with_ext.exists() {
                return Some(with_ext);
            }
            return None;
        }
        // Otherwise join with base
        let cand = base.join(rel);
        if cand.exists() {
            return Some(cand);
        }
        let mut cand_ext = cand.clone();
        cand_ext.set_extension("art");
        if cand_ext.exists() {
            return Some(cand_ext);
        }
        // try as directory with mod.art
        let cand_mod = cand.join("mod.art");
        if cand_mod.exists() {
            return Some(cand_mod);
        }
    // Not found locally — but first check if project lock_map provides a pinned path
    // (lock_map is captured from outer scope via move closure semantics in Rust; since this is a nested fn we will re-check in caller)

    // Not found locally — try user cache (~/.artcode/cache)
        if let Some(home) = dirs::home_dir() {
            let cache_dir = home.join(".artcode").join("cache");
            // Try cached package by rel (name or name-version)
            let cand_cache = cache_dir.join(rel);
            if cand_cache.exists() {
                // If it's a file, return it; if dir, try to confirm via Art.toml/main or fallback to main.art/mod.art
                if cand_cache.is_file() {
                    return Some(cand_cache);
                }
                // try Art.toml 'main' field first
                let art_toml = cand_cache.join("Art.toml");
                if art_toml.exists() {
                    if let Ok(s) = std::fs::read_to_string(&art_toml) {
                        if let Ok(v) = toml::from_str::<toml::Value>(&s) {
                            if let Some(mainf) = v.get("main").and_then(|m| m.as_str()) {
                                let candidate_main = cand_cache.join(mainf);
                                if candidate_main.exists() {
                                    return Some(candidate_main);
                                }
                            }
                        }
                    }
                }
                let m1 = cand_cache.join("main.art");
                if m1.exists() {
                    return Some(m1);
                }
                let m2 = cand_cache.join("mod.art");
                if m2.exists() {
                    return Some(m2);
                }
            }
            // Try any package directory under cache that starts with rel (name-)
            if let Ok(entries) = std::fs::read_dir(&cache_dir) {
                for e in entries.flatten() {
                    let p = e.path();
                    // If directory, try to read Art.toml to match package name
                    if p.is_dir() {
                        let art_toml = p.join("Art.toml");
                        if art_toml.exists() {
                            if let Ok(s) = std::fs::read_to_string(&art_toml) {
                                if let Ok(v) = toml::from_str::<toml::Value>(&s) {
                                    if let Some(name_v) = v.get("name").and_then(|n| n.as_str()) {
                                        if name_v == rel {
                                            // prefer main field
                                            if let Some(mainf) = v.get("main").and_then(|m| m.as_str()) {
                                                let candidate_main = p.join(mainf);
                                                if candidate_main.exists() { return Some(candidate_main); }
                                            }
                                            let m1 = p.join("main.art");
                                            if m1.exists() { return Some(m1); }
                                            let m2 = p.join("mod.art");
                                            if m2.exists() { return Some(m2); }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // fallback: match by directory name prefix
                    if let Some(fname) = p.file_name().and_then(|s| s.to_str()) {
                        if fname.starts_with(rel) {
                            let m1 = p.join("main.art");
                            if m1.exists() { return Some(m1); }
                            let m2 = p.join("mod.art");
                            if m2.exists() { return Some(m2); }
                        }
                    }
                }
            }
        }
        None
    }

    fn process_file(path: &Path, visited: &mut HashSet<PathBuf>, out: &mut core::Program, errors: &mut Vec<(String, diagnostics::Diagnostic)>, lock_map: &std::collections::HashMap<String, PathBuf>) {
        // canonicalize if possible
        let key = match fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => path.to_path_buf(),
        };

        // If the canonicalized path is a directory (e.g., from .art-lock), try to pick an entry file
        let actual_path = if key.is_dir() {
            let m1 = key.join("main.art");
            if m1.exists() {
                m1
            } else {
                let m2 = key.join("mod.art");
                if m2.exists() {
                    m2
                } else {
                    errors.push((String::new(), diagnostics::Diagnostic::new(diagnostics::DiagnosticKind::Parse, format!("Locked path {} is a directory but contains no main.art or mod.art", key.display()), diagnostics::Span::new(0,0,0,0))));
                    return;
                }
            }
        } else {
            key.clone()
        };

        // use canonical file key for visited tracking
        let file_key = match fs::canonicalize(&actual_path) {
            Ok(p) => p,
            Err(_) => actual_path.clone(),
        };
        if visited.contains(&file_key) {
            return;
        }
        visited.insert(file_key.clone());

        let source = match fs::read_to_string(&actual_path) {
            Ok(s) => s,
            Err(e) => {
                errors.push((String::new(), diagnostics::Diagnostic::new(diagnostics::DiagnosticKind::Parse, format!("Failed to read {}: {}", actual_path.display(), e), diagnostics::Span::new(0,0,0,0))));
                return;
            }
        };

        // Lex and parse
        let mut lexer = Lexer::new(source.clone());
        let tokens = match lexer.scan_tokens() {
            Ok(t) => t,
            Err(diag) => {
                errors.push((source.clone(), diag));
                return;
            }
        };
        let mut parser = Parser::new(tokens);
        let (program, diags) = parser.parse();
        if !diags.is_empty() {
            for d in diags {
                errors.push((source.clone(), d));
            }
            return;
        }

        // Process imports first, then append non-import statements
        // base directory for relative resolution
        let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
        for stmt in program.iter() {
            if let core::Stmt::Import { path } = stmt {
                // Convert tokens to path string segments and join with '/'
                let parts: Vec<String> = path.iter().map(|t| t.lexeme.clone()).collect();
                let rel = parts.join("/");
                // First, check project lock map for a pinned path
                if let Some(pinned) = lock_map.get(&rel) {
                    process_file(pinned, visited, out, errors, lock_map);
                    continue;
                }
                // candidate path relative to current file
                if let Some(cand) = resolve_candidate(base_dir, &rel) {
                    process_file(&cand, visited, out, errors, lock_map);
                } else {
                    errors.push((source.clone(), diagnostics::Diagnostic::new(diagnostics::DiagnosticKind::Parse, format!("Cannot resolve import '{}' from {}", rel, base_dir.display()), diagnostics::Span::new(0,0,0,0))));
                }
            }
        }
        for stmt in program {
            if let core::Stmt::Import { .. } = stmt {
                // already resolved
            } else {
                out.push(stmt);
            }
        }
    }

    // helper to join tokens parent dir with relative path string
    // pathbuf_join is now not needed; resolution uses resolve_candidate

    // Start processing from entry file path
    process_file(&entry_path, &mut visited, &mut out_program, &mut errors, &lock_map);

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok((out_program, main_source))
}
