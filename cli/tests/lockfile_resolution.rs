use assert_cmd::Command;
use tempfile::TempDir;
use std::fs;

#[test]
fn resolver_prefers_art_lock() {
    // prepare cache package
    let home = TempDir::new().expect("home");
    let cache = home.path().join(".artcode").join("cache");
    fs::create_dir_all(&cache).expect("create cache");
    let pkg_dir = cache.join("pinned-0.0.1");
    fs::create_dir_all(&pkg_dir).expect("pkg dir");
    fs::write(pkg_dir.join("main.art"), "let x = 999;").expect("write");
    fs::write(pkg_dir.join("Art.toml"), "name = \"pinned\"\nversion = \"0.0.1\"").expect("write toml");

    // create project with .art-lock pointing to cache package
    let proj = TempDir::new().expect("proj");
    let main = proj.path().join("main.art");
    fs::write(&main, "import pinned;\nlet y = x;").expect("write main");
    let lock = serde_json::json!({"name": "pinned", "version": "0.0.1", "path": pkg_dir.to_string_lossy()});
    fs::write(proj.path().join(".art-lock"), serde_json::to_string_pretty(&lock).unwrap()).expect("write lock");

    let mut cmd = Command::cargo_bin("art").expect("binary");
    cmd.arg("run").arg(main.to_str().unwrap());
    cmd.env("HOME", home.path());
    let out = cmd.output().expect("run art");
    assert!(out.status.success(), "art failed: stderr={}", String::from_utf8_lossy(&out.stderr));
}
