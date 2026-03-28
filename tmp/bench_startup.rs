use interpreter::interpreter::Interpreter;
use std::time::Instant;

fn main() {
    let start = Instant::now();
    for _ in 0..1000 {
        let _ = Interpreter::with_prelude();
    }
    let duration = start.elapsed();
    println!("1000 Interpreter::with_prelude() took: {:?}", duration);
    println!("Average time per instantiation: {:?}", duration / 1000);
}
