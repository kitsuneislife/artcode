use diagnostics::{Diagnostic, DiagnosticKind};
use interpreter::type_infer::{TypeEnv, TypeInfer};
use lexer::lexer::Lexer;
use parser::parser::Parser;
use serde_json::Value;
use std::io::{self, BufRead, Read, Write};

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
                        "definitionProvider": true
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
            // Skeleton return empty (not found/unsupported yet)
            let response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": null
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
