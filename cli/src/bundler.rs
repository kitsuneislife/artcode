use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use codegen_js::{CodegenJs, CodegenOptions, ModuleFormat};
use core::ast::Stmt;
use lexer::Lexer;
use parser::Parser;

// Minimal JS runtime mapping Artcode builtins to browser/Node-compatible equivalents.
pub const JS_RUNTIME: &str = r#"// Artcode JS runtime
const println = (...args) => console.log(...args);
const print   = (...args) => (typeof process !== 'undefined'
  ? process.stdout.write(args.join(''))
  : console.log(...args));
const str_split      = (s, sep) => s.split(sep);
const str_join       = (arr, sep) => arr.join(sep);
const str_contains   = (s, sub) => s.includes(sub);
const str_starts_with = (s, pre) => s.startsWith(pre);
const str_replace    = (s, from, to) => s.split(from).join(to);
const str_slice      = (s, start, end) => s.slice(start, end);
const str_to_int     = (s) => { const n = parseInt(s, 10); return isNaN(n) ? { tag: 'Err', payload: 'not an integer' } : { tag: 'Ok', payload: n }; };
const str_to_float   = (s) => { const n = parseFloat(s);   return isNaN(n) ? { tag: 'Err', payload: 'not a float'   } : { tag: 'Ok', payload: n }; };
const len            = (v) => v.length ?? 0;
const none           = null;
const some           = (v) => v;
"#;

fn resolve_import_path(base_dir: &Path, path_parts: &[String]) -> PathBuf {
    let mut p = base_dir.to_path_buf();
    for part in path_parts {
        p = p.join(part);
    }
    p.with_extension("art")
}

fn collect_imports(stmts: &[Stmt]) -> Vec<Vec<String>> {
    stmts
        .iter()
        .filter_map(|s| {
            if let Stmt::Import { path } = s {
                Some(path.iter().map(|t| t.lexeme.clone()).collect())
            } else {
                None
            }
        })
        .collect()
}

fn compile_file(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    ordered: &mut Vec<(PathBuf, String)>,
    errors: &mut Vec<String>,
) {
    let canon = match path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            errors.push(format!("cannot resolve '{}': {}", path.display(), e));
            return;
        }
    };

    if visited.contains(&canon) {
        return;
    }
    visited.insert(canon.clone());

    let src = match std::fs::read_to_string(&canon) {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("cannot read '{}': {}", canon.display(), e));
            return;
        }
    };

    let tokens = match Lexer::new(src.clone()).scan_tokens() {
        Ok(t) => t,
        Err(d) => {
            errors.push(format!("lex error in '{}': {}", canon.display(), d.message));
            return;
        }
    };

    let (program, diags) = Parser::new(tokens).parse();
    if !diags.is_empty() {
        for d in &diags {
            errors.push(format!("parse error in '{}': {}", canon.display(), d.message));
        }
        return;
    }

    let base_dir = canon.parent().unwrap_or(Path::new("."));
    for import_parts in collect_imports(&program) {
        let dep_path = resolve_import_path(base_dir, &import_parts);
        compile_file(&dep_path, visited, ordered, errors);
    }

    // Emit JS without import statements (they are inlined)
    let opts = CodegenOptions {
        source_file: Some(canon.to_string_lossy().to_string()),
        emit_source_map: false,
        module_format: ModuleFormat::Bundle,
    };
    let output = CodegenJs::new(opts).emit_program(&program);
    ordered.push((canon, output.code));
}

pub struct BundleOutput {
    pub code: String,
}

pub fn bundle(entry: &str, emit_sourcemap: bool) -> Result<BundleOutput, Vec<String>> {
    let entry_path = Path::new(entry);
    let entry_canon = match entry_path.canonicalize() {
        Ok(p) => p,
        Err(e) => return Err(vec![format!("cannot resolve '{}': {}", entry, e)]),
    };

    let _ = emit_sourcemap; // source map across bundled files is v0.5 scope

    let mut visited = HashSet::new();
    let mut ordered: Vec<(PathBuf, String)> = Vec::new();
    let mut errors = Vec::new();

    // Pre-populate visited with entry so deps load first
    let base_dir = entry_canon.parent().unwrap_or(Path::new("."));
    let src = match std::fs::read_to_string(&entry_canon) {
        Ok(s) => s,
        Err(e) => return Err(vec![format!("cannot read '{}': {}", entry, e)]),
    };

    let tokens = match Lexer::new(src.clone()).scan_tokens() {
        Ok(t) => t,
        Err(d) => return Err(vec![format!("lex error: {}", d.message)]),
    };

    let (program, diags) = Parser::new(tokens).parse();
    if !diags.is_empty() {
        return Err(diags.iter().map(|d| d.message.clone()).collect());
    }

    // Load dependencies first
    for import_parts in collect_imports(&program) {
        let dep_path = resolve_import_path(base_dir, &import_parts);
        compile_file(&dep_path, &mut visited, &mut ordered, &mut errors);
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    // Emit entry file (Bundle mode skips import stmts)
    let opts = CodegenOptions {
        source_file: Some(entry_canon.to_string_lossy().to_string()),
        emit_source_map: false,
        module_format: ModuleFormat::Bundle,
    };
    let entry_js = CodegenJs::new(opts).emit_program(&program);

    let mut code = String::new();
    code.push_str(JS_RUNTIME);
    code.push('\n');

    let mut seen_modules: HashMap<PathBuf, bool> = HashMap::new();
    for (path, module_code) in &ordered {
        if seen_modules.insert(path.clone(), true).is_none() {
            code.push_str(&format!("// --- module: {} ---\n", path.display()));
            code.push_str(module_code);
            code.push('\n');
        }
    }

    code.push_str("// --- entry ---\n");
    code.push_str(&entry_js.code);

    Ok(BundleOutput { code })
}
