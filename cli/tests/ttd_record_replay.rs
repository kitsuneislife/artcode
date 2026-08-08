//! End-to-end smoke test for time-travel debugging through the CLI.
//!
//! `crates/interpreter/tests/time_travel.rs` covers the `Tracer`/`Replayer`
//! API in-process. This test covers the pair of commands a user actually runs
//! — `art run --record` followed by `art debug --replay` — including the
//! interactive shell reading from stdin.
//!
//! Ported from the former `scripts/run_ttd_keyframes_smoke.sh`, which nothing
//! ever invoked and which wrote its output into the tracked `examples/_outputs`
//! directory.

use assert_cmd::Command;
use std::path::PathBuf;
use tempfile::TempDir;

fn workspace_root() -> PathBuf {
    // `cli/` is one level below the workspace root.
    match PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent() {
        Some(p) => p.to_path_buf(),
        None => panic!("cannot locate workspace root"),
    }
}

#[test]
fn record_then_replay_completes_without_panicking() {
    let example = workspace_root()
        .join("examples")
        .join("44_ttd_keyframes.art");
    assert!(example.exists(), "missing example: {}", example.display());

    // The trace goes to a temp dir so a failed run never leaves state behind.
    let work = TempDir::new().expect("workdir");
    let trace = work.path().join("44_ttd_keyframes.artlog");

    let record = Command::cargo_bin("art")
        .expect("binary")
        .arg("run")
        .arg("--record")
        .arg(&trace)
        .arg(&example)
        .output()
        .expect("run art --record");
    assert!(
        record.status.success(),
        "record failed: {}",
        String::from_utf8_lossy(&record.stderr)
    );
    assert!(trace.exists(), "trace not written to {}", trace.display());

    // The debug shell is interactive: blank lines step forward, and it exits
    // when stdin closes. Thirty of them are enough to walk this trace.
    let replay = Command::cargo_bin("art")
        .expect("binary")
        .arg("debug")
        .arg("--replay")
        .arg(&trace)
        .arg(&example)
        .write_stdin("\n".repeat(30))
        .output()
        .expect("run art debug --replay");

    let stderr = String::from_utf8_lossy(&replay.stderr);
    assert!(
        !stderr.to_lowercase().contains("panic"),
        "panic during replay: {}",
        stderr
    );
    assert!(
        replay.status.success(),
        "replay exited with {}: {}",
        replay.status,
        stderr
    );
}
