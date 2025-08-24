use assert_cmd::Command;
use tempfile::TempDir;
// use std::fs; // not needed directly

#[test]
fn resolver_finds_package_in_cache() {
    // create temp dir to act as HOME
    let home = TempDir::new().expect("home tempdir");
    let cache = home.path().join(".artcode").join("cache");
    std::fs::create_dir_all(&cache).expect("create cache");

    // install a package 'pkg-0.1.0' with main.art
    let pkg_dir = cache.join("pkg-0.1.0");
    std::fs::create_dir_all(&pkg_dir).expect("create pkg dir");
    std::fs::write(pkg_dir.join("main.art"), "let x = 123;").expect("write main.art");

    // create main program that imports 'pkg'
    let work = TempDir::new().expect("workdir");
    let main = work.path().join("main.art");
    std::fs::write(&main, "import pkg;\nlet y = x;").expect("write main");

    // Run art with HOME env pointing to our temp home so resolver finds cache
    let mut cmd = Command::cargo_bin("art").expect("binary");
    cmd.arg("run").arg(main.to_str().unwrap());
    cmd.env("HOME", home.path());
    let out = cmd.output().expect("run art");
    assert!(out.status.success(), "art failed: stdout={} stderr={}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}
