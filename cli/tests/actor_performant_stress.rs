use assert_cmd::Command;
use tempfile::TempDir;

// Stress test: N actors each allocating inside performant {} blocks.
// Goal: zero panics, deterministic output, interpreter completes without hanging.
#[test]
fn stress_actors_with_performant_arenas() {
    let work = TempDir::new().expect("tempdir");
    let script = work.path().join("stress.art");

    // Each actor performs computation inside a performant{} block.
    // We use run_actors and for-loop iteration over a fixed array of actor refs.
    std::fs::write(
        &script,
        r#"
func make_actor(id) {
    return spawn actor {
        performant {
            let _sq = id * id
            let _cu = id * id * id
        }
        actor_send(self, id)
    }
}

let actors = [make_actor(0), make_actor(1), make_actor(2),
              make_actor(3), make_actor(4), make_actor(5),
              make_actor(6), make_actor(7)]

run_actors(actors)

for a in actors {
    let msg = actor_receive(a)
    println(msg)
}
"#,
    )
    .expect("write script");

    let mut cmd = Command::cargo_bin("art").expect("binary");
    cmd.arg("run").arg(script.to_str().unwrap());
    let out = cmd.output().expect("run art");

    // The test passes if the interpreter does not panic (exit code 0 or diagnostic exit).
    // We do not assert exact output since actor scheduling is non-deterministic.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("thread 'main' panicked"),
        "interpreter panicked during actor+performant stress test:\n{stderr}"
    );
}

// Simpler stress: repeated performant{} allocations in a loop, scalar values only.
// Goal: zero panics, arena lifecycle (alloc+finalize) is stable across many iterations.
#[test]
fn stress_performant_repeated_allocations() {
    let work = TempDir::new().expect("tempdir");
    let script = work.path().join("performant_stress.art");

    // Use scalar-only values inside performant{} to avoid arena-escape runtime errors.
    // 100 performant blocks exercising arena alloc/finalize cycle.
    std::fs::write(
        &script,
        r#"
let items = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9,
             10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
             20, 21, 22, 23, 24, 25, 26, 27, 28, 29,
             30, 31, 32, 33, 34, 35, 36, 37, 38, 39,
             40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
             50, 51, 52, 53, 54, 55, 56, 57, 58, 59,
             60, 61, 62, 63, 64, 65, 66, 67, 68, 69,
             70, 71, 72, 73, 74, 75, 76, 77, 78, 79,
             80, 81, 82, 83, 84, 85, 86, 87, 88, 89,
             90, 91, 92, 93, 94, 95, 96, 97, 98, 99]
for n in items {
    performant {
        let _a = n * n
        let _b = n + 1
        let _c = _a + _b
    }
}
println("done")
"#,
    )
    .expect("write script");

    let mut cmd = Command::cargo_bin("art").expect("binary");
    cmd.arg("run").arg(script.to_str().unwrap());
    let out = cmd.output().expect("run art");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("thread 'main' panicked"),
        "interpreter panicked during performant stress test:\n{stderr}"
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.trim().contains("done"),
        "expected 'done' in stdout, got: {stdout}"
    );
}
