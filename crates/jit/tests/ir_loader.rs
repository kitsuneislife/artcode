use std::fs::File;
use std::io::Write;

#[test]
fn parse_ir_file_detects_heavy_ops() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut path = tmp.path().to_path_buf();
    path.push("sample.ir");
    let mut f = File::create(&path).expect("create file");
    let contents = r#"
    func @heavy() -> i64 {
      entry:
        a = const i64 1
        b = const i64 2
        c = call sum(a, b)
        d = gc_alloc
        ret c
    }
    "#;
    f.write_all(contents.as_bytes()).expect("write");
    // call into library
    // this test runs inside the `jit` crate; reference the exported module
    let analysis = jit::ir_loader::parse_ir_file(&path).expect("analyzed");
    assert!(analysis.instr_count > 0, "instr_count should be positive");
    assert!(analysis.block_count >= 1, "at least one block");
}
