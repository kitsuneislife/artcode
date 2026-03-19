use assert_cmd::Command;

#[test]
fn doc_std_lists_registered_builtins() {
    let mut cmd = Command::cargo_bin("art").expect("binary present");
    cmd.arg("doc").arg("std");

    let output = cmd.output().expect("run art doc std");
    assert!(output.status.success(), "art doc std exited non-zero");

    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(
        stdout.contains("Gerado automaticamente a partir do registro de builtins do prelude"),
        "stdout should indicate generated docs"
    );
    assert!(
        stdout.contains("dag_topo_sort(nodes: Array, deps: Array)"),
        "stdout should include dag_topo_sort signature"
    );
    assert!(
        stdout.contains("arena_with(arena_id: Int, callback: Fn)"),
        "stdout should include arena_with signature"
    );
    assert!(
        stdout.contains("rand_next()"),
        "stdout should include random builtin signature"
    );
}
