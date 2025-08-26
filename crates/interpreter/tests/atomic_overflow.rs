use interpreter::Interpreter;
use core::ast::ArtValue;
use interpreter::test_helpers::test_helpers as th;

#[test]
fn atomic_add_overflow_emits_diag() {
    let mut interp = Interpreter::new();
    // create atomic with large value near i64::MAX
    let hv = th::heap_create_atomic(&mut interp, ArtValue::Int(i64::MAX - 1));
    if let ArtValue::Atomic(h) = hv {
        // adding 10 should overflow
        let res = th::heap_atomic_add(&mut interp, h, 10);
        assert!(res.is_none());
        let diags = interp.take_diagnostics();
        assert!(diags.iter().any(|d| d.message.contains("overflow")));
    } else {
        panic!("expected atomic handle");
    }
}
