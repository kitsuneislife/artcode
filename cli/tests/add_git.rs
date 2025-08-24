use assert_cmd::Command;
use tempfile::TempDir;
// use std::fs; // intentionally not required; use fully qualified std::fs in test body
use std::process::Command as ProcCommand;

#[test]
fn art_add_installs_from_git_file_url() {
    // create a local git repo
    let repo = TempDir::new().expect("repo");
    let pkg = repo.path().join("gitlib");
    std::fs::create_dir_all(&pkg).expect("mkdir");
    std::fs::write(pkg.join("main.art"), "let x = 42;").expect("write");
    std::fs::write(pkg.join("Art.toml"), "name = \"gitlib\"\nversion = \"0.2.0\"").expect("write toml");
    // init git
    let status = ProcCommand::new("git").arg("init").arg("-q").arg(pkg.to_str().unwrap()).status().expect("git init");
    assert!(status.success());
    let status = ProcCommand::new("git").arg("-C").arg(pkg.to_str().unwrap()).arg("add").arg(".").status().expect("git add");
    assert!(status.success());
    let status = ProcCommand::new("git").arg("-C").arg(pkg.to_str().unwrap()).arg("commit").arg("-m").arg("init").status().expect("git commit");
    assert!(status.success());

    // run art add with file:// URL
    let url = format!("file://{}", pkg.to_str().unwrap());
    let mut cmd = Command::cargo_bin("art").expect("binary");
    cmd.arg("add").arg(&url);
    let home = TempDir::new().expect("home");
    cmd.env("HOME", home.path());
    let out = cmd.output().expect("run art add");
    assert!(out.status.success(), "art add failed: stderr={}", String::from_utf8_lossy(&out.stderr));
    // check cache dir
    let cache_pkg = home.path().join(".artcode").join("cache").join("gitlib-0.2.0");
    assert!(cache_pkg.exists(), "cache package not found");
    assert!(cache_pkg.join("main.art").exists(), "main.art missing in cache");
}
