use diagnostics::{Diagnostic, DiagnosticKind};
use core::TokenType;
use interpreter::type_infer::{TypeEnv, TypeInfer};
use lexer::lexer::Lexer;
use parser::parser::Parser;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};

const TOKEN_TYPES: [&str; 6] = ["keyword", "variable", "function", "string", "number", "operator"];
const KEYWORDS: &[&str] = &[
    "let", "if", "else", "true", "false", "struct", "enum", "and", "or", "match", "case",
    "import", "func", "performant", "spawn", "actor", "return", "while", "for", "in", "try",
    "catch", "weak", "unowned", "none", "as",
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
            TokenType::Let => {
                if i + 1 < tokens.len() {
                    let id = &tokens[i + 1];
                    if matches!(id.token_type, TokenType::Identifier) {
                        map.entry(id.lexeme.clone()).or_insert(SymbolDecl {
                            line: id.line.saturating_sub(1),
                            start_char: id.col.saturating_sub(1),
                            end_char: id.col.saturating_sub(1) + id.lexeme.chars().count(),
                        });
                    }
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
                    while j < tokens.len() && !matches!(tokens[j].token_type, TokenType::RightParen) {
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
            TokenType::For => {
                if i + 1 < tokens.len() {
                    let id = &tokens[i + 1];
                    if matches!(id.token_type, TokenType::Identifier) {
                        map.entry(id.lexeme.clone()).or_insert(SymbolDecl {
                            line: id.line.saturating_sub(1),
                            start_char: id.col.saturating_sub(1),
                            end_char: id.col.saturating_sub(1) + id.lexeme.chars().count(),
                        });
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    map
}

fn collect_workspace_declarations(documents: &HashMap<String, String>) -> HashMap<String, SymbolLoc> {
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

fn decode_file_uri_path(uri: &str) -> Option<PathBuf> {
    let raw = uri.strip_prefix("file://")?;
    // Minimal decode for common VSCode file URIs.
    let decoded = raw
        .replace("%20", " ")
        .replace("%23", "#")
        .replace("%25", "%");
    Some(PathBuf::from(decoded))
}

fn to_file_uri(path: &Path) -> Option<String> {
    let canonical = std::fs::canonicalize(path).ok()?;
    Some(format!("file://{}", canonical.to_string_lossy()))
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
    all_documents.entry(uri.to_string()).or_insert(current_text.clone());

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
    defs.get(&word).map(|loc| (loc.uri.clone(), loc.decl.clone()))
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

    let mut seen = HashSet::new();
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
    let mut labels = HashSet::new();
    for kw in KEYWORDS {
        labels.insert((*kw).to_string());
    }

    let mut uris: Vec<&String> = all_docs.keys().collect();
    uris.sort();
    for uri in uris {
        if let Some(text) = all_docs.get(uri) {
            let mut lexer = Lexer::new(text.to_string());
            if let Ok(tokens) = lexer.scan_tokens() {
                for t in tokens {
                    if matches!(t.token_type, TokenType::Identifier) {
                        labels.insert(t.lexeme);
                    }
                }
            }
        }
    }

    let mut names: Vec<String> = labels.into_iter().collect();
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let kind = if is_keyword_name(&name) { 14 } else { 6 };
            serde_json::json!({"label": name, "kind": kind})
        })
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
                if parts.len() == 2 {
                    if let Ok(len) = parts[1].trim().parse::<usize>() {
                        content_length = Some(len);
                    }
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

fn handle_message(
    req: &Value,
    stdout: &mut io::Stdout,
    documents: &mut std::collections::HashMap<String, String>,
) {
    let id = req.get("id");
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

    match method {
        "initialize" => {
            // Send back ServerCapabilities
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "capabilities": {
                        "textDocumentSync": 1, // Full
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
                    "serverInfo": {
                        "name": "art-lsp",
                        "version": "0.1.0"
                    }
                }
            });
            send_response(stdout, &response);
        }
        "initialized" => {
            // Client is ready, nothing to reply to (Notification)
        }
        "textDocument/didOpen" => {
            if let Some(doc) = req.get("params").and_then(|p| p.get("textDocument")) {
                if let (Some(uri), Some(text)) = (
                    doc.get("uri").and_then(|u| u.as_str()),
                    doc.get("text").and_then(|t| t.as_str()),
                ) {
                    documents.insert(uri.to_string(), text.to_string());
                    publish_diagnostics(uri, text, stdout);
                }
            }
        }
        "textDocument/didChange" => {
            if let Some(params) = req.get("params") {
                if let Some(uri) = params
                    .get("textDocument")
                    .and_then(|d| d.get("uri").and_then(|u| u.as_str()))
                {
                    if let Some(changes) = params.get("contentChanges").and_then(|c| c.as_array()) {
                        if let Some(change) = changes.last() {
                            if let Some(text) = change.get("text").and_then(|t| t.as_str()) {
                                documents.insert(uri.to_string(), text.to_string());
                                publish_diagnostics(uri, text, stdout);
                            }
                        }
                    }
                }
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
        "textDocument/hover" => {
            // Skeleton implementation
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "contents": {
                        "kind": "markdown",
                        "value": "**Artcode Type Inference:** `unknown`\n\n*(LSP Hover Prototype running...)*"
                    }
                }
            });
            send_response(stdout, &response);
        }
        "textDocument/definition" => {
            let result = req.get("params").and_then(|params| {
                let uri = params
                    .get("textDocument")
                    .and_then(|d| d.get("uri").and_then(|u| u.as_str()))?;
                let pos = params.get("position")?;
                let line = pos.get("line")?.as_u64()? as usize;
                let character = pos.get("character")?.as_u64()? as usize;
                let (decl_uri, d) = resolve_definition_location(documents, uri, line, character)?;
                Some(serde_json::json!({
                    "uri": decl_uri,
                    "range": {
                        "start": { "line": d.line, "character": d.start_char },
                        "end": { "line": d.line, "character": d.end_char }
                    }
                }))
            });
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result
            });
            send_response(stdout, &response);
        }
        "textDocument/completion" => {
            let result = req
                .get("params")
                .and_then(|p| p.get("textDocument"))
                .and_then(|d| d.get("uri").and_then(|u| u.as_str()))
                .and_then(|uri| {
                    if documents.contains_key(uri) {
                        Some(workspace_completion_items(documents))
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| completion_items(""));

            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "isIncomplete": false,
                    "items": result
                }
            });
            send_response(stdout, &response);
        }
        "textDocument/rename" => {
            let result = req.get("params").and_then(|params| {
                let uri = params
                    .get("textDocument")
                    .and_then(|d| d.get("uri").and_then(|u| u.as_str()))?;
                let pos = params.get("position")?;
                let line = pos.get("line")?.as_u64()? as usize;
                let character = pos.get("character")?.as_u64()? as usize;
                let new_name = params.get("newName")?.as_str()?;

                workspace_rename_edits(documents, uri, line, character, new_name)
            });

            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": result
            });
            send_response(stdout, &response);
        }
        "textDocument/semanticTokens/full" => {
            let data = req
                .get("params")
                .and_then(|p| p.get("textDocument"))
                .and_then(|d| d.get("uri").and_then(|u| u.as_str()))
                .and_then(|uri| documents.get(uri))
                .map(|text| semantic_tokens_data(text))
                .unwrap_or_default();

            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "data": data
                }
            });
            send_response(stdout, &response);
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
            // Ignore unrecognized methods
            if let Some(i) = id {
                let response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": i,
                    "error": {
                        "code": -32601,
                        "message": "Method not found"
                    }
                });
                send_response(stdout, &response);
            }
        }
    }
}

fn send_response(stdout: &mut io::Stdout, response: &Value) {
    let msg = serde_json::to_string(response).unwrap();
    let payload = format!("Content-Length: {}\r\n\r\n{}", msg.len(), msg);
    if let Ok(_) = stdout.write_all(payload.as_bytes()) {
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
        docs.insert(
            "file:///lib.art".to_string(),
            "let answer = 42".to_string(),
        );

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
        docs.insert("file:///main.art".to_string(), "println(helper)".to_string());
        docs.insert(
            "file:///lib.art".to_string(),
            "func helper(x) { return x }".to_string(),
        );

        let items = workspace_completion_items(&docs);
        assert!(items
            .iter()
            .any(|i| i.get("label").and_then(|l| l.as_str()) == Some("helper")));
    }

    #[test]
    fn definition_resolves_imported_file_not_open() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let main_path = tmp.path().join("main.art");
        let lib_path = tmp.path().join("lib.art");

        std::fs::write(&main_path, "import lib;\nprintln(answer);\n").expect("write main");
        std::fs::write(&lib_path, "let answer = 42;\n").expect("write lib");

        let main_uri = format!("file://{}", main_path.to_string_lossy());
        let lib_uri = format!("file://{}", lib_path.to_string_lossy());
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

        let main_uri = format!("file://{}", main_path.to_string_lossy());
        let lib_uri = format!("file://{}", lib_path.to_string_lossy());
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
}
