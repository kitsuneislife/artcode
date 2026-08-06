//! Shared helpers for the integration tests that shell out to the `art` binary.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Directory the running test executable was built into (`<target-dir>/<profile>`).
///
/// The harness lives at `<target-dir>/<profile>/deps/<test>`, so the profile
/// directory is two levels up. Deriving it this way instead of hardcoding
/// `target/debug` keeps these tests working under any target directory —
/// notably `cargo llvm-cov`, which builds into `target/llvm-cov-target` and
/// therefore never produces `target/debug/art` at all.
pub fn profile_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors().nth(2).map(Path::to_path_buf)
}

/// Path the `art` binary occupies for the current build.
pub fn art_binary(profile_dir: &Path) -> PathBuf {
    profile_dir.join(format!("art{}", std::env::consts::EXE_SUFFIX))
}

/// Builds `cli` into the same target directory the test harness came from.
///
/// Needed when these tests run on their own (`cargo test -p interpreter`),
/// which never builds the `cli` package. Both the target directory and the
/// profile come from the harness path, so the binary lands where
/// [`art_binary`] looks for it.
pub fn build_cli(profile_dir: &Path) -> bool {
    let (Some(target_dir), Some(profile)) = (profile_dir.parent(), profile_dir.file_name()) else {
        return false;
    };

    let mut cmd = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()));
    cmd.arg("build").arg("-p").arg("cli");
    cmd.arg("--target-dir").arg(target_dir);
    if profile == "release" {
        cmd.arg("--release");
    }

    matches!(cmd.status(), Ok(st) if st.success())
}

/// Resolves the `art` binary, building it once if it is not there yet.
pub fn ensure_art_binary() -> PathBuf {
    let profile_dir = match profile_dir() {
        Some(p) => p,
        None => panic!("nao foi possivel localizar o diretorio de build a partir do executavel"),
    };

    let bin = art_binary(&profile_dir);
    if !bin.exists() {
        assert!(build_cli(&profile_dir), "cargo build -p cli falhou");
        assert!(bin.exists(), "binario art ausente em {}", bin.display());
    }
    bin
}

/// Workspace root, derived from this crate's manifest directory.
pub fn workspace_root() -> PathBuf {
    match PathBuf::from(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2) {
        Some(p) => p.to_path_buf(),
        None => panic!("nao foi possivel localizar a raiz do workspace"),
    }
}

/// Runs `art run <example>` and asserts it exits successfully.
pub fn run_example(example: &str) {
    let bin = ensure_art_binary();
    let path = workspace_root().join("examples").join(example);
    let status = match Command::new(&bin).arg("run").arg(&path).status() {
        Ok(s) => s,
        Err(e) => panic!("falha ao executar o binario art: {:?}", e),
    };
    assert!(status.success(), "execução de {} falhou", example);
}
