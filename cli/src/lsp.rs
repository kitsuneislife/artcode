use diagnostics::{Diagnostic, DiagnosticKind};
use core::TokenType;
use interpreter::type_infer::{TypeEnv, TypeInfer};
use lexer::lexer::Lexer;
use parser::parser::Parser;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Read, Write};

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

fn is_identifier_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
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
            let result = req
                .get("params")
                .and_then(|p| p.get("textDocument"))
                .and_then(|d| d.get("uri").and_then(|u| u.as_str()))
                .and_then(|uri| {
                    let text = documents.get(uri)?;
                    let pos = req.get("params")?.get("position")?;
                    let line = pos.get("line")?.as_u64()? as usize;
                    let character = pos.get("character")?.as_u64()? as usize;
                    let word = word_at_position(text, line, character)?;
                    let decls = collect_declarations(text);
                    let d = decls.get(&word)?;
                    Some(serde_json::json!({
                        "uri": uri,
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
                .and_then(|uri| documents.get(uri))
                .map(|text| completion_items(text))
                .unwrap_or_default();

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
                let text = documents.get(uri)?;
                let pos = params.get("position")?;
                let line = pos.get("line")?.as_u64()? as usize;
                let character = pos.get("character")?.as_u64()? as usize;
                let old_name = word_at_position(text, line, character)?;
                let new_name = params.get("newName")?.as_str()?;

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

                Some(serde_json::json!({
                    "changes": {
                        uri: edits
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
}
