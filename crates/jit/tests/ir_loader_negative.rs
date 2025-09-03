use std::fs::File;
use std::io::Write;

#[test]
fn rejects_missing_header() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut path = tmp.path().to_path_buf();
    path.push("bad.ir");
    let mut f = File::create(&path).expect("create");
    let contents = r#"
    // missing func header
      entry:
        a = const i64 1
        ret a
    }
    "#;
    f.write_all(contents.as_bytes()).expect("write");
    assert!(jit::ir_loader::parse_ir_file(&path).is_none());
}

#[test]
fn rejects_unknown_assignment() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut path = tmp.path().to_path_buf();
    path.push("bad2.ir");
    let mut f = File::create(&path).expect("create");
    let contents = r#"
    func @f() -> i64 {
      entry:
        x = unknown_op y, z
        ret x
    }
    "#;
    f.write_all(contents.as_bytes()).expect("write");
    assert!(jit::ir_loader::parse_ir_file(&path).is_none());
}
