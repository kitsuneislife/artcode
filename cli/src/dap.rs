// Minimal DAP (Debug Adapter Protocol) output helpers.
// When `art debug --dap` is used, the CLI writes DAP-compatible JSON messages
// to stdout so editors with DAP support (VS Code, Neovim, etc.) can display
// the current execution position.
// Only the subset needed to show current position is implemented:
//   - `initialized` event (handshake)
//   - `stopped` event with `reason` and `source` location

use std::io::{self, Write};

fn send(body: &str) {
    let content = body.trim_end();
    let header = format!("Content-Length: {}\r\n\r\n", content.len());
    let _ = io::stdout().write_all(header.as_bytes());
    let _ = io::stdout().write_all(content.as_bytes());
    let _ = io::stdout().flush();
}

pub fn send_initialized() {
    send(r#"{"type":"event","event":"initialized","body":{}}"#);
}

pub fn send_stopped(reason: &str, path: &str, line: u32, col: u32) {
    let msg = format!(
        "{{\"type\":\"event\",\"event\":\"stopped\",\"body\":{{\"reason\":\"{reason}\",\"source\":{{\"path\":\"{path}\"}},\"line\":{line},\"column\":{col}}}}}",
    );
    send(&msg);
}

pub fn send_terminated() {
    send(r#"{"type":"event","event":"terminated","body":{}}"#);
}
