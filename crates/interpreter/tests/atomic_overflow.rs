use interpreter::Interpreter;
use core::ast::ArtValue;

#[test]
fn atomic_add_overflow_emits_diag() {
    let mut interp = Interpreter::new();
    // create atomic with large value near i64::MAX
    let hv = interp.heap_create_atomic(ArtValue::Int(i64::MAX - 1));
    if let ArtValue::Atomic(h) = hv {
        // adding 10 should overflow
        let res = interp.heap_atomic_add(h, 10);
        assert!(res.is_none());
        let diags = interp.take_diagnostics();
        assert!(diags.iter().any(|d| d.message.contains("overflow")));
    } else {
        panic!("expected atomic handle");
    }
}
