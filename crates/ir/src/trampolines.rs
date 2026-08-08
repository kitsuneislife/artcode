use std::mem::transmute;

/// Call a compiled function pointer safely using known trampolines for common
/// signatures. Returns Err if the requested signature is unsupported or the
/// JIT execution aborted (deopt).
pub enum Sig {
    I64_0,
    I64_1,
    I64_2,
}

pub fn call_jit_fn(addr: usize, sig: Sig, args: &[i64]) -> Result<i64, String> {
    match sig {
        Sig::I64_0 => {
            if !args.is_empty() {
                return Err("expected 0 args".to_string());
            }
            let mut out = 0i64;
            let f: extern "C" fn(*mut i64) -> i64 = unsafe { transmute(addr) };
            let status = f(&mut out);
            if status == 0 {
                Ok(out)
            } else {
                Err("deopt".to_string())
            }
        }
        Sig::I64_1 => {
            if args.len() != 1 {
                return Err("expected 1 arg".to_string());
            }
            let mut out = 0i64;
            let f: extern "C" fn(*mut i64, i64) -> i64 = unsafe { transmute(addr) };
            let status = f(&mut out, args[0]);
            if status == 0 {
                Ok(out)
            } else {
                Err("deopt".to_string())
            }
        }
        Sig::I64_2 => {
            if args.len() != 2 {
                return Err("expected 2 args".to_string());
            }
            let mut out = 0i64;
            let f: extern "C" fn(*mut i64, i64, i64) -> i64 = unsafe { transmute(addr) };
            let status = f(&mut out, args[0], args[1]);
            if status == 0 {
                Ok(out)
            } else {
                Err("deopt".to_string())
            }
        }
    }
}
