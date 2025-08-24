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
        None
    }

    fn process_file(path: &Path, visited: &mut HashSet<PathBuf>, out: &mut core::Program, errors: &mut Vec<(String, diagnostics::Diagnostic)>) {
        // canonicalize if possible
        let key = match fs::canonicalize(path) {
            Ok(p) => p,
            Err(_) => path.to_path_buf(),
        };
        if visited.contains(&key) {
            return;
        }
        visited.insert(key.clone());

    let source = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                errors.push((String::new(), diagnostics::Diagnostic::new(diagnostics::DiagnosticKind::Parse, format!("Failed to read {}: {}", path.display(), e), diagnostics::Span::new(0,0,0,0))));
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
                // candidate path relative to current file
                if let Some(cand) = resolve_candidate(base_dir, &rel) {
                    process_file(&cand, visited, out, errors);
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
    process_file(&entry_path, &mut visited, &mut out_program, &mut errors);

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok((out_program, main_source))
}
