use std::path::{Path, PathBuf};
use std::process::Command;

/// Locates the `art` binary next to the running test executable.
///
/// The test harness lives at `<target-dir>/<profile>/deps/<test>`, so the
/// binary sits two levels up. Deriving the path this way instead of hardcoding
/// `target/debug/art` keeps the test working under any target directory —
/// notably `cargo llvm-cov`, which builds into `target/llvm-cov-target` and
/// therefore never produces `target/debug/art` at all.
fn profile_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors().nth(2).map(Path::to_path_buf)
}

fn art_binary(profile_dir: &Path) -> PathBuf {
    profile_dir.join(format!("art{}", std::env::consts::EXE_SUFFIX))
}

/// Builds `cli` into the same target directory the test harness came from.
///
/// Needed when this test runs on its own (`cargo test -p interpreter`), since
/// that never builds the `cli` package. Both the target directory and the
/// profile are taken from the harness path so the binary lands where
/// `art_binary` will look for it.
fn build_cli(profile_dir: &Path) -> bool {
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

/// Teste de integração que executa o binário `art` apenas para o exemplo 13.
#[test]
fn run_example_13() {
    let root = match PathBuf::from(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2) {
        Some(p) => p.to_path_buf(),
        None => panic!("integration_example_13 setup failed"),
    };
    let profile_dir = match profile_dir() {
        Some(p) => p,
        None => panic!("nao foi possivel localizar o diretorio de build a partir do executavel"),
    };

    let bin = art_binary(&profile_dir);
    if !bin.exists() {
        assert!(build_cli(&profile_dir), "cargo build -p cli falhou");
        assert!(bin.exists(), "binario art ausente em {}", bin.display());
    }

    let example = root.join("examples/13_weak_cycle_demo.art");
    let status = match Command::new(&bin).arg("run").arg(example).status() {
        Ok(s) => s,
        Err(e) => panic!("falha ao executar o binario art: {:?}", e),
    };
    assert!(status.success(), "execução do exemplo 13 falhou");
}
