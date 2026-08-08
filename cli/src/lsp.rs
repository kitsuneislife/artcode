use core::TokenType;
use diagnostics::{Diagnostic, DiagnosticKind};
use interpreter::type_infer::{TypeEnv, TypeInfer};
use lexer::lexer::Lexer;
use parser::parser::Parser;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};

const TOKEN_TYPES: [&str; 6] = [
    "keyword", "variable", "function", "string", "number", "operator",
];

const BUILTIN_NAMES: &[&str] = &[
    "println",
    "len",
    "type_of",
    "map_new",
    "map_set",
    "map_get",
    "map_has",
    "set_new",
    "set_add",
    "set_has",
    "math_abs",
    "math_pow",
    "math_clamp",
    "dag_topo_sort",
    "time_now",
    "io_read_text",
    "io_write_text",
    "http_get_text",
    "random_seed",
    "random_next",
    "gc_stats",
    "runtime_version",
    "str_split",
    "str_join",
    "str_contains",
    "str_starts_with",
    "str_replace",
    "str_slice",
    "str_to_int",
    "str_to_float",
    "buffer_new",
    "serialize",
    "deserialize",
    "capability_acquire",
    "capability_kind",
    "arena_new",
    "arena_release",
    "arena_with",
    "atomic_new",
    "atomic_load",
    "atomic_store",
    "atomic_add",
    "mutex_new",
    "mutex_lock",
    "mutex_unlock",
    "actor_send",
    "actor_receive",
    "actor_receive_envelope",
    "actor_yield",
    "actor_set_mailbox_limit",
    "run_actors",
    "envelope",
    "make_envelope",
];

const KEYWORDS: &[&str] = &[
    "let",
    "if",
    "else",
    "true",
    "false",
    "struct",
    "enum",
    "and",
    "or",
    "match",
    "case",
    "import",
    "func",
    "performant",
    "spawn",
    "actor",
    "return",
    "while",
    "for",
    "in",
    "try",
    "catch",
    "weak",
    "unowned",
    "none",
    "as",
];

#[derive(Clone, Debug)]
struct SymbolDecl {
    line: usize,
    start_char: usize,
    end_char: usize,
}

#[derive(Clone, Debug)]
struct SymbolLoc {
    uri: String,
    decl: SymbolDecl,
}

fn is_identifier_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    let first = match chars.next() {
        Some(c) => c,
        None => return false,
    };
    if !(first.is_alphabetic() || first == '_') {
        return false;
    }
    chars.all(is_identifier_char)
}

fn is_keyword_name(name: &str) -> bool {
    KEYWORDS.contains(&name)
}

fn line_chars(text: &str, line: usize) -> Option<Vec<char>> {
    text.lines().nth(line).map(|s| s.chars().collect())
}

fn word_at_position(text: &str, line: usize, character: usize) -> Option<String> {
    let chars = line_chars(text, line)?;
    if chars.is_empty() {
        return None;
    }
    let idx = character.min(chars.len().saturating_sub(1));
    if !is_identifier_char(chars[idx]) {
        return None;
    }
    let mut start = idx;
    let mut end = idx;
    while start > 0 && is_identifier_char(chars[start - 1]) {
        start -= 1;
    }
    while end + 1 < chars.len() && is_identifier_char(chars[end + 1]) {
        end += 1;
    }
    Some(chars[start..=end].iter().collect())
}

fn lsp_position_from_offset(text: &str, offset: usize) -> (usize, usize) {
    let mut line = 0usize;
    let mut col = 0usize;
    for (i, c) in text.chars().enumerate() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn collect_declarations(text: &str) -> HashMap<String, SymbolDecl> {
    let mut map = HashMap::new();
    let mut lexer = Lexer::new(text.to_string());
    let tokens = match lexer.scan_tokens() {
        Ok(t) => t,
        Err(_) => return map,
    };

    let mut i = 0usize;
    while i < tokens.len() {
        let tok = &tokens[i];
        match tok.token_type {
            TokenType::Let if i + 1 < tokens.len() => {
                let id = &tokens[i + 1];
                if matches!(id.token_type, TokenType::Identifier) {
                    map.entry(id.lexeme.clone()).or_insert(SymbolDecl {
                        line: id.line.saturating_sub(1),
                        start_char: id.col.saturating_sub(1),
                        end_char: id.col.saturating_sub(1) + id.lexeme.chars().count(),
                    });
                }
            }
            TokenType::Func => {
                // function name
                if i + 1 < tokens.len() {
                    let f = &tokens[i + 1];
                    if matches!(f.token_type, TokenType::Identifier) {
                        map.entry(f.lexeme.clone()).or_insert(SymbolDecl {
                            line: f.line.saturating_sub(1),
                            start_char: f.col.saturating_sub(1),
                            end_char: f.col.saturating_sub(1) + f.lexeme.chars().count(),
                        });
                    }
                }
                // parameters
                let mut j = i + 1;
                while j < tokens.len() && !matches!(tokens[j].token_type, TokenType::LeftParen) {
                    j += 1;
                }
                if j < tokens.len() {
                    j += 1;
                    while j < tokens.len() && !matches!(tokens[j].token_type, TokenType::RightParen)
                    {
                        if matches!(tokens[j].token_type, TokenType::Identifier) {
                            let p = &tokens[j];
                            map.entry(p.lexeme.clone()).or_insert(SymbolDecl {
                                line: p.line.saturating_sub(1),
                                start_char: p.col.saturating_sub(1),
                                end_char: p.col.saturating_sub(1) + p.lexeme.chars().count(),
                            });
                        }
                        j += 1;
                    }
                }
            }
            TokenType::For if i + 1 < tokens.len() => {
                let id = &tokens[i + 1];
                if matches!(id.token_type, TokenType::Identifier) {
                    map.entry(id.lexeme.clone()).or_insert(SymbolDecl {
                        line: id.line.saturating_sub(1),
                        start_char: id.col.saturating_sub(1),
                        end_char: id.col.saturating_sub(1) + id.lexeme.chars().count(),
                    });
                }
            }
            TokenType::Struct | TokenType::Enum if i + 1 < tokens.len() => {
                let id = &tokens[i + 1];
                if matches!(id.token_type, TokenType::Identifier) {
                    map.entry(id.lexeme.clone()).or_insert(SymbolDecl {
                        line: id.line.saturating_sub(1),
                        start_char: id.col.saturating_sub(1),
                        end_char: id.col.saturating_sub(1) + id.lexeme.chars().count(),
                    });
                }
            }
            _ => {}
        }
        i += 1;
    }
    map
}

#[derive(Clone, Debug)]
struct HoverInfo {
    kind: HoverKind,
    #[allow(dead_code)]
    name: String,
    detail: String,
}

#[derive(Clone, Debug)]
enum HoverKind {
    Variable,
    Function,
    Struct,
    Enum,
    Builtin,
}

fn collect_hover_info(text: &str) -> HashMap<String, HoverInfo> {
    let mut map: HashMap<String, HoverInfo> = HashMap::new();
    let mut lexer = Lexer::new(text.to_string());
    let tokens = match lexer.scan_tokens() {
        Ok(t) => t,
        Err(_) => return map,
    };

    for name in BUILTIN_NAMES {
        map.insert(
            name.to_string(),
            HoverInfo {
                kind: HoverKind::Builtin,
                name: name.to_string(),
                detail: format!("builtin function `{}`", name),
            },
        );
    }

    let mut i = 0usize;
    while i < tokens.len() {
        match &tokens[i].token_type {
            TokenType::Let
                // let name [: type] = expr
                if i + 1 < tokens.len() && matches!(tokens[i+1].token_type, TokenType::Identifier) => {
                    let name = tokens[i+1].lexeme.clone();
                    // Look for optional type annotation
                    let mut detail = format!("let {}", name);
                    if i + 2 < tokens.len() && matches!(tokens[i+2].token_type, TokenType::Colon)
                        && i + 3 < tokens.len() && matches!(tokens[i+3].token_type, TokenType::Identifier) {
                            detail = format!("let {}: {}", name, tokens[i+3].lexeme);
                        }
                    map.entry(name.clone()).or_insert(HoverInfo {
                        kind: HoverKind::Variable,
                        name,
                        detail,
                    });
                }
            TokenType::Func
                if i + 1 < tokens.len() && matches!(tokens[i+1].token_type, TokenType::Identifier) => {
                    let fname = tokens[i+1].lexeme.clone();
                    // Collect params
                    let mut j = i + 2;
                    while j < tokens.len() && !matches!(tokens[j].token_type, TokenType::LeftParen) {
                        j += 1;
                    }
                    let mut params = Vec::new();
                    if j < tokens.len() {
                        j += 1;
                        while j < tokens.len() && !matches!(tokens[j].token_type, TokenType::RightParen) {
                            if matches!(tokens[j].token_type, TokenType::Identifier) {
                                let pname = tokens[j].lexeme.clone();
                                if j + 1 < tokens.len() && matches!(tokens[j+1].token_type, TokenType::Colon)
                                    && j + 2 < tokens.len() && matches!(tokens[j+2].token_type, TokenType::Identifier) {
                                    params.push(format!("{}: {}", pname, tokens[j+2].lexeme));
                                } else {
                                    params.push(pname);
                                }
                            }
                            j += 1;
                        }
                    }
                    // Look for return type annotation: -> Type
                    let mut ret = String::new();
                    let mut k = j + 1;
                    while k < tokens.len() && !matches!(tokens[k].token_type, TokenType::LeftBrace) {
                        if matches!(tokens[k].token_type, TokenType::Arrow) && k + 1 < tokens.len() {
                            ret = format!(" -> {}", tokens[k+1].lexeme);
                        }
                        k += 1;
                    }
                    let detail = format!("func {}({}){}", fname, params.join(", "), ret);
                    map.entry(fname.clone()).or_insert(HoverInfo {
                        kind: HoverKind::Function,
                        name: fname,
                        detail,
                    });
                }
            TokenType::Struct
                if i + 1 < tokens.len() && matches!(tokens[i+1].token_type, TokenType::Identifier) => {
                    let sname = tokens[i+1].lexeme.clone();
                    // Collect fields
                    let mut j = i + 2;
                    while j < tokens.len() && !matches!(tokens[j].token_type, TokenType::LeftBrace) {
                        j += 1;
                    }
                    let mut fields = Vec::new();
                    if j < tokens.len() {
                        j += 1;
                        while j < tokens.len() && !matches!(tokens[j].token_type, TokenType::RightBrace) {
                            if matches!(tokens[j].token_type, TokenType::Identifier) {
                                let fname = tokens[j].lexeme.clone();
                                if j + 1 < tokens.len() && matches!(tokens[j+1].token_type, TokenType::Colon)
                                    && j + 2 < tokens.len() && matches!(tokens[j+2].token_type, TokenType::Identifier) {
                                    fields.push(format!("{}: {}", fname, tokens[j+2].lexeme));
                                } else {
                                    fields.push(fname);
                                }
                            }
                            j += 1;
                        }
                    }
                    let detail = format!("struct {} {{ {} }}", sname, fields.join(", "));
                    map.entry(sname.clone()).or_insert(HoverInfo {
                        kind: HoverKind::Struct,
                        name: sname,
                        detail,
                    });
                }
            TokenType::Enum
                if i + 1 < tokens.len() && matches!(tokens[i+1].token_type, TokenType::Identifier) => {
                    let ename = tokens[i+1].lexeme.clone();
                    let detail = format!("enum {}", ename);
                    map.entry(ename.clone()).or_insert(HoverInfo {
                        kind: HoverKind::Enum,
                        name: ename,
                        detail,
                    });
                }
            _ => {}
        }
        i += 1;
    }
    map
}

fn collect_workspace_declarations(
    documents: &HashMap<String, String>,
) -> HashMap<String, SymbolLoc> {
    let all_docs = collect_project_documents(documents);
    let mut out = HashMap::new();
    let mut uris: Vec<&String> = all_docs.keys().collect();
    uris.sort();

    for uri in uris {
        if let Some(text) = all_docs.get(uri) {
            let defs = collect_declarations(text);
            let mut names: Vec<_> = defs.into_iter().collect();
            names.sort_by(|a, b| a.0.cmp(&b.0));
            for (name, decl) in names {
                out.entry(name).or_insert(SymbolLoc {
                    uri: uri.clone(),
                    decl,
                });
            }
        }
    }
    out
}

/// Percent-decodes every `%XX` escape in a file URI path component.
///
/// Editors escape more than the three characters the previous hand-rolled
/// version handled — notably VS Code sends Windows drive letters as `c%3A`.
fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(byte) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn decode_file_uri_path(uri: &str) -> Option<PathBuf> {
    let raw = uri.strip_prefix("file://")?;
    let decoded = percent_decode(raw);

    // On Windows a file URI is `file:///C:/dir/file.art`, so after stripping
    // `file://` the remainder is `/C:/dir/file.art` — the leading slash is part
    // of the URI grammar, not of the path, and must go before `PathBuf` sees it.
    #[cfg(windows)]
    let decoded = {
        let trimmed = decoded.strip_prefix('/').unwrap_or(&decoded);
        let bytes = trimmed.as_bytes();
        let is_drive_path = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
        let path = if is_drive_path { trimmed } else { &decoded };
        path.replace('/', "\\")
    };

    Some(PathBuf::from(decoded))
}

fn to_file_uri(path: &Path) -> Option<String> {
    let canonical = std::fs::canonicalize(path).ok()?;
    let text = canonical.to_string_lossy().into_owned();

    // `canonicalize` returns extended-length paths (`\\?\C:\dir`) on Windows.
    // That prefix is a Win32 API detail and is not valid inside a file URI.
    // Rebinding rather than mutating keeps the binding immutable on Unix, where
    // this block does not exist and a `mut` would trip `-D unused-mut`.
    #[cfg(windows)]
    let text = text
        .strip_prefix(r"\\?\")
        .unwrap_or(text.as_str())
        .replace('\\', "/");

    // Unix paths already start with `/`; Windows paths start with a drive
    // letter and need the extra slash that separates authority from path.
    if text.starts_with('/') {
        Some(format!("file://{}", text))
    } else {
        Some(format!("file:///{}", text))
    }
}

fn parse_import_paths(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut lexer = Lexer::new(text.to_string());
    let tokens = match lexer.scan_tokens() {
        Ok(t) => t,
        Err(_) => return out,
    };

    let mut i = 0usize;
    while i < tokens.len() {
        if !matches!(tokens[i].token_type, TokenType::Import) {
            i += 1;
            continue;
        }

        i += 1;
        let mut parts: Vec<String> = Vec::new();
        while i < tokens.len() {
            match tokens[i].token_type {
                TokenType::Identifier => {
                    parts.push(tokens[i].lexeme.clone());
                    i += 1;
                    if i < tokens.len() && matches!(tokens[i].token_type, TokenType::Dot) {
                        i += 1;
                    }
                }
                TokenType::Semicolon => {
                    i += 1;
                    break;
                }
                _ => {
                    i += 1;
                }
            }
        }

        if !parts.is_empty() {
            out.push(parts.join("/"));
        }
    }

    out
}

fn resolve_import_candidate(base_file: &Path, module: &str) -> Option<PathBuf> {
    let base_dir = base_file.parent().unwrap_or_else(|| Path::new("."));
    let rel = PathBuf::from(module);

    let direct = base_dir.join(&rel);
    if direct.exists() {
        if direct.is_file() {
            return Some(direct);
        }
        let mod_art = direct.join("mod.art");
        if mod_art.exists() {
            return Some(mod_art);
        }
        let main_art = direct.join("main.art");
        if main_art.exists() {
            return Some(main_art);
        }
    }

    let mut with_ext = base_dir.join(&rel);
    with_ext.set_extension("art");
    if with_ext.exists() {
        return Some(with_ext);
    }

    None
}

fn collect_project_documents(documents: &HashMap<String, String>) -> HashMap<String, String> {
    let mut out = documents.clone();
    let mut visited = HashSet::new();

    let mut uris: Vec<String> = documents.keys().cloned().collect();
    uris.sort();
    for uri in uris {
        collect_import_graph_from_uri(&uri, documents, &mut out, &mut visited);
    }

    out
}

fn collect_import_graph_from_uri(
    uri: &str,
    open_documents: &HashMap<String, String>,
    all_documents: &mut HashMap<String, String>,
    visited: &mut HashSet<PathBuf>,
) {
    let path = match decode_file_uri_path(uri) {
        Some(p) => p,
        None => return,
    };
    let canon = std::fs::canonicalize(&path).unwrap_or(path.clone());
    if !visited.insert(canon.clone()) {
        return;
    }

    let current_text = open_documents
        .get(uri)
        .cloned()
        .or_else(|| std::fs::read_to_string(&canon).ok());
    let current_text = match current_text {
        Some(t) => t,
        None => return,
    };
    all_documents
        .entry(uri.to_string())
        .or_insert(current_text.clone());

    for module in parse_import_paths(&current_text) {
        let import_path = match resolve_import_candidate(&canon, &module) {
            Some(p) => p,
            None => continue,
        };

        let import_uri = match to_file_uri(&import_path) {
            Some(u) => u,
            None => continue,
        };

        if !all_documents.contains_key(&import_uri) {
            if let Some(open_text) = open_documents.get(&import_uri) {
                all_documents.insert(import_uri.clone(), open_text.clone());
            } else if let Ok(text) = std::fs::read_to_string(&import_path) {
                all_documents.insert(import_uri.clone(), text);
            }
        }

        collect_import_graph_from_uri(&import_uri, open_documents, all_documents, visited);
    }
}

fn resolve_definition_location(
    documents: &HashMap<String, String>,
    uri: &str,
    line: usize,
    character: usize,
) -> Option<(String, SymbolDecl)> {
    let text = documents.get(uri)?;
    let word = word_at_position(text, line, character)?;

    let local_defs = collect_declarations(text);
    if let Some(d) = local_defs.get(&word) {
        return Some((uri.to_string(), d.clone()));
    }

    let defs = collect_workspace_declarations(documents);
    defs.get(&word)
        .map(|loc| (loc.uri.clone(), loc.decl.clone()))
}

fn find_identifier_occurrences(text: &str, name: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let needle: Vec<char> = name.chars().collect();
    if needle.is_empty() || chars.len() < needle.len() {
        return out;
    }

    let n = needle.len();
    let mut i = 0usize;
    while i + n <= chars.len() {
        if chars[i..i + n] == needle[..] {
            let left_ok = i == 0 || !is_identifier_char(chars[i - 1]);
            let right_ok = i + n == chars.len() || !is_identifier_char(chars[i + n]);
            if left_ok && right_ok {
                out.push((i, i + n));
            }
            i += n;
        } else {
            i += 1;
        }
    }
    out
}

fn workspace_rename_edits(
    documents: &HashMap<String, String>,
    uri: &str,
    line: usize,
    character: usize,
    new_name: &str,
) -> Option<Value> {
    if !is_valid_identifier(new_name) || is_keyword_name(new_name) {
        return None;
    }

    let all_docs = collect_project_documents(documents);
    let current_text = all_docs.get(uri)?;
    let old_name = word_at_position(current_text, line, character)?;
    if is_keyword_name(&old_name) {
        return None;
    }

    let defs = collect_workspace_declarations(documents);
    if !defs.contains_key(&old_name) {
        return None;
    }

    let mut changes = serde_json::Map::new();
    let mut uris: Vec<&String> = all_docs.keys().collect();
    uris.sort();
    for doc_uri in uris {
        if let Some(text) = all_docs.get(doc_uri) {
            let edits: Vec<Value> = find_identifier_occurrences(text, &old_name)
                .into_iter()
                .map(|(start, end)| {
                    let (sl, sc) = lsp_position_from_offset(text, start);
                    let (el, ec) = lsp_position_from_offset(text, end);
                    serde_json::json!({
                        "range": {
                            "start": { "line": sl, "character": sc },
                            "end": { "line": el, "character": ec }
                        },
                        "newText": new_name
                    })
                })
                .collect();

            if !edits.is_empty() {
                changes.insert(doc_uri.clone(), Value::Array(edits));
            }
        }
    }

    if changes.is_empty() {
        return None;
    }

    Some(serde_json::json!({ "changes": changes }))
}

fn token_class(t: &TokenType, prev: Option<&TokenType>) -> Option<usize> {
    let is_keyword = matches!(
        t,
        TokenType::Let
            | TokenType::If
            | TokenType::Else
            | TokenType::True
            | TokenType::False
            | TokenType::Struct
            | TokenType::Enum
            | TokenType::And
            | TokenType::Or
            | TokenType::Match
            | TokenType::Case
            | TokenType::Import
            | TokenType::Func
            | TokenType::Performant
            | TokenType::Spawn
            | TokenType::Actor
            | TokenType::Return
            | TokenType::While
            | TokenType::For
            | TokenType::In
            | TokenType::Try
            | TokenType::Catch
            | TokenType::Weak
            | TokenType::Unowned
            | TokenType::None
            | TokenType::As
    );
    if is_keyword {
        return Some(0);
    }
    match t {
        TokenType::Identifier => {
            if matches!(prev, Some(TokenType::Func)) {
                Some(2)
            } else {
                Some(1)
            }
        }
        TokenType::String(_) | TokenType::InterpolatedString(_) => Some(3),
        TokenType::Number(_) => Some(4),
        TokenType::Plus
        | TokenType::Minus
        | TokenType::Star
        | TokenType::Slash
        | TokenType::Equal
        | TokenType::EqualEqual
        | TokenType::Bang
        | TokenType::BangEqual
        | TokenType::Greater
        | TokenType::GreaterEqual
        | TokenType::Less
        | TokenType::LessEqual
        | TokenType::And
        | TokenType::Or => Some(5),
        _ => None,
    }
}

fn completion_items(text: &str) -> Vec<Value> {
    let mut items: Vec<Value> = KEYWORDS
        .iter()
        .map(|k| serde_json::json!({"label": *k, "kind": 14}))
        .collect();

    // Builtins with function kind (3)
    for b in BUILTIN_NAMES {
        items.push(serde_json::json!({"label": *b, "kind": 3}));
    }

    let mut seen: HashSet<String> = BUILTIN_NAMES.iter().map(|s| s.to_string()).collect();
    let mut lexer = Lexer::new(text.to_string());
    if let Ok(tokens) = lexer.scan_tokens() {
        for t in tokens {
            if matches!(t.token_type, TokenType::Identifier) && seen.insert(t.lexeme.clone()) {
                items.push(serde_json::json!({"label": t.lexeme, "kind": 6}));
            }
        }
    }
    items
}

fn workspace_completion_items(documents: &HashMap<String, String>) -> Vec<Value> {
    let all_docs = collect_project_documents(documents);

    // Seed with keywords (kind 14), builtins (kind 3)
    let mut seen: HashMap<String, u32> = HashMap::new();
    for kw in KEYWORDS {
        seen.insert(kw.to_string(), 14);
    }
    for b in BUILTIN_NAMES {
        seen.insert(b.to_string(), 3);
    }

    // Collect identifier kinds from all workspace docs
    let hover_by_doc: Vec<HashMap<String, HoverInfo>> = {
        let mut uris: Vec<&String> = all_docs.keys().collect();
        uris.sort();
        uris.iter()
            .filter_map(|uri| all_docs.get(*uri).map(|t| collect_hover_info(t)))
            .collect()
    };

    for info_map in &hover_by_doc {
        for (name, info) in info_map {
            if seen.contains_key(name) {
                continue;
            }
            let kind = match info.kind {
                HoverKind::Function => 3,
                HoverKind::Struct => 7,
                HoverKind::Enum => 13,
                HoverKind::Builtin => 3,
                HoverKind::Variable => 6,
            };
            seen.insert(name.clone(), kind);
        }
    }

    // Also sweep raw identifiers that weren't captured by hover
    let mut uris: Vec<&String> = all_docs.keys().collect();
    uris.sort();
    for uri in uris {
        if let Some(text) = all_docs.get(uri) {
            let mut lexer = Lexer::new(text.to_string());
            if let Ok(tokens) = lexer.scan_tokens() {
                for t in tokens {
                    if matches!(t.token_type, TokenType::Identifier) {
                        seen.entry(t.lexeme).or_insert(6);
                    }
                }
            }
        }
    }

    let mut names: Vec<(String, u32)> = seen.into_iter().collect();
    names.sort_by(|a, b| a.0.cmp(&b.0));
    names
        .into_iter()
        .map(|(label, kind)| serde_json::json!({"label": label, "kind": kind}))
        .collect()
}

fn semantic_tokens_data(text: &str) -> Vec<usize> {
    let mut data = Vec::new();
    let mut lexer = Lexer::new(text.to_string());
    let tokens = match lexer.scan_tokens() {
        Ok(t) => t,
        Err(_) => return data,
    };

    let mut prev_line = 0usize;
    let mut prev_char = 0usize;
    let mut prev_ty: Option<TokenType> = None;
    for tok in tokens {
        let line = tok.line.saturating_sub(1);
        let ch = tok.col.saturating_sub(1);
        let len = tok.lexeme.chars().count();
        if len == 0 {
            prev_ty = Some(tok.token_type);
            continue;
        }
        if let Some(kind) = token_class(&tok.token_type, prev_ty.as_ref()) {
            let delta_line = line.saturating_sub(prev_line);
            let delta_start = if delta_line == 0 {
                ch.saturating_sub(prev_char)
            } else {
                ch
            };
            data.push(delta_line);
            data.push(delta_start);
            data.push(len);
            data.push(kind);
            data.push(0); // token modifiers bitset
            prev_line = line;
            prev_char = ch;
        }
        prev_ty = Some(tok.token_type);
    }
    data
}

pub fn start_server() {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut stdout = io::stdout();
    let mut documents: std::collections::HashMap<String, String> = std::collections::HashMap::new();

    // Standard JSON-RPC 2.0 loop over stdin
    loop {
        let mut content_length: Option<usize> = None;
        let mut buffer = String::new();

        // Read HTTP-like headers
        loop {
            buffer.clear();
            if reader.read_line(&mut buffer).unwrap_or(0) == 0 {
                return; // EOF
            }
            if buffer == "\r\n" || buffer == "\n" {
                break;
            }
            if buffer.to_lowercase().starts_with("content-length:") {
                let parts: Vec<&str> = buffer.split(':').collect();
                if parts.len() == 2
                    && let Ok(len) = parts[1].trim().parse::<usize>()
                {
                    content_length = Some(len);
                }
            }
        }

        let len = match content_length {
            Some(l) => l,
            None => continue,
        };

        let mut body = vec![0; len];
        if reader.read_exact(&mut body).is_err() {
            return;
        }

        let body_str = String::from_utf8_lossy(&body);

        // Parse JSON-RPC Payload
        if let Ok(parsed) = serde_json::from_str::<Value>(&body_str) {
            handle_message(&parsed, &mut stdout, &mut documents);
        }
    }
}

fn hover_markdown(
    documents: &HashMap<String, String>,
    uri: &str,
    line: usize,
    character: usize,
) -> String {
    let text = match documents.get(uri) {
        Some(t) => t,
        None => return "**Artcode**".to_string(),
    };
    let word = match word_at_position(text, line, character) {
        Some(w) => w,
        None => return "**Artcode**".to_string(),
    };

    // Collect hover info from current doc and workspace
    let all_docs = collect_project_documents(documents);
    let mut info_map: HashMap<String, HoverInfo> = HashMap::new();
    let mut uris: Vec<&String> = all_docs.keys().collect();
    uris.sort();
    for u in uris {
        if let Some(t) = all_docs.get(u) {
            for (k, v) in collect_hover_info(t) {
                info_map.entry(k).or_insert(v);
            }
        }
    }

    if let Some(info) = info_map.get(&word) {
        let kind_label = match info.kind {
            HoverKind::Function => "function",
            HoverKind::Struct => "struct",
            HoverKind::Enum => "enum",
            HoverKind::Builtin => "builtin",
            HoverKind::Variable => "variable",
        };
        format!("**{}** `{}`\n\n```\n{}\n```", kind_label, word, info.detail)
    } else {
        format!("**`{}`**", word)
    }
}

fn process_request(req: &Value, documents: &mut HashMap<String, String>) -> Option<Value> {
    let id = req.get("id")?;
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

    let result: Value = match method {
        "initialize" => serde_json::json!({
            "capabilities": {
                "textDocumentSync": 1,
                "hoverProvider": true,
                "definitionProvider": true,
                "completionProvider": {
                    "resolveProvider": false,
                    "triggerCharacters": [".", "_"]
                },
                "renameProvider": true,
                "semanticTokensProvider": {
                    "legend": {
                        "tokenTypes": TOKEN_TYPES,
                        "tokenModifiers": []
                    },
                    "full": true
                }
            },
            "serverInfo": { "name": "art-lsp", "version": "0.1.0" }
        }),

        "textDocument/hover" => {
            let params = req.get("params")?;
            let uri = params.get("textDocument")?.get("uri")?.as_str()?;
            let pos = params.get("position")?;
            let line = pos.get("line")?.as_u64()? as usize;
            let character = pos.get("character")?.as_u64()? as usize;
            let md = hover_markdown(documents, uri, line, character);
            serde_json::json!({ "contents": { "kind": "markdown", "value": md } })
        }

        "textDocument/definition" => {
            let params = req.get("params")?;
            let uri = params.get("textDocument")?.get("uri")?.as_str()?;
            let pos = params.get("position")?;
            let line = pos.get("line")?.as_u64()? as usize;
            let character = pos.get("character")?.as_u64()? as usize;
            let (decl_uri, d) = resolve_definition_location(documents, uri, line, character)?;
            serde_json::json!({
                "uri": decl_uri,
                "range": {
                    "start": { "line": d.line, "character": d.start_char },
                    "end": { "line": d.line, "character": d.end_char }
                }
            })
        }

        "textDocument/completion" => {
            let uri = req
                .get("params")
                .and_then(|p| p.get("textDocument"))
                .and_then(|d| d.get("uri"))
                .and_then(|u| u.as_str());
            let items = if uri.map(|u| documents.contains_key(u)).unwrap_or(false) {
                workspace_completion_items(documents)
            } else {
                completion_items("")
            };
            serde_json::json!({ "isIncomplete": false, "items": items })
        }

        "textDocument/rename" => {
            let params = req.get("params")?;
            let uri = params.get("textDocument")?.get("uri")?.as_str()?;
            let pos = params.get("position")?;
            let line = pos.get("line")?.as_u64()? as usize;
            let character = pos.get("character")?.as_u64()? as usize;
            let new_name = params.get("newName")?.as_str()?;
            workspace_rename_edits(documents, uri, line, character, new_name)?
        }

        "textDocument/semanticTokens/full" => {
            let uri = req
                .get("params")
                .and_then(|p| p.get("textDocument"))
                .and_then(|d| d.get("uri"))
                .and_then(|u| u.as_str());
            let data = uri
                .and_then(|u| documents.get(u))
                .map(|t| semantic_tokens_data(t))
                .unwrap_or_default();
            serde_json::json!({ "data": data })
        }

        "shutdown" => Value::Null,

        _ => return None,
    };

    Some(serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    }))
}

fn handle_message(
    req: &Value,
    stdout: &mut io::Stdout,
    documents: &mut std::collections::HashMap<String, String>,
) {
    let id = req.get("id");
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

    // Handle stateful notifications first (no id required)
    match method {
        "initialized" => {
            // Client is ready, nothing to reply to (Notification)
        }
        "textDocument/didOpen" => {
            if let Some(doc) = req.get("params").and_then(|p| p.get("textDocument"))
                && let (Some(uri), Some(text)) = (
                    doc.get("uri").and_then(|u| u.as_str()),
                    doc.get("text").and_then(|t| t.as_str()),
                )
            {
                documents.insert(uri.to_string(), text.to_string());
                publish_diagnostics(uri, text, stdout);
            }
        }
        "textDocument/didChange" => {
            if let Some(params) = req.get("params")
                && let Some(uri) = params
                    .get("textDocument")
                    .and_then(|d| d.get("uri").and_then(|u| u.as_str()))
                && let Some(changes) = params.get("contentChanges").and_then(|c| c.as_array())
                && let Some(change) = changes.last()
                && let Some(text) = change.get("text").and_then(|t| t.as_str())
            {
                documents.insert(uri.to_string(), text.to_string());
                publish_diagnostics(uri, text, stdout);
            }
        }
        "textDocument/didClose" => {
            if let Some(uri) = req
                .get("params")
                .and_then(|p| p.get("textDocument"))
                .and_then(|d| d.get("uri").and_then(|u| u.as_str()))
            {
                documents.remove(uri);
            }
        }
        "shutdown" => {
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": null
            });
            send_response(stdout, &response);
            std::process::exit(0);
        }
        _ => {
            // Delegate request methods to process_request
            if let Some(response) = process_request(req, documents) {
                send_response(stdout, &response);
            } else if let Some(i) = id {
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": i,
                    "error": { "code": -32601, "message": "Method not found" }
                });
                send_response(stdout, &response);
            }
        }
    }
}

fn send_response(stdout: &mut io::Stdout, response: &Value) {
    let Ok(msg) = serde_json::to_string(response) else {
        return;
    };
    let payload = format!("Content-Length: {}\r\n\r\n{}", msg.len(), msg);
    if stdout.write_all(payload.as_bytes()).is_ok() {
        let _ = stdout.flush();
    }
}

fn publish_diagnostics(uri: &str, text: &str, stdout: &mut io::Stdout) {
    let mut lexer = Lexer::new(text.to_string());
    let tokens = match lexer.scan_tokens() {
        Ok(t) => t,
        Err(diag) => {
            send_diagnostics_rpc(uri, std::slice::from_ref(&diag), stdout);
            return;
        }
    };

    let mut parser = Parser::new(tokens);
    let (program, mut diags) = parser.parse();

    // Executamos o Type Checker independentemente de parsing incompleto
    // Se a AST existir em algum formato será verificada.
    if !program.is_empty() {
        let mut tenv = TypeEnv::new();
        let mut tinf = TypeInfer::new(&mut tenv);
        if let Err(type_diags) = tinf.run(&program) {
            diags.extend(type_diags);
        }
    }

    send_diagnostics_rpc(uri, &diags, stdout);
}

fn send_diagnostics_rpc(uri: &str, diags: &[Diagnostic], stdout: &mut io::Stdout) {
    let lsp_diags: Vec<Value> = diags
        .iter()
        .map(|d| {
            let severity = match d.kind {
                DiagnosticKind::Lex | DiagnosticKind::Parse | DiagnosticKind::Type => 1, // Error
                DiagnosticKind::Lint => 2,                                               // Warning
                DiagnosticKind::Runtime => 1,
                _ => 1,
            };
            // Artcode Span lines and cols are 1-indexed. LSP is 0-indexed.
            let line = d.span.line.saturating_sub(1);
            let col = d.span.col.saturating_sub(1);
            // Fallback approximation of End position (Span length if single line)
            let end_col = col + d.span.end.saturating_sub(d.span.start);

            serde_json::json!({
                "range": {
                    "start": { "line": line, "character": col },
                    "end": { "line": line, "character": end_col }
                },
                "severity": severity,
                "source": "artcode",
                "message": d.message
            })
        })
        .collect();

    let response = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "textDocument/publishDiagnostics",
        "params": {
            "uri": uri,
            "diagnostics": lsp_diags
        }
    });
    send_response(stdout, &response);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definition_finds_let_binding() {
        let src = "let answer = 42;\nprintln(answer);";
        let defs = collect_declarations(src);
        let d = defs.get("answer").expect("missing answer declaration");
        assert_eq!(d.line, 0);
        assert_eq!(d.start_char, 4);
    }

    #[test]
    fn rename_detects_word_boundaries() {
        let src = "let a = 1;\nlet aa = a;\nprintln(a);";
        let occ = find_identifier_occurrences(src, "a");
        assert_eq!(occ.len(), 3);
    }

    #[test]
    fn semantic_tokens_produces_data() {
        let src = "let x = 1;\nfunc f(v) { return v + x }";
        let data = semantic_tokens_data(src);
        assert!(!data.is_empty());
        assert_eq!(data.len() % 5, 0);
    }

    #[test]
    fn definition_resolves_across_open_documents() {
        let mut docs = HashMap::new();
        docs.insert(
            "file:///main.art".to_string(),
            "import \"./lib.art\"\nprintln(answer)".to_string(),
        );
        docs.insert("file:///lib.art".to_string(), "let answer = 42".to_string());

        let loc = resolve_definition_location(&docs, "file:///main.art", 1, 9)
            .expect("definition should resolve in lib.art");
        assert_eq!(loc.0, "file:///lib.art");
        assert_eq!(loc.1.line, 0);
    }

    #[test]
    fn rename_produces_changes_for_multiple_documents() {
        let mut docs = HashMap::new();
        docs.insert(
            "file:///main.art".to_string(),
            "import \"./lib.art\"\nprintln(answer)".to_string(),
        );
        docs.insert(
            "file:///lib.art".to_string(),
            "let answer = 42\nprintln(answer)".to_string(),
        );

        let edit = workspace_rename_edits(&docs, "file:///main.art", 1, 9, "result")
            .expect("expected workspace edit");
        let changes = edit
            .get("changes")
            .and_then(|c| c.as_object())
            .expect("changes should be object");
        assert!(changes.contains_key("file:///main.art"));
        assert!(changes.contains_key("file:///lib.art"));
    }

    #[test]
    fn completion_includes_identifiers_from_workspace_documents() {
        let mut docs = HashMap::new();
        docs.insert(
            "file:///main.art".to_string(),
            "println(helper)".to_string(),
        );
        docs.insert(
            "file:///lib.art".to_string(),
            "func helper(x) { return x }".to_string(),
        );

        let items = workspace_completion_items(&docs);
        assert!(
            items
                .iter()
                .any(|i| i.get("label").and_then(|l| l.as_str()) == Some("helper"))
        );
    }

    #[test]
    fn file_uri_roundtrips_to_the_same_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let file_path = tmp.path().join("round trip.art");
        std::fs::write(&file_path, "let x = 1;\n").expect("write file");

        let uri = to_file_uri(&file_path).expect("uri");
        assert!(uri.starts_with("file:///"), "unexpected uri: {}", uri);
        assert!(!uri.contains(r"\\?\"), "verbatim prefix leaked: {}", uri);

        let decoded = decode_file_uri_path(&uri).expect("decode");
        assert_eq!(
            std::fs::canonicalize(&decoded).expect("canonicalize decoded"),
            std::fs::canonicalize(&file_path).expect("canonicalize original"),
        );
    }

    #[test]
    fn percent_encoded_drive_letter_decodes() {
        // VS Code sends Windows paths as `file:///c%3A/dir/file.art`.
        let decoded = decode_file_uri_path("file:///c%3A/dir/file.art").expect("decode");
        let expected = if cfg!(windows) {
            r"c:\dir\file.art"
        } else {
            "/c:/dir/file.art"
        };
        assert_eq!(decoded, PathBuf::from(expected));
    }

    #[test]
    fn definition_resolves_imported_file_not_open() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let main_path = tmp.path().join("main.art");
        let lib_path = tmp.path().join("lib.art");

        std::fs::write(&main_path, "import lib;\nprintln(answer);\n").expect("write main");
        std::fs::write(&lib_path, "let answer = 42;\n").expect("write lib");

        // Build the URIs the same way the server does, so the assertions stay
        // valid on Windows (drive letters, backslashes) as well as on Unix.
        let main_uri = to_file_uri(&main_path).expect("main uri");
        let lib_uri = to_file_uri(&lib_path).expect("lib uri");
        let mut docs = HashMap::new();
        docs.insert(
            main_uri.clone(),
            std::fs::read_to_string(&main_path).expect("read main"),
        );

        let loc = resolve_definition_location(&docs, &main_uri, 1, 9)
            .expect("definition should resolve in lib.art from disk");
        assert_eq!(loc.0, lib_uri);
        assert_eq!(loc.1.line, 0);
    }

    #[test]
    fn rename_updates_imported_file_not_open() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let main_path = tmp.path().join("main.art");
        let lib_path = tmp.path().join("lib.art");

        std::fs::write(&main_path, "import lib;\nprintln(answer);\n").expect("write main");
        std::fs::write(&lib_path, "let answer = 42;\nprintln(answer);\n").expect("write lib");

        // Build the URIs the same way the server does, so the assertions stay
        // valid on Windows (drive letters, backslashes) as well as on Unix.
        let main_uri = to_file_uri(&main_path).expect("main uri");
        let lib_uri = to_file_uri(&lib_path).expect("lib uri");
        let mut docs = HashMap::new();
        docs.insert(
            main_uri.clone(),
            std::fs::read_to_string(&main_path).expect("read main"),
        );

        let edit = workspace_rename_edits(&docs, &main_uri, 1, 9, "result")
            .expect("expected workspace edit");
        let changes = edit
            .get("changes")
            .and_then(|c| c.as_object())
            .expect("changes should be object");
        assert!(changes.contains_key(&main_uri));
        assert!(changes.contains_key(&lib_uri));
    }

    // --- process_request smoke tests ---

    fn make_req(id: u64, method: &str, params: Value) -> Value {
        serde_json::json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
    }

    #[test]
    fn smoke_initialize_returns_capabilities() {
        let mut docs = HashMap::new();
        let req = make_req(1, "initialize", serde_json::json!({ "capabilities": {} }));
        let resp = process_request(&req, &mut docs).expect("initialize must return a response");
        assert_eq!(resp["id"], 1);
        let caps = &resp["result"]["capabilities"];
        assert_eq!(caps["hoverProvider"], true);
        assert_eq!(caps["definitionProvider"], true);
        assert!(caps["completionProvider"].is_object());
        assert_eq!(caps["renameProvider"], true);
    }

    #[test]
    fn smoke_completion_returns_builtins() {
        let mut docs = HashMap::new();
        docs.insert("file:///a.art".to_string(), "let x = 1".to_string());
        let req = make_req(
            2,
            "textDocument/completion",
            serde_json::json!({ "textDocument": { "uri": "file:///a.art" }, "position": { "line": 0, "character": 0 } }),
        );
        let resp = process_request(&req, &mut docs).expect("completion must return a response");
        let items = resp["result"]["items"].as_array().expect("items array");
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
        assert!(
            labels.contains(&"println"),
            "println should be a completion item"
        );
        assert!(
            labels.contains(&"str_split"),
            "str_split should be a completion item"
        );
    }

    #[test]
    fn smoke_hover_shows_function_signature() {
        let src = "func greet(name: String) -> String { return name }";
        let mut docs = HashMap::new();
        docs.insert("file:///b.art".to_string(), src.to_string());
        let req = make_req(
            3,
            "textDocument/hover",
            serde_json::json!({ "textDocument": { "uri": "file:///b.art" }, "position": { "line": 0, "character": 5 } }),
        );
        let resp = process_request(&req, &mut docs).expect("hover must return a response");
        let md = resp["result"]["contents"]["value"]
            .as_str()
            .expect("markdown value");
        assert!(md.contains("greet"), "hover should mention function name");
    }

    #[test]
    fn smoke_definition_finds_let_binding() {
        let src = "let answer = 42;\nprintln(answer);";
        let mut docs = HashMap::new();
        docs.insert("file:///c.art".to_string(), src.to_string());
        let req = make_req(
            4,
            "textDocument/definition",
            serde_json::json!({ "textDocument": { "uri": "file:///c.art" }, "position": { "line": 1, "character": 8 } }),
        );
        let resp = process_request(&req, &mut docs).expect("definition must return a response");
        assert_eq!(resp["result"]["range"]["start"]["line"], 0);
    }

    #[test]
    fn smoke_unknown_method_returns_none() {
        let mut docs = HashMap::new();
        let req = make_req(5, "workspace/nonExistent", serde_json::json!({}));
        let resp = process_request(&req, &mut docs);
        assert!(resp.is_none(), "unknown methods should return None");
    }

    #[test]
    fn smoke_shutdown_returns_null_result() {
        let mut docs = HashMap::new();
        let req = make_req(6, "shutdown", serde_json::json!(null));
        let resp = process_request(&req, &mut docs).expect("shutdown must return a response");
        assert_eq!(resp["id"], 6);
        assert!(resp["result"].is_null());
    }
}
