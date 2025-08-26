JIT scaffold crate
===================

This crate is a small scaffold for future JIT work. The real implementation should live
behind the `jit` feature and depend on `inkwell` (LLVM bindings). The scaffold ensures
the workspace builds for contributors that don't have LLVM installed.

Usage
-----

To enable the real JIT implementation (future):

1. Install LLVM and the required development headers on your system.
2. Add `inkwell` to the crate with the `jit` feature and implement `compile_function`.
3. Build with `cargo build -p jit --features jit`.
