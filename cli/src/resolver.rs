use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use diagnostics::{Diagnostic, DiagnosticKind, Span};
use lexer::lexer::Lexer;
use parser::parser::Parser;
use rayon::prelude::*;

struct ResolveContext {
    visited: Mutex<HashSet<PathBuf>>,
    deps: Mutex<HashMap<PathBuf, Vec<PathBuf>>>,
    errors: Mutex<Vec<(String, diagnostics::Diagnostic)>>,
    lock_map: HashMap<String, PathBuf>,
}

fn push_error(ctx: &ResolveContext, source: String, message: String) {
    if let Ok(mut errors) = ctx.errors.lock() {
        errors.push((
            source,
            Diagnostic::new(DiagnosticKind::Parse, message, Span::new(0, 0, 0, 0)),
        ));
    }
}

fn read_lock_map(entry_path: &Path) -> HashMap<String, PathBuf> {
    let mut lock_map = HashMap::new();
    let parent = match entry_path.parent() {
        Some(p) => p,
        None => return lock_map,
    };
    let lock = parent.join(".art-lock");
    if !lock.exists() {
        return lock_map;
    }

    if let Ok(s) = std::fs::read_to_string(&lock) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
            let name = v.get("name").and_then(|n| n.as_str());
            let path = v.get("path").and_then(|p| p.as_str());
            if let (Some(name), Some(path)) = (name, path) {
                lock_map.insert(name.to_string(), PathBuf::from(path));
            }
        }
    }
    lock_map
}

fn resolve_candidate(base: &Path, rel: &str) -> Option<PathBuf> {
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

    let cand = base.join(rel);
    if cand.exists() {
        return Some(cand);
    }
    let mut cand_ext = cand.clone();
    cand_ext.set_extension("art");
    if cand_ext.exists() {
        return Some(cand_ext);
    }
    let cand_mod = cand.join("mod.art");
    if cand_mod.exists() {
        return Some(cand_mod);
    }

    if let Some(home) = dirs::home_dir() {
        let cache_dir = home.join(".artcode").join("cache");
        let cand_cache = cache_dir.join(rel);
        if cand_cache.exists() {
            if cand_cache.is_file() {
                return Some(cand_cache);
            }
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

        if let Ok(entries) = std::fs::read_dir(&cache_dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    let art_toml = p.join("Art.toml");
                    if art_toml.exists() {
                        if let Ok(s) = std::fs::read_to_string(&art_toml) {
                            if let Ok(v) = toml::from_str::<toml::Value>(&s) {
                                if let Some(name_v) = v.get("name").and_then(|n| n.as_str()) {
                                    if name_v == rel {
                                        if let Some(mainf) = v.get("main").and_then(|m| m.as_str())
                                        {
                                            let candidate_main = p.join(mainf);
                                            if candidate_main.exists() {
                                                return Some(candidate_main);
                                            }
                                        }
                                        let m1 = p.join("main.art");
                                        if m1.exists() {
                                            return Some(m1);
                                        }
                                        let m2 = p.join("mod.art");
                                        if m2.exists() {
                                            return Some(m2);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(fname) = p.file_name().and_then(|s| s.to_str()) {
                    if fname.starts_with(rel) {
                        let m1 = p.join("main.art");
                        if m1.exists() {
                            return Some(m1);
                        }
                        let m2 = p.join("mod.art");
                        if m2.exists() {
                            return Some(m2);
                        }
                    }
                }
            }
        }
    }
    None
}

fn normalize_module_path(path: &Path) -> Result<(PathBuf, PathBuf), String> {
    let key = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let actual_path = if key.is_dir() {
        let m1 = key.join("main.art");
        if m1.exists() {
            m1
        } else {
            let m2 = key.join("mod.art");
            if m2.exists() {
                m2
            } else {
                return Err(format!(
                    "Locked path {} is a directory but contains no main.art or mod.art",
                    key.display()
                ));
            }
        }
    } else {
        key.clone()
    };
    let file_key = fs::canonicalize(&actual_path).unwrap_or_else(|_| actual_path.clone());
    Ok((actual_path, file_key))
}

fn parse_program(
    path: &Path,
) -> Result<(String, core::Program), Vec<(String, diagnostics::Diagnostic)>> {
    let source = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            return Err(vec![(
                String::new(),
                Diagnostic::new(
                    DiagnosticKind::Parse,
                    format!("Failed to read {}: {}", path.display(), e),
                    Span::new(0, 0, 0, 0),
                ),
            )]);
        }
    };

    let mut lexer = Lexer::new(source.clone());
    let tokens = match lexer.scan_tokens() {
        Ok(t) => t,
        Err(diag) => return Err(vec![(source.clone(), diag)]),
    };

    let mut parser = Parser::new(tokens);
    let (program, diags) = parser.parse();
    if !diags.is_empty() {
        return Err(diags.into_iter().map(|d| (source.clone(), d)).collect());
    }

    Ok((source, program))
}

fn collect_module_graph(path: PathBuf, ctx: &Arc<ResolveContext>) {
    let (actual_path, key) = match normalize_module_path(&path) {
        Ok(v) => v,
        Err(msg) => {
            push_error(ctx, String::new(), msg);
            return;
        }
    };

    {
        let mut visited = match ctx.visited.lock() {
            Ok(v) => v,
            Err(_) => {
                push_error(
                    ctx,
                    String::new(),
                    "resolver visited mutex poisoned".to_string(),
                );
                return;
            }
        };
        if visited.contains(&key) {
            return;
        }
        visited.insert(key.clone());
    }

    let (source, program) = match parse_program(&actual_path) {
        Ok(v) => v,
        Err(diags) => {
            if let Ok(mut errors) = ctx.errors.lock() {
                errors.extend(diags);
            }
            return;
        }
    };

    let base_dir = actual_path.parent().unwrap_or_else(|| Path::new("."));
    let mut imports: Vec<PathBuf> = Vec::new();

    for stmt in &program {
        if let core::Stmt::Import { path } = stmt {
            let parts: Vec<String> = path.iter().map(|t| t.lexeme.clone()).collect();
            let rel = parts.join("/");

            let candidate = match ctx.lock_map.get(&rel) {
                Some(pinned) => Some(pinned.clone()),
                None => resolve_candidate(base_dir, &rel),
            };

            if let Some(cand) = candidate {
                match normalize_module_path(&cand) {
                    Ok((_actual_dep, dep_key)) => imports.push(dep_key),
                    Err(msg) => push_error(ctx, source.clone(), msg),
                }
            } else {
                push_error(
                    ctx,
                    source.clone(),
                    format!(
                        "Cannot resolve import '{}' from {}",
                        rel,
                        base_dir.display()
                    ),
                );
            }
        }
    }

    if let Ok(mut deps) = ctx.deps.lock() {
        deps.insert(key.clone(), imports.clone());
    }

    imports
        .into_par_iter()
        .for_each(|dep| collect_module_graph(dep, ctx));
}

fn emit_module(
    key: &PathBuf,
    deps: &HashMap<PathBuf, Vec<PathBuf>>,
    emitted: &mut HashSet<PathBuf>,
    out: &mut core::Program,
) -> Result<(), Vec<(String, diagnostics::Diagnostic)>> {
    if emitted.contains(key) {
        return Ok(());
    }

    if let Some(children) = deps.get(key) {
        for dep in children {
            emit_module(dep, deps, emitted, out)?;
        }
    }

    let (_source, program) = parse_program(key)?;
    for stmt in program {
        if let core::Stmt::Import { .. } = stmt {
            continue;
        }
        out.push(stmt);
    }
    emitted.insert(key.clone());
    Ok(())
}

/// Resolve imports starting from `entry` file. Returns Ok((program, main_source)) on success.
/// On error returns a vector of (source_string, Diagnostic) for diagnostics produced while
/// lexing/parsing any file.
pub fn resolve(
    entry: &str,
) -> Result<(core::Program, String), Vec<(String, diagnostics::Diagnostic)>> {
    let entry_path = PathBuf::from(entry);

    let main_source = match fs::read_to_string(&entry_path) {
        Ok(s) => s,
        Err(e) => {
            let diag = Diagnostic::new(
                DiagnosticKind::Parse,
                format!("Failed to read {}: {}", entry, e),
                Span::new(0, 0, 0, 0),
            );
            return Err(vec![(String::new(), diag)]);
        }
    };

    let ctx = Arc::new(ResolveContext {
        visited: Mutex::new(HashSet::new()),
        deps: Mutex::new(HashMap::new()),
        errors: Mutex::new(Vec::new()),
        lock_map: read_lock_map(&entry_path),
    });

    collect_module_graph(entry_path.clone(), &ctx);

    let errors = match ctx.errors.lock() {
        Ok(v) => v.clone(),
        Err(_) => vec![(
            String::new(),
            Diagnostic::new(
                DiagnosticKind::Parse,
                "resolver errors mutex poisoned".to_string(),
                Span::new(0, 0, 0, 0),
            ),
        )],
    };
    if !errors.is_empty() {
        return Err(errors);
    }

    let (_entry_actual, entry_key) = match normalize_module_path(&entry_path) {
        Ok(v) => v,
        Err(msg) => {
            return Err(vec![(
                String::new(),
                Diagnostic::new(DiagnosticKind::Parse, msg, Span::new(0, 0, 0, 0)),
            )]);
        }
    };

    let deps_map = match ctx.deps.lock() {
        Ok(v) => v.clone(),
        Err(_) => {
            return Err(vec![(
                String::new(),
                Diagnostic::new(
                    DiagnosticKind::Parse,
                    "resolver deps mutex poisoned".to_string(),
                    Span::new(0, 0, 0, 0),
                ),
            )]);
        }
    };

    let mut out_program: core::Program = Vec::new();
    let mut emitted: HashSet<PathBuf> = HashSet::new();
    emit_module(&entry_key, &deps_map, &mut emitted, &mut out_program)?;

    Ok((out_program, main_source))
}
