use serde_json::Value;
use std::io::{self, BufRead, Read, Write};

pub fn start_server() {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut stdout = io::stdout();

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
            handle_message(&parsed, &mut stdout);
        }
    }
}

fn handle_message(req: &Value, stdout: &mut io::Stdout) {
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
