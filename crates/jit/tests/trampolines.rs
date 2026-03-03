use jit::{call_jit_fn, Sig};

// helper: get function pointer as usize for a known function type

extern "C" fn zero() -> i64 {
    42
}
extern "C" fn one(x: i64) -> i64 {
    x + 1
}
extern "C" fn two(a: i64, b: i64) -> i64 {
    a + b
}

#[test]
fn call_zero() {
    let p = unsafe { std::mem::transmute::<extern "C" fn() -> i64, usize>(zero) };
    let r = call_jit_fn(p, Sig::I64_0, &[]).expect("call");
    assert_eq!(r, 42);
}

#[test]
fn call_one() {
    let p = unsafe { std::mem::transmute::<extern "C" fn(i64) -> i64, usize>(one) };
    let r = call_jit_fn(p, Sig::I64_1, &[5]).expect("call");
    assert_eq!(r, 6);
}

#[test]
fn call_two() {
    let p = unsafe { std::mem::transmute::<extern "C" fn(i64, i64) -> i64, usize>(two) };
    let r = call_jit_fn(p, Sig::I64_2, &[2, 3]).expect("call");
    assert_eq!(r, 5);
}

#[test]
fn wrong_arity_errors() {
    let p = unsafe { std::mem::transmute::<extern "C" fn() -> i64, usize>(zero) };
    assert!(call_jit_fn(p, Sig::I64_1, &[]).is_err());
}
