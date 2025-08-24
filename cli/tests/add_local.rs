use assert_cmd::Command;
use tempfile::TempDir;
use std::fs;

#[test]
fn art_add_installs_local_path() {
    let work = TempDir::new().expect("workdir");
    let pkg = work.path().join("mylib");
    std::fs::create_dir_all(&pkg).expect("mkdir");
    std::fs::write(pkg.join("main.art"), "let x = 10;").expect("write");
    std::fs::write(pkg.join("Art.toml"), "name = \"mylib\"\nversion = \"0.1.0\"").expect("write toml");

    let mut cmd = Command::cargo_bin("art").expect("binary");
    cmd.arg("add").arg(pkg.to_str().unwrap());
    // run with HOME set to a temp home so cache is isolated
    let home = TempDir::new().expect("home");
    cmd.env("HOME", home.path());
    let out = cmd.output().expect("run art add");
    assert!(out.status.success(), "art add failed: stderr={}", String::from_utf8_lossy(&out.stderr));
    // check cache dir
    let cache_pkg = home.path().join(".artcode").join("cache").join("mylib-0.1.0");
    assert!(cache_pkg.exists(), "cache package not found");
    assert!(cache_pkg.join("main.art").exists(), "main.art missing in cache");
}
